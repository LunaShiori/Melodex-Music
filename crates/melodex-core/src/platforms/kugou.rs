use async_trait::async_trait;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde_json::Value;

use crate::error::{Error, Reason, Result};
use crate::model::{Fee, PlayUrl, SearchPage, Source, Variant};
use crate::net::{Net, ProxyFor};
use crate::platform::{MusicPlatform, SearchQuery};
use crate::quality::Quality;

const SEARCH_API: &str = "https://songsearch.kugou.com/song_search_v2";
const PLAY_API: &str = "https://m.kugou.com/app/i/getSongInfo.php";
const REFERER: &str = "https://www.kugou.com/";
/// 本音源的标识，用于「哪些音源走代理」的判定。
const SOURCE: Source = Source::Kugou;
/// 播放直链的附加请求头（本地代理用）。
const STREAM_HEADERS: &[(&str, &str)] = &[("Referer", REFERER)];
/// 搜索页默认每页条数上限，接口自己也会夹。
const PAGE_SIZE_MAX: u32 = 50;

/// 酷狗音乐。
///
/// 搜索走 `songsearch.kugou.com` 的网页接口，播放链接走手机页面的
/// `getSongInfo`——后者不需要签名，但**同一首歌的不同音质是三个不同 hash**
/// （128k / 320k / 无损各一份），所以 `Variant::alt_ids` 里按
/// `"sq:<hash>"` / `"hq:<hash>"` 的标记形式存着，解析时按档位挑。
pub struct Kugou {
    net: Net,
}

impl Kugou {
    pub fn new(net: Net) -> Self {
        Self { net }
    }

    /// 按档位挑了该用哪个 hash：无损取 `sq:`，320k 取 `hq:`，其余用主 id。
    fn hash_for(variant: &Variant, quality: &'static Quality) -> String {
        let want = match quality.code {
            "sq" => "sq:",
            "hq" => "hq:",
            _ => "",
        };
        if want.is_empty() {
            return variant.id.clone();
        }
        variant
            .alt_ids
            .iter()
            .find_map(|tagged| tagged.strip_prefix(want))
            .filter(|hash| !hash.is_empty())
            .map(|hash| hash.to_string())
            // 会员档没有独立 hash 时退回主 id：能取到 128k 也比整首失败好
            .unwrap_or_else(|| variant.id.clone())
    }
}

#[async_trait]
impl MusicPlatform for Kugou {
    fn source(&self) -> Source {
        Source::Kugou
    }

