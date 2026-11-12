//! B 站音乐区检索。
//!
//! 只做「搜」，不做「播」：视频音轨是 DASH 分离流，直链要求 `Referer: bilibili.com`，
//! 而且高清需要登录态与大会员，浏览器 `<audio>` 拿不到这些条件。所以这里只把
//! 音乐区的视频列出来，点开交给 B 站自己播放——这是能长期稳定的边界。
//!
//! 接口要 `buvid3` Cookie，否则返回 HTTP 412（风控）。首次调用先用
//! `/x/frontend/finger/spi` 换一个，进程内缓存复用。

use std::sync::Mutex;
use std::sync::OnceLock;

use serde_json::Value;

use crate::error::{Error, Result};
use crate::net::Net;

const SEARCH_API: &str = "https://api.bilibili.com/x/web-interface/search/type";
const FINGER_API: &str = "https://api.bilibili.com/x/frontend/finger/spi";
const REFERER: &str = "https://www.bilibili.com/";
/// 音乐分区 id，搜索接口用它做分区过滤。
pub const MUSIC_TID: u32 = 3;

/// 一条搜到的视频。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiliVideo {
    pub bvid: String,
    pub title: String,
    pub author: String,
    pub cover: String,
    /// 秒。
    pub duration_secs: u32,
    pub plays: u64,
    pub url: String,
}

static BUVID: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn buvid_slot() -> &'static Mutex<Option<String>> {
    BUVID.get_or_init(|| Mutex::new(None))
}

/// 取（或换一个）buvid3。失败返回空串，让调用方按风控提示处理。
async fn buvid3(net: &Net) -> String {
    if let Ok(guard) = buvid_slot().lock() {
        if let Some(value) = guard.as_ref() {
            return value.clone();
        }
    }
    let value = match net.json::<Value>(FINGER_API, &[("Referer", REFERER)]).await {
        Ok(json) => json
            .get("data")
            .and_then(|d| d.get("b_3"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        Err(_) => String::new(),
    };
    if !value.is_empty() {
        if let Ok(mut guard) = buvid_slot().lock() {
            *guard = Some(value.clone());
        }
    }
    value
}

/// 清掉缓存的 buvid3——被风控后重新握手用。
pub fn reset_buvid() {
    if let Ok(mut guard) = buvid_slot().lock() {
        *guard = None;
    }
}

/// 在音乐分区搜视频。`page` 从 1 开始。
pub async fn search(net: &Net, term: &str, page: u32) -> Result<Vec<BiliVideo>> {
    let term = term.trim();
    if term.is_empty() {
        return Ok(Vec::new());
    }
    let cookie = format!("buvid3={}", buvid3(net).await);
    let q = format!(
        "{}?search_type=video&tid={}&keyword={}&page={}",
        SEARCH_API,
        MUSIC_TID,
        utf8_encode(term),
        page.max(1)
    );
    let headers = [("Referer", REFERER), ("Cookie", cookie.as_str())];
    let json = net.json::<Value>(&q, &headers).await?;
    let code = json.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    if code == -412 || code == 412 {
        reset_buvid();
        return Err(Error::Source {
            platform: "bilibili".into(),
            reason: "B 站风控拦截（412），稍后重试".into(),
        });
    }
    if code != 0 {
        let message = json
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("B 站接口返回异常");
        return Err(Error::Source {
            platform: "bilibili".into(),
            reason: message.to_string(),
        });
    }
    let result = json
        .get("data")
        .and_then(|d| d.get("result"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(result.iter().filter_map(video_of).collect())
}

fn video_of(raw: &Value) -> Option<BiliVideo> {
    let bvid = raw.get("bvid")?.as_str()?.to_string();
    if bvid.is_empty() {
        return None;
    }
    let title = strip_em(raw.get("title").and_then(|v| v.as_str()).unwrap_or(""));
    let author = raw
        .get("author")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let cover = raw
        .get("pic")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .replace("http://", "https://");
    // 接口偶尔给协议相对地址 `//i0.hdslb.com/...`，补上 https
    let cover = if cover.starts_with("//") {
        format!("https:{cover}")
    } else {
        cover
    };
    let duration_secs = raw
        .get("duration")
        .and_then(|v| v.as_str())
        .map(duration_of)
        .unwrap_or(0);
    let plays = raw.get("play").and_then(|v| v.as_u64()).unwrap_or(0);
    Some(BiliVideo {
        url: format!("https://www.bilibili.com/video/{bvid}"),
        bvid,
        title,
        author,
        cover,
        duration_secs,
        plays,
    })
}

/// 搜索结果标题里带 `<em class="keyword">` 高亮，去掉标签只留文字。
fn strip_em(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_tag = false;
    for ch in raw.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    decode_entities(out.trim())
}

fn decode_entities(raw: &str) -> String {
    raw.replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#39;", "'")
}

/// `04:52` / `1:02:33` → 秒。
fn duration_of(raw: &str) -> u32 {
    raw.split(':')
        .filter_map(|part| part.trim().parse::<u32>().ok())
        .fold(0u32, |acc, v| acc.saturating_mul(60).saturating_add(v))
}

fn utf8_encode(raw: &str) -> String {
    raw.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_highlight_tags_and_entities() {
        let raw = r#"<em class="keyword">晴天</em> 周杰伦 &amp; 叶惠美"#;
        assert_eq!(strip_em(raw), "晴天 周杰伦 & 叶惠美");
    }

    #[test]
    fn parses_durations() {
        assert_eq!(duration_of("04:52"), 292);
        assert_eq!(duration_of("1:02:33"), 3753);
        assert_eq!(duration_of(""), 0);
    }

    #[test]
    fn reads_video_fields() {
        let raw = serde_json::json!({
            "bvid": "BV1BZbSzZEGT",
            "title": "<em class=\"keyword\">晴天</em>",
            "author": "某UP主",
            "pic": "http://i0.hdslb.com/bfs/archive/x.jpg",
            "duration": "04:52",
            "play": 12345
        });
        let video = video_of(&raw).expect("应解析出视频");
        assert_eq!(video.bvid, "BV1BZbSzZEGT");
        assert_eq!(video.title, "晴天");
        assert_eq!(video.author, "某UP主");
        assert!(video.cover.starts_with("https://"));
        assert_eq!(video.duration_secs, 292);
        assert_eq!(video.url, "https://www.bilibili.com/video/BV1BZbSzZEGT");
    }

    #[test]
    fn skips_records_without_bvid() {
        let raw = serde_json::json!({ "title": "无 bvid" });
        assert!(video_of(&raw).is_none());
    }

    /// 真实网络探针：`cargo test --features net -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_search_music() {
        let net = Net::new().expect("net");
        let videos = search(&net, "晴天 周杰伦", 1).await.expect("搜索应成功");
        println!("first: {:?}", videos.first());
        assert!(!videos.is_empty(), "音乐分区应有结果");
    }
}
