use async_trait::async_trait;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use serde_json::json;

use crate::cookie::value_of as cookie_value;
use crate::error::{keep_most_actionable, Error, Reason, Result};
use crate::model::{Fee, PlayUrl, SearchPage, Source, Variant};
use crate::net::Net;
use crate::platform::{CredStore, MusicPlatform, SearchQuery};

const REFERER: &str = "https://y.qq.com/";
const MUSICU: &str = "https://u.y.qq.com/cgi-bin/musicu.fcg";
const COVER_BASE: &str = "https://y.qq.com/music/photo_new/T002R300x300M000";

/// QQ 音乐：搜索走 `client_search_cp`，播放链接走 `musicu.fcg` 的 vkey 接口。
pub struct QqMusic {
    net: Net,
    creds: CredStore,
}

impl QqMusic {
    pub fn new(net: Net, creds: CredStore) -> Self {
        Self { net, creds }
    }

    fn headers(&self) -> Vec<(&'static str, &str)> {
        vec![("Referer", REFERER), ("Origin", "https://y.qq.com")]
    }

    /// 请求 vkey 并拼出完整可下载地址。`filenames` 为空表示让服务端给默认档。
    ///
    /// 返回 `Ok(None)` 表示「服务端正常响应但没有这个档位」，调用方可以继续降级；
    /// 返回 `Err` 表示这首在当前账号下明确下不了，原因已经分类好。
    async fn vkey(
        &self,
        mid: &str,
        filenames: Option<&[String]>,
    ) -> Result<Option<(String, String)>> {
        let guid = guid();
        // 未登录时 uin 固定 0（匿名）；登录后必须带真实 uin，否则服务端不会下发 vkey
        let cookie = self.creds.cookie(Source::Qq);
        let uin = cookie
            .as_deref()
            .and_then(|value| cookie_value(value, "uin").or_else(|| cookie_value(value, "p_uin")))
            .map(|value| value.trim_start_matches(['o', 'u', '_']).to_string())
            .filter(|value| !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or_else(|| "0".to_string());
        // comm.uin 在协议里是数字，param.uin 是字符串，两者不能混用
        let uin_num: u64 = uin.parse().unwrap_or(0);
        let mut param = json!({
            "guid": guid,
            "songmid": [mid],
            "songtype": [0],
            "uin": uin,
            "loginflag": 1,
            "platform": "20"
        });
        if let Some(files) = filenames {
            param["filename"] = json!(files);
        }
        let payload = json!({
            "req": {
                "module": "CDN.SrfCdnDispatchServer",
                "method": "GetCdnDispatch",
                "param": { "guid": guid, "calltype": 0, "userip": "" }
            },
            "req_0": {
                "module": "vkey.GetVkeyServer",
                "method": "CgiGetVkey",
                "param": param
            },
            "comm": { "uin": uin_num, "format": "json", "ct": 6, "cv": 0 }
        });
        let encoded = utf8_percent_encode(&payload.to_string(), NON_ALPHANUMERIC).to_string();
        let url = format!("{MUSICU}?format=json&data={encoded}");

        let mut headers = self.headers();
        if let Some(ref c) = cookie {
            headers.push(("Cookie", c.as_str()));
        }

        let body: serde_json::Value = self.net.json(&url, &headers).await?;

        // 整个请求被拒：匿名请求固定 104009（msg 形如 "1.2.3.4;invalidq;"），
        // 这是「必须登录」最明确的信号，比继续降级音质有意义得多。
        let req_code = body
            .pointer("/req_0/code")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if req_code != 0 {
            let msg = body
                .pointer("/req_0/data/msg")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let reason = match req_code {
                // 匿名请求的固定返回值，登录后即可解除
                104009 => Reason::NeedLogin,
                // 版权限制 / 不支持该歌曲
                104003 => Reason::Copyright,
                _ => Reason::Unknown,
            };
            return Err(Error::unavailable_with(
                Source::Qq,
                reason,
                if msg.is_empty() {
                    format!("req_0.code={req_code}")
                } else {
                    format!("req_0.code={req_code} {msg}")
                },
            ));
        }

        let purl = body
            .pointer("/req_0/data/midurlinfo/0/purl")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if purl.is_empty() {
            // 没有链接时，`result` 才是真正的说明：
            // 0       —— 有权限但这次没给（继续降级无意义，也算取不到）
            // 101404  —— 该音源没有这个档位的文件
            let result = body
                .pointer("/req_0/data/midurlinfo/0/result")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let need_buy = body
                .pointer("/req_0/data/midurlinfo/0/pneedbuy")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let tips = body
                .pointer("/req_0/data/midurlinfo/0/tips")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let reason = match result {
                // 档位不存在：继续往下试更低音质是合理的，交给调用方处理
                101404 => return Ok(None),
                _ if need_buy != 0 => Reason::VipOnly,
                _ if !tips.is_empty() => Reason::VipOnly,
                _ => Reason::Unknown,
            };
            return Err(Error::unavailable_with(
                Source::Qq,
                reason,
                if tips.is_empty() {
                    format!("result={result}")
                } else {
                    tips.to_string()
                },
            ));
        }
        let sip = body
            .pointer("/req_0/data/sip/0")
            .and_then(|v| v.as_str())
            .unwrap_or("http://ws.stream.qqmusic.qq.com/");
        let full = if purl.starts_with("http") {
            purl.to_string()
        } else {
            format!(
                "{}{}",
                sip.trim_end_matches('/'),
                format!("/{purl}").as_str()
            )
        };
        let format = purl
            .rsplit('.')
            .next()
            .filter(|ext| ext.len() <= 4 && !ext.contains('/'))
            .unwrap_or("m4a")
            .to_string();
        Ok(Some((full, format)))
    }
}

#[async_trait]
impl MusicPlatform for QqMusic {
    fn source(&self) -> Source {
        Source::Qq
    }

