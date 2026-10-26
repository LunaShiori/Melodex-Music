use async_trait::async_trait;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

use crate::error::{Error, Reason, Result};
use crate::model::{Fee, PlayUrl, SearchPage, Source, Variant};
use crate::net::Net;
use crate::platform::{CredStore, MusicPlatform, SearchQuery};
use crate::pseudo_json;
use crate::quality::Quality;

const SEARCH_API: &str = "http://search.kuwo.cn/r.s";
const REFERER: &str = "http://www.kuwo.cn/";
/// 播放直链的附加请求头（本地代理用）。
const STREAM_HEADERS: &[(&str, &str)] = &[("Referer", REFERER)];

/// 酷我音乐：搜索接口返回单引号 JSON，用 [`crate::pseudo_json`] 解析；
/// 播放链接由 `antiserver` 直出，拿到就能下。
pub struct Kuwo {
    net: Net,
    #[allow(dead_code)]
    creds: CredStore,
}

impl Kuwo {
    pub fn new(net: Net, creds: CredStore) -> Self {
        Self { net, creds }
    }
}

#[async_trait]
impl MusicPlatform for Kuwo {
    fn source(&self) -> Source {
        Source::Kuwo
    }

    fn stream_headers(&self) -> &'static [(&'static str, &'static str)] {
        STREAM_HEADERS
    }

    async fn search(&self, query: &SearchQuery) -> Result<SearchPage> {
        let keyword = utf8_percent_encode(&query.keyword, NON_ALPHANUMERIC).to_string();
        let url = format!(
            "{SEARCH_API}?all={keyword}&ft=music&itemset=web_2013&client=kt&pn={}&rn={}&rformat=json&encoding=utf8",
            query.offset(),
            query.limit
        );
        let body = self
            .net
            .text(
                &url,
                &[("Referer", REFERER), ("User-Agent", crate::net::UA)],
            )
            .await?;
        let value = pseudo_json::parse(&body)
            .map_err(|err| Error::decode(format!("酷我响应解析失败：{err}")))?;
        let total = value.num_of("TOTAL") as u64;
        let list = value
            .get("abslist")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::decode("酷我搜索缺少 abslist"))?;

        let items: Vec<Variant> = list
            .iter()
            .filter_map(|item| {
                let rid = item.str_of("MUSICRID");
                let id = rid.strip_prefix("MUSIC_").unwrap_or(&rid).to_string();
                if id.is_empty() {
                    return None;
                }
                let mut variant = Variant::new(Source::Kuwo, id, clean(&item.str_of("SONGNAME")));
                variant.artists = split_artists(&clean(&item.str_of("ARTIST")));
                variant.album = clean(&item.str_of("ALBUM"));
                variant.duration_ms = (item.num_of("DURATION") as u64).saturating_mul(1000);
                let formats = item.str_of("FORMATS");
                variant.bitrate = bitrate_of(&formats);
                variant.format = if formats.contains("AAC") && variant.bitrate == 0 {
                    "aac".into()
                } else {
                    "mp3".into()
                };
                let pic = item.str_of("web_albumpic_short");
                if !pic.is_empty() {
                    variant.cover = Some(format!("https://img2.kuwo.cn/star/albumcover/{pic}"));
                }
                variant.fee = if item.str_of("PAY") == "1" {
                    Fee::Vip
                } else {
                    Fee::Free
                };
                Some(variant)
            })
            .collect();

        let has_more = (query.offset() as u64 + items.len() as u64) < total;
        Ok(SearchPage {
            source: Source::Kuwo,
            page: query.page,
            total,
            has_more,
            items,
        })
    }

    async fn resolve(&self, variant: &Variant, quality: &'static Quality) -> Result<PlayUrl> {
        // 酷我高音质走 mobi.kuwo.cn/mobi.s 的 convert_url_with_sign 接口，
        // 用数字 rid + `br={bitrate}k{format}` 定位资源，可拿到 320k/flac/母带。
        // 加密格式（mflac/mgg）返回 ekey，需 kwDES 解密后交给下载器 QMCv2 解密。
        let rid: u64 = variant
            .id
            .parse()
            .map_err(|_| Error::unavailable(Source::Kuwo, Reason::QualityMissing))?;
        if rid == 0 {
            return Err(Error::unavailable(Source::Kuwo, Reason::QualityMissing));
        }

        // 每次请求生成随机 user（u32）和 android_id（16 位 hex）
        let user: u32 = random_u32();
        let android_id: String = random_hex(16);
        // br 用真实的档位码（code 字段），如 20900kmflac / 2000kflac / 320kmp3
        let br = format!("{}k{}", quality.code, quality.format);
        let url = format!(
            "http://mobi.kuwo.cn/mobi.s?f=web&user={user}&android_id={android_id}&\
             source=kwplayer_ar_5.1.0.0_B_jiakong_vh.apk&type=convert_url_with_sign&from=PC&\
             rid={rid}&br={br}&format={}",
            quality.format
        );

        let body = self
            .net
            .text(
                &url,
                &[("User-Agent", "okhttp/4.10.0"), ("Referer", REFERER)],
            )
            .await?;
        let data: serde_json::Value = serde_json::from_str(&body)
            .map_err(|err| Error::decode(format!("酷我 mobi.s 响应解析失败：{err}")))?;

        let code = data.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
        if code != 200 {
            let msg = data
                .get("msg")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            return Err(Error::unavailable_with(
                Source::Kuwo,
                Reason::QualityMissing,
                if msg.is_empty() {
                    format!("code={code}")
                } else {
                    msg
                },
            ));
        }

        let data_obj = data
            .get("data")
            .ok_or_else(|| Error::unavailable(Source::Kuwo, Reason::QualityMissing))?;
        let link = data_obj
            .get("url")
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| Error::unavailable(Source::Kuwo, Reason::QualityMissing))?;

        // 加密格式（mflac/mgg）取 ekey，用 kwDES 解密后去掉 user 前缀
        let format = quality.format;
        let encrypted = matches!(format, "mflac" | "mgg");
        let ekey = if encrypted {
            let raw = data_obj
                .get("ekey")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if raw.is_empty() {
                None
            } else {
                let decrypted = super::kuwo_des::base64_decrypt(raw, super::kuwo_des::DEFAULT_KEY)
                    .map_err(|e| Error::decode(format!("酷我 ekey 解密失败：{e}")))?;
                // 去掉开头的本次随机 user 前缀
                let user_str = user.to_string();
                Some(
                    decrypted
                        .strip_prefix(&user_str)
                        .unwrap_or(&decrypted)
                        .to_string(),
                )
            }
        } else {
            None
        };

        Ok(PlayUrl {
            url: link.to_string(),
            bitrate: quality.bitrate,
            format: format.to_string(),
            size: 0,
            ekey,
        })
    }
}

