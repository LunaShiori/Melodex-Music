use async_trait::async_trait;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

use crate::error::{Error, Reason, Result};
use crate::model::{Fee, PlayUrl, SearchPage, Source, Variant};
use crate::net::Net;
use crate::platform::{CredStore, MusicPlatform, SearchQuery};
use crate::pseudo_json;
use crate::quality::Quality;

const SEARCH_API: &str = "http://search.kuwo.cn/r.s";
const RESOLVE_API: &str = "http://antiserver.kuwo.cn/anti.s";
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
        let url = format!(
            "{RESOLVE_API}?type=convert_url&rid=MUSIC_{}&format=mp3&response=url",
            variant.id
        );
        let body = self
            .net
            .text(
                &url,
                &[("Referer", REFERER), ("User-Agent", crate::net::UA)],
            )
            .await?;
        let link = body.trim();
        if !link.starts_with("http") {
            // antiserver 拿不到链接时返回的是纯文本，按内容判断原因
            let lowered = link.to_ascii_lowercase();
            let reason = if lowered.contains("copyright") || link.contains("版权") {
                Reason::Copyright
            } else if lowered.contains("vip") || link.contains("会员") {
                Reason::VipOnly
            } else if link.is_empty() {
                Reason::Unknown
            } else {
                Reason::Copyright
            };
            return Err(Error::unavailable_with(
                Source::Kuwo,
                reason,
                if link.is_empty() {
                    "antiserver 返回空响应".to_string()
                } else {
                    link.chars().take(60).collect::<String>()
                },
            ));
        }
        // 实测：antiserver 的 convert_url 无论 `br` 传 320k 还是 flac，
        // 返回的都是同一个 128k mp3（晴天 269 秒对应 4.32 MB）。所以档位表里
        // 只有一档，这里也不看 quality——按实际返回如实上报，绝不虚标。
        // 真实码率在下载完成后会按「文件大小 / 时长」再校准一次。
        let _ = quality;
        Ok(PlayUrl {
            url: link.to_string(),
            bitrate: 128,
            format: "mp3".into(),
            size: 0,
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