    async fn search(&self, query: &SearchQuery) -> Result<SearchPage> {
        let keyword = utf8_percent_encode(&query.keyword, NON_ALPHANUMERIC).to_string();
        let url = format!(
            "https://c.y.qq.com/soso/fcgi-bin/client_search_cp?p={}&n={}&w={keyword}&format=json&cr=1&t=0",
            query.page, query.limit
        );
        let raw: RawSearch = self.net.json(&url, &self.headers()).await?;
        let song = raw
            .data
            .and_then(|d| d.song)
            .ok_or_else(|| Error::decode("QQ 音乐搜索返回空 data.song"))?;
        let total = song.totalnum.unwrap_or(0);
        let items: Vec<Variant> = song
            .list
            .unwrap_or_default()
            .into_iter()
            .map(|song| song.into_variant())
            .collect();
        let has_more = (query.offset() as u64 + items.len() as u64) < total;
        Ok(SearchPage {
            source: Source::Qq,
            page: query.page,
            total,
            has_more,
            items,
        })
    }

    async fn resolve(&self, variant: &Variant, bitrate: u32) -> Result<PlayUrl> {
        let mid = variant.id.as_str();
        // 档位含义：F000 = FLAC，M800 = 320k mp3，M500 = 128k mp3。
        // 从高到低依次尝试，命中即返回；全部落空时把信息量最大的原因带出去。
        let mut chain: Vec<(&str, u32, &str)> = Vec::new();
        if bitrate >= 700 {
            chain.push(("F000", 999, "flac"));
        }
        if bitrate >= 300 {
            chain.push(("M800", 320, "mp3"));
        }
        chain.push(("M500", 128, "mp3"));

        let mut best: Option<Error> = None;
        for (prefix, level, ext) in chain {
            let files = vec![format!("{prefix}{mid}{mid}.{ext}")];
            match self.vkey(mid, Some(&files)).await {
                Ok(Some((url, format))) => {
                    let actual = if format == "flac" { 999 } else { level };
                    return Ok(PlayUrl {
                        url,
                        bitrate: actual,
                        format,
                        size: 0,
                    });
                }
                // 该档位服务端没有文件，继续试更低一档
                Ok(None) => {}
                Err(err) => keep_most_actionable(&mut best, err),
            }
        }
        Err(best.unwrap_or_else(|| Error::unavailable(Source::Qq, Reason::QualityMissing)))
    }
}

fn guid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(123_456_789);
    format!("{:010}", 1_000_000_000 + nanos % 9_000_000_000)
}