    fn stream_headers(&self) -> &'static [(&'static str, &'static str)] {
        STREAM_HEADERS
    }

    async fn search(&self, query: &SearchQuery) -> Result<SearchPage> {
        let keyword = utf8_percent_encode(&query.keyword, NON_ALPHANUMERIC).to_string();
        let pagesize = query.limit.clamp(1, PAGE_SIZE_MAX);
        let url = format!(
            "{SEARCH_API}?keyword={keyword}&page={}&pagesize={pagesize}&platform=WebFilter\
             &userid=0&clientver=2000&iscorrection=1&privilege_filter=0",
            query.page
        );
        let body = self
            .net
            .text_src(
                SOURCE,
                ProxyFor::Search,
                &url,
                &[("Referer", REFERER), ("User-Agent", crate::net::UA)],
            )
            .await?;
        let value: Value = serde_json::from_str(&body)
            .map_err(|err| Error::decode(format!("酷狗搜索结果解析失败：{err}")))?;

        let data = value
            .get("data")
            .ok_or_else(|| Error::decode("酷狗搜索缺少 data"))?;
        let total = data.get("total").and_then(Value::as_u64).unwrap_or(0);
        let list = data
            .get("lists")
            .and_then(Value::as_array)
            .ok_or_else(|| Error::decode("酷狗搜索缺少 lists"))?;

        let items: Vec<Variant> = list.iter().filter_map(variant_of).collect();
        let has_more = (query.offset() as u64 + items.len() as u64) < total;
        Ok(SearchPage {
            source: Source::Kugou,
            page: query.page,
            total,
            has_more,
            items,
        })
    }

    async fn resolve(&self, variant: &Variant, quality: &'static Quality) -> Result<PlayUrl> {
        let hash = Self::hash_for(variant, quality);
        if hash.is_empty() {
            return Err(Error::unavailable(Source::Kugou, Reason::QualityMissing));
        }
        let url = format!("{PLAY_API}?cmd=playInfo&hash={hash}");
        let body = self
            .net
            .text_src(
                SOURCE,
                ProxyFor::Link,
                &url,
                &[(
                    "User-Agent",
                    "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X)",
                )],
            )
            .await?;
        let value: Value = serde_json::from_str(&body)
            .map_err(|err| Error::decode(format!("酷狗播放信息解析失败：{err}")))?;

        let link = value
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        if link.is_empty() {
            // 链接为空是酷狗的付费/版权标记：pay_type > 0 或 privilege 越高越受限
            let pay_type = value.get("pay_type").and_then(Value::as_i64).unwrap_or(0);
            let privilege = value.get("privilege").and_then(Value::as_i64).unwrap_or(0);
            let reason = if pay_type > 0 || privilege >= 10 {
                Reason::VipOnly
            } else {
                Reason::QualityMissing
            };
            return Err(Error::unavailable_with(
                Source::Kugou,
                reason,
                format!("pay_type={pay_type} privilege={privilege}"),
            ));
        }

        let bitrate = value
            .get("bitRate")
            .and_then(Value::as_u64)
            .unwrap_or(quality.bitrate as u64) as u32;
        let format = value
            .get("extName")
            .and_then(Value::as_str)
            .filter(|ext| !ext.is_empty())
            .unwrap_or(quality.format);
        let size = value.get("fileSize").and_then(Value::as_u64).unwrap_or(0);

        Ok(PlayUrl {
            url: link,
            bitrate,
            format: format.to_string(),
            size,
            ekey: None,
        })
    }
}

/// 把一条搜索结果转成 [`Variant`]。缺 hash 的记录直接丢掉——没有 hash 就取不到链接。
fn variant_of(item: &Value) -> Option<Variant> {
    let id = str_of(item, "FileHash");
    if id.is_empty() {
        return None;
    }
    let mut variant = Variant::new(Source::Kugou, id, str_of(item, "SongName"));
    variant.artists = split_artists(&str_of(item, "SingerName"));
    variant.album = str_of(item, "AlbumName");
    variant.duration_ms = item
        .get("Duration")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_mul(1000);
    variant.fee = if num_of(item, "Privilege") >= 10 || num_of(item, "PayType") > 0 {
        Fee::Vip
    } else {
        Fee::Free
    };
    variant.cover = cover_of(item);

    // 320k / 无损的独立 hash，带上档位标记存进 alt_ids
    let hq = str_of(item, "HQFileHash");
    if !hq.is_empty() {
        variant.alt_ids.push(format!("hq:{hq}"));
    }
    let sq = str_of(item, "SQFileHash");
    if !sq.is_empty() {
        variant.alt_ids.push(format!("sq:{sq}"));
    }

    // 界面上标最高可用档位：有 SQ hash 算无损，有 HQ 算 320k
    if !sq.is_empty() {
        variant.bitrate = 1_000;
        variant.format = "flac".into();
    } else if !hq.is_empty() {
        variant.bitrate = 320;
        variant.format = "mp3".into();
    } else {
        variant.bitrate = num_of(item, "Bitrate") as u32;
        variant.format = "mp3".into();
    }
    variant.size = num_of(item, "FileSize");
    Some(variant)
}

/// 封面地址里带 `{size}` 占位符，换成接口认的尺寸。
fn cover_of(item: &Value) -> Option<String> {
    let raw = str_of(item, "Image");
    if raw.is_empty() {
        return None;
    }
    Some(raw.replace("{size}", "480"))
}