/// 酷我返回的文本里带 HTML 实体，统一还原。
fn clean(raw: &str) -> String {
    raw.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .trim()
        .to_string()
}

fn split_artists(raw: &str) -> Vec<String> {
    raw.split(['&', ',', '、', '/'])
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

/// `FORMATS` 里 MP3H 表示 320kbps，MP3128 表示 128kbps。
fn bitrate_of(formats: &str) -> u32 {
    if formats.contains("MP3H") {
        320
    } else if formats.contains("MP3128") {
        128
    } else if formats.contains("AAC") {
        96
    } else {
        0
    }
}

/// 生成随机 u32（mobi.s 接口的 user 参数）。
fn random_u32() -> u32 {
    use getrandom::getrandom;
    let mut buf = [0u8; 4];
    let _ = getrandom(&mut buf);
    u32::from_le_bytes(buf)
}

/// 生成 n 字节的随机小写 hex 字符串（mobi.s 接口的 android_id 参数）。
fn random_hex(bytes: usize) -> String {
    use getrandom::getrandom;
    let mut buf = vec![0u8; bytes];
    let _ = getrandom(&mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_entities() {
        assert_eq!(clean("晴天&nbsp;(KTV版伴奏)"), "晴天 (KTV版伴奏)");
        assert_eq!(clean(" A&amp;B "), "A&B");
    }

    #[test]
    fn splits_artists() {
        assert_eq!(split_artists("周杰伦&方文山"), vec!["周杰伦", "方文山"]);
        assert_eq!(split_artists("周杰伦、方文山"), vec!["周杰伦", "方文山"]);
        assert!(split_artists("").is_empty());
    }

    #[test]
    fn maps_bitrate() {
        assert_eq!(bitrate_of("AAC48|MP3128|MP3H|WMA128"), 320);
        assert_eq!(bitrate_of("MP3128|WMA96"), 128);
        assert_eq!(bitrate_of("AAC48"), 96);
        assert_eq!(bitrate_of("WMA96"), 0);
    }

    #[test]
    fn parses_abslist_into_variants() {
        let body = "{'TOTAL':'3599','abslist':[{'MUSICRID':'MUSIC_51685512','SONGNAME':'晴天&nbsp;(KTV版伴奏)','ARTIST':'周杰伦','ALBUM':'叶惠美','DURATION':'269','FORMATS':'AAC48|MP3128|MP3H','PAY':'0'}]}";
        let value = pseudo_json::parse(body).unwrap();
        let item = &value.get("abslist").and_then(|v| v.as_array()).unwrap()[0];
        let rid = item.str_of("MUSICRID");
        assert_eq!(rid.strip_prefix("MUSIC_"), Some("51685512"));
        assert_eq!(clean(&item.str_of("SONGNAME")), "晴天 (KTV版伴奏)");
        assert_eq!(bitrate_of(&item.str_of("FORMATS")), 320);
    }
}
