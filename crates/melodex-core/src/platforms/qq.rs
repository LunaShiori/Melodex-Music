use async_trait::async_trait;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use serde_json::json;

use crate::cookie::value_of as cookie_value;
use crate::error::{Error, Reason, Result};
use crate::model::{Fee, PlayUrl, SearchPage, Source, Variant};
use crate::net::Net;
use crate::platform::{CredStore, MusicPlatform, SearchQuery};
use crate::quality::{Quality, QualityLevel};

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
    /// 登录态通过 `comm.uin` + `comm.authst` 传递，**不需要拼 Cookie**：
    /// `authst` 就是扫码登录换来的 `musickey`。匿名时两者都是空串，
    /// 服务端对每个档位返回 `result = 104003`（实测）。
    ///
    /// 返回 `Ok(None)` 表示「服务端没有这个档位」，调用方可以继续降级；
    /// 返回 `Err` 表示这首在当前账号下明确下不了，原因已经分类好。
    async fn vkey(
        &self,
        mid: &str,
        filenames: Option<&[String]>,
    ) -> Result<Option<(String, String)>> {
        let cookie = self.creds.cookie(Source::Qq);
        let uin = cookie
            .as_deref()
            .and_then(|value| cookie_value(value, "uin").or_else(|| cookie_value(value, "p_uin")))
            .map(|value| value.trim_start_matches(['o', 'u', '_']).to_string())
            .filter(|value| !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or_default();
        let authst = cookie
            .as_deref()
            .and_then(|value| {
                cookie_value(value, "authst")
                    .or_else(|| cookie_value(value, "qm_keyst"))
                    .or_else(|| cookie_value(value, "qqmusic_key"))
            })
            .unwrap_or_default();

        let mut param = json!({
            "guid": guid(),
            "songmid": [mid],
            "songtype": [0],
            "uin": uin,
            "loginflag": 1,
            "platform": "20"
        });
        if let Some(files) = filenames {
            param["filename"] = json!(files);
        }

        const REQUEST_KEY: &str = "vkey.GetVkeyServer.CgiGetVkey";
        let mut body = serde_json::Map::new();
        body.insert("loginUin".into(), json!(uin));
        body.insert(
            "comm".into(),
            json!({
                "format": "json",
                "ct": 24,
                "cv": 0,
                "tmeAppID": "qqmusic",
                "uin": uin,
                "qq": uin,
                "authst": authst,
            }),
        );
        body.insert(
            REQUEST_KEY.into(),
            json!({
                "module": "vkey.GetVkeyServer",
                "method": "CgiGetVkey",
                "param": param,
            }),
        );

        let response = self
            .net
            .client()
            .post(MUSICU)
            .header("Referer", REFERER)
            .header("Origin", "https://y.qq.com")
            .json(&serde_json::Value::Object(body))
            .send()
            .await?;
        let text = response.text().await?;
        let decoded: serde_json::Value = serde_json::from_str(&text).map_err(|err| {
            let head: String = text.chars().take(120).collect();
            Error::decode(format!("{err} <- {head}"))
        })?;
        let resp = decoded.get(REQUEST_KEY).cloned().unwrap_or_default();

        // 整个请求被拒（例如匿名请求格式不对）：code 非 0
        let req_code = resp.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
        if req_code != 0 {
            let msg = resp
                .pointer("/data/msg")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let reason = match req_code {
                104009 => Reason::NeedLogin,
                104003 => Reason::NeedLogin,
                104004 => Reason::Copyright,
                _ => Reason::Unknown,
            };
            return Err(Error::unavailable_with(
                Source::Qq,
                reason,
                if msg.is_empty() {
                    format!("code={req_code}")
                } else {
                    format!("code={req_code} {msg}")
                },
            ));
        }

        let item = resp
            .pointer("/data/midurlinfo/0")
            .cloned()
            .unwrap_or_default();
        let purl = item
            .get("purl")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if purl.is_empty() {
            let result = item.get("result").and_then(|v| v.as_i64()).unwrap_or(0);
            let need_buy = item.get("pneedbuy").and_then(|v| v.as_i64()).unwrap_or(0);
            let tips = item
                .get("tips")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            // 档位不存在 → 交给调用方继续降级
            if result == 101404 {
                return Ok(None);
            }
            let reason = if need_buy != 0 || !tips.is_empty() {
                Reason::VipOnly
            } else if authst.is_empty() {
                // 没有登录令牌时，104003 的含义就是「登录后才给」
                Reason::NeedLogin
            } else {
                Reason::Copyright
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

        let sip = resp
            .pointer("/data/sip/0")
            .and_then(|v| v.as_str())
            .unwrap_or("http://ws.stream.qqmusic.qq.com/");
        let full = if purl.starts_with("http") {
            purl.to_string()
        } else {
            format!("{}/{}", sip.trim_end_matches('/'), purl)
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

    async fn resolve(&self, variant: &Variant, quality: &'static Quality) -> Result<PlayUrl> {
        let mid = variant.id.as_str();
        // 文件名 = 档位前缀 + mid + mid + 扩展名，例如 F0000039MnYb0qxYhV0039MnYb0qxYhV.flac
        let files = vec![format!(
            "{}{mid}{mid}.{}",
            quality.code, quality.format
        )];
        let missing = || Error::unavailable(Source::Qq, Reason::QualityMissing);
        match self.vkey(mid, Some(&files)).await? {
            Some((url, format)) => {
                // 服务端对没权限的档位有时不报错，而是静默回一个低码率文件。
                // 无损及以上必须是 flac，否则视为「这个档位拿不到」，交给引擎降级。
                if quality.level >= QualityLevel::Lossless && format != "flac" {
                    return Err(missing());
                }
                Ok(PlayUrl {
                    url,
                    bitrate: quality.bitrate,
                    format,
                    size: 0,
                })
            }
            // 服务端明确说没有这个档位
            None => Err(missing()),
        }
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