#[derive(Debug, Deserialize)]
struct RawSearch {
    #[serde(default)]
    data: Option<RawData>,
}

#[derive(Debug, Deserialize)]
struct RawData {
    #[serde(default)]
    song: Option<RawSongBlock>,
}

#[derive(Debug, Deserialize)]
struct RawSongBlock {
    #[serde(default)]
    list: Option<Vec<RawSong>>,
    #[serde(default)]
    totalnum: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RawSong {
    #[serde(default)]
    songmid: String,
    #[serde(default)]
    songname: String,
    #[serde(default)]
    singer: Vec<RawSinger>,
    #[serde(default)]
    albumname: Option<String>,
    #[serde(default)]
    albummid: Option<String>,
    /// 时长（秒）
    #[serde(default)]
    interval: u64,
    #[serde(default)]
    size128: u64,
    #[serde(default)]
    size320: u64,
    #[serde(default)]
    sizeflac: u64,
    #[serde(default)]
    pay: Option<RawPay>,
}

#[derive(Debug, Deserialize)]
struct RawSinger {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize, Default)]
struct RawPay {
    #[serde(default)]
    payplay: i32,
    #[serde(default)]
    paytrackprice: i32,
}

impl RawSong {
    fn into_variant(self) -> Variant {
        let mut variant = Variant::new(Source::Qq, self.songmid.clone(), self.songname);
        variant.artists = self.singer.into_iter().map(|s| s.name).collect();
        variant.album = self.albumname.unwrap_or_default();
        variant.cover = self
            .albummid
            .filter(|mid| !mid.is_empty())
            .map(|mid| format!("{COVER_BASE}{mid}.jpg"));
        variant.duration_ms = self.interval.saturating_mul(1000);
        if self.sizeflac > 0 {
            variant.bitrate = 999;
            variant.format = "flac".into();
            variant.size = self.sizeflac;
        } else if self.size320 > 0 {
            variant.bitrate = 320;
            variant.format = "mp3".into();
            variant.size = self.size320;
        } else {
            variant.bitrate = 128;
            variant.format = "mp3".into();
            variant.size = self.size128;
        }
        variant.fee = match self.pay {
            Some(pay) if pay.paytrackprice > 0 => Fee::Paid,
            Some(pay) if pay.payplay > 0 => Fee::Vip,
            Some(_) => Fee::Free,
            None => Fee::Unknown,
        };
        variant
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_payload() {
        let json = r#"{"code":0,"data":{"song":{"curnum":2,"totalnum":480,"list":[
          {"songmid":"0039MnYb0qxYhV","songname":"晴天","interval":269,
           "albumname":"叶惠美","albummid":"000MkMni19ClKG",
           "singer":[{"name":"周杰伦"}],"size128":4317292,"size320":10792943,"sizeflac":55397039,
           "pay":{"payplay":1,"paytrackprice":200}}
        ]}}}"#;
        let raw: RawSearch = serde_json::from_str(json).unwrap();
        let block = raw.data.unwrap().song.unwrap();
        assert_eq!(block.totalnum, Some(480));
        let variant = block.list.unwrap().remove(0).into_variant();
        assert_eq!(variant.id, "0039MnYb0qxYhV");
        assert_eq!(variant.duration_ms, 269_000);
        assert_eq!(variant.bitrate, 999);
        assert_eq!(variant.format, "flac");
        assert_eq!(variant.fee, Fee::Paid);
        assert_eq!(
            variant.cover.as_deref(),
            Some("https://y.qq.com/music/photo_new/T002R300x300M000000MkMni19ClKG.jpg")
        );
    }

    #[test]
    fn guid_is_ten_digits() {
        let value = guid();
        assert_eq!(value.len(), 10);
        assert!(value.chars().all(|c| c.is_ascii_digit()));
    }
}