fn str_of(item: &Value, key: &str) -> String {
    item.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn num_of(item: &Value, key: &str) -> u64 {
    item.get(key).and_then(Value::as_u64).unwrap_or(0)
}

/// 酷狗用 `、` 分隔多个歌手，偶尔混进 `/`。
fn split_artists(raw: &str) -> Vec<String> {
    raw.split(['、', '/', '&'])
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quality;
    use serde_json::json;

    fn sample() -> Value {
        json!({
            "FileHash": "AAA111",
            "HQFileHash": "BBB222",
            "SQFileHash": "CCC333",
            "SongName": "晴天",
            "SingerName": "周杰伦",
            "AlbumName": "叶惠美",
            "Duration": 269,
            "FileSize": 4317292,
            "Privilege": 10,
            "PayType": 3,
            "Image": "http://imge.kugou.com/stdmusic/{size}/cover.jpg"
        })
    }

    #[test]
    fn variant_keeps_quality_hashes() {
        let variant = variant_of(&sample()).expect("有 hash 就该产出变体");
        assert_eq!(variant.id, "AAA111");
        assert_eq!(variant.alt_ids, vec!["hq:BBB222", "sq:CCC333"]);
        assert_eq!(variant.artists, vec!["周杰伦"]);
        assert_eq!(variant.duration_ms, 269_000);
        assert_eq!(variant.bitrate, 1_000);
        assert_eq!(variant.format, "flac");
        assert_eq!(variant.fee, Fee::Vip);
        assert_eq!(
            variant.cover.as_deref(),
            Some("http://imge.kugou.com/stdmusic/480/cover.jpg")
        );
    }

    #[test]
    fn drops_records_without_hash() {
        let mut item = sample();
        item["FileHash"] = json!("");
        assert!(variant_of(&item).is_none());
    }

    #[test]
    fn picks_hash_by_quality() {
        let variant = variant_of(&sample()).unwrap();
        let sq = quality::find(Source::Kugou, crate::quality::QualityLevel::Lossless).unwrap();
        assert_eq!(Kugou::hash_for(&variant, sq), "CCC333");

        let hq = quality::find(Source::Kugou, crate::quality::QualityLevel::Exquisite).unwrap();
        assert_eq!(Kugou::hash_for(&variant, hq), "BBB222");

        let std = quality::find(Source::Kugou, crate::quality::QualityLevel::Standard).unwrap();
        assert_eq!(Kugou::hash_for(&variant, std), "AAA111");
    }

    #[test]
    fn falls_back_to_main_hash_when_quality_hash_missing() {
        let mut item = sample();
        item["SQFileHash"] = json!("");
        let variant = variant_of(&item).unwrap();
        let sq = quality::find(Source::Kugou, crate::quality::QualityLevel::Lossless).unwrap();
        assert_eq!(Kugou::hash_for(&variant, sq), "AAA111");
    }

    #[test]
    fn splits_multiple_artists() {
        assert_eq!(
            split_artists("周杰伦、方文山"),
            vec!["周杰伦".to_string(), "方文山".to_string()]
        );
    }
    /// 真实网络探针：`cargo test -p melodex-core --features net kugou -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "需要外网，手动跑"]
    async fn live_search_then_resolve() {
        let platform = Kugou::new(crate::net::Net::with_proxy("").expect("建客户端"));
        let page = platform
            .search(&SearchQuery::new("钢琴 纯音乐", 1, 10))
            .await
            .expect("搜索应成功");
        println!("total={} got={}", page.total, page.items.len());
        assert!(!page.items.is_empty(), "搜索结果不应为空");

        let mut hit = None;
        for variant in page.items.iter().take(8) {
            println!(
                "{} / {} / {:?}",
                variant.title,
                variant.artist_line(),
                variant.alt_ids
            );
            for quality in
                crate::quality::ladder(Source::Kugou, crate::quality::QualityLevel::Standard)
            {
                if let Ok(play) = platform.resolve(variant, quality).await {
                    println!(
                        "  resolved {} {}",
                        play.format,
                        play.url.get(..60).unwrap_or(&play.url)
                    );
                    hit = Some(play);
                    break;
                }
            }
            if hit.is_some() {
                break;
            }
        }
        assert!(hit.is_some(), "至少应有一首能取到直链");
    }
}
