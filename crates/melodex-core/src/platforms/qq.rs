use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use serde_json::json;

use crate::cookie::value_of as cookie_value;
use crate::error::{Error, Reason, Result};
use crate::lyric::unescape_entities;
use crate::model::{
    Fee, Lyric, PlayUrl, PlaylistDetail, PlaylistSummary, SearchPage, Source, Variant,
};
use crate::net::Net;
use crate::platform::{CredStore, MusicPlatform, SearchQuery};
use crate::qq_file::{candidate_files, extension_of, is_lossless_format, FileInfo};
use crate::quality::{Quality, QualityLevel};

const REFERER: &str = "https://y.qq.com/";
/// 歌词接口的 Referer 必须是播放器页，换成首页会返回空歌词。
const LYRIC_REFERER: &str = "https://y.qq.com/portal/player.html";
/// 播放直链的附加请求头（本地代理用）。
const STREAM_HEADERS: &[(&str, &str)] = &[("Referer", REFERER), ("Origin", "https://y.qq.com")];
const MUSICU: &str = "https://u.y.qq.com/cgi-bin/musicu.fcg";
const LYRIC_API: &str = "https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg";
const COVER_BASE: &str = "https://y.qq.com/music/photo_new/T002R300x300M000";

/// QQ 音乐：搜索走 `client_search_cp`，播放链接走 `musicu.fcg` 的 vkey 接口，
/// 歌词走 `fcg_query_lyric_new`，歌词/母带档位所需的 `vs` 数组走歌曲详情接口。
pub struct QqMusic {
    net: Net,
    creds: CredStore,
    /// 歌曲详情缓存：`vs` 数组按档位取不同下标的 mid（母带 `vs[3]`、全景声 `vs[4]`），
    /// 一次下载会对同一首歌反复要链接，缓存住避免每档都多发一次详情请求。
    details: Mutex<HashMap<String, FileInfo>>,
}

impl QqMusic {
    pub fn new(net: Net, creds: CredStore) -> Self {
        Self {
            net,
            creds,
            details: Mutex::new(HashMap::new()),
        }
    }

    fn headers(&self) -> Vec<(&'static str, &str)> {
        vec![("Referer", REFERER), ("Origin", "https://y.qq.com")]
    }

    /// 从已保存的 Cookie 里拆出 `uin` 与 `authst`（= 扫码登录换来的 `musickey`）。
    fn credentials(&self) -> (String, String) {
        let Some(cookie) = self.creds.cookie(Source::Qq) else {
            return (String::new(), String::new());
        };
        let uin = cookie_value(&cookie, "uin")
            .or_else(|| cookie_value(&cookie, "p_uin"))
            .or_else(|| cookie_value(&cookie, "wxuin"))
            .map(|value| value.trim_start_matches(['o', 'u', '_']).to_string())
            .filter(|value| !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or_default();
        // `cookie_value` 借的是 cookie 字符串，这里要拿走所有权：后面拼 JSON 时
        // cookie 的局部变量可能已经离开作用域。
        let authst = cookie_value(&cookie, "authst")
            .or_else(|| cookie_value(&cookie, "qm_keyst"))
            .or_else(|| cookie_value(&cookie, "qqmusic_key"))
            .or_else(|| cookie_value(&cookie, "musickey"))
            .unwrap_or_default()
            .to_string();
        (uin, authst)
    }

    fn logged_in(&self) -> bool {
        !self.credentials().1.is_empty()
    }

    /// 歌曲详情里的 `vs` / `media_mid`。带缓存，失败时返回空结构（降级用 media_mid 拼）。
    async fn file_info(&self, mid: &str) -> FileInfo {
        let cached = {
            self.details
                .lock()
                .ok()
                .and_then(|guard| guard.get(mid).cloned())
        };
        if let Some(found) = cached {
            return found;
        }
        let info = self.fetch_file_info(mid).await.unwrap_or_default();
        if let Ok(mut guard) = self.details.lock() {
            // 缓存只服务于「同一首歌连取几档」这个场景，超过上限直接清空即可
            if guard.len() > 256 {
                guard.clear();
            }
            guard.insert(mid.to_string(), info.clone());
        }
        info
    }

    async fn fetch_file_info(&self, mid: &str) -> Result<FileInfo> {
        let (uin, authst) = self.credentials();
        let body = json!({
            "comm": {
                "format": "json",
                "ct": 24,
                "cv": 0,
                "tmeAppID": "qqmusic",
                "uin": uin,
                "qq": uin,
                "authst": authst,
            },
            "songinfo": {
                "module": "music.pf_song_detail_svr",
                "method": "get_song_detail_yqq",
                "param": { "song_type": 0, "song_mid": mid },
            }
        });
        let decoded = self.musicu(&body).await?;
        let track = decoded
            .pointer("/songinfo/data/track_info")
            .cloned()
            .unwrap_or_default();
        let media_mid = track
            .pointer("/file/media_mid")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let vs = track
            .get("vs")
            .and_then(|v| v.as_array())
            .map(|list| {
                list.iter()
                    .map(|item| item.as_str().unwrap_or_default().to_string())
                    .collect()
            })
            .unwrap_or_default();
        Ok(FileInfo { media_mid, vs })
    }

    /// POST 一次 `musicu.fcg` 并返回整个响应。
    async fn musicu(&self, body: &serde_json::Value) -> Result<serde_json::Value> {
        let response = self
            .net
            .client()
            .post(MUSICU)
            .header("Referer", REFERER)
            .header("Origin", "https://y.qq.com")
            .json(body)
            .send()
            .await?;
        let text = response.text().await?;
        serde_json::from_str(&text).map_err(|err| {
            let head: String = text.chars().take(120).collect();
            Error::decode(format!("{err} <- {head}"))
        })
    }

    /// 批量要链接：一次请求把该档位的所有候选文件名都递上去，服务端按顺序回结果。
    ///
    /// 之所以要「多候选」：QQ 的文件名前缀在不同曲库/不同年代并不统一
    /// （无损既有 `F0M0{media_mid}.mflac` 也有 `F000{songmid}.flac`，
    /// 母带既要试 `AIM0{vs[3]}` 也要试 `AI00{songmid}`），
    /// 而服务端对不认识的文件名只回 `101404`，不会告诉我们「正确的拼法应该是什么」。
    /// 一次带多个候选，比逐个试要省好几个来回。
    async fn vkey_batch(&self, mid: &str, filenames: &[String]) -> Result<VkeyBatch> {
        let (uin, authst) = self.credentials();
        let body = json!({
            "loginUin": uin,
            "comm": {
                "format": "json",
                "ct": 24,
                "cv": 0,
                "tmeAppID": "qqmusic",
                "uin": uin,
                "qq": uin,
                "authst": authst,
            },
            "vkey.GetVkeyServer.CgiGetVkey": {
                "module": "vkey.GetVkeyServer",
                "method": "CgiGetVkey",
                "param": {
                    "guid": guid(),
                    // songmid 与 filename 一一对应，服务端按下标回 midurlinfo
                    "songmid": filenames.iter().map(|_| mid).collect::<Vec<_>>(),
                    "songtype": filenames.iter().map(|_| 0).collect::<Vec<_>>(),
                    "uin": uin,
                    "loginflag": 1,
                    "platform": "20",
                    "filename": filenames,
                }
            }
        });
        let decoded = self.musicu(&body).await?;
        let resp = decoded
            .get("vkey.GetVkeyServer.CgiGetVkey")
            .cloned()
            .unwrap_or_default();
        let code = resp.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
        let sip = resp
            .pointer("/data/sip/0")
            .and_then(|v| v.as_str())
            .unwrap_or("http://ws.stream.qqmusic.qq.com/")
            .trim_end_matches('/')
            .to_string();
        let items = resp
            .pointer("/data/midurlinfo")
            .and_then(|v| v.as_array())
            .map(|list| {
                list.iter()
                    .map(|item| {
                        let purl = item
                            .get("purl")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default();
                        MidUrl {
                            url: if purl.is_empty() {
                                String::new()
                            } else if purl.starts_with("http") {
                                purl.to_string()
                            } else {
                                format!("{sip}/{purl}")
                            },
                            result: item.get("result").and_then(|v| v.as_i64()).unwrap_or(0),
                            ekey: item
                                .get("ekey")
                                .and_then(|v| v.as_str())
                                .filter(|v| !v.is_empty())
                                .map(|v| v.to_string()),
                            tips: item
                                .get("tips")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default()
                                .to_string(),
                            need_buy: item.get("pneedbuy").and_then(|v| v.as_i64()).unwrap_or(0),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(VkeyBatch { code, items })
    }
}

#[async_trait]
impl MusicPlatform for QqMusic {
    fn source(&self) -> Source {
        Source::Qq
    }

    /// QQ 的 CDN 校验 Referer：不带 `https://y.qq.com/` 时直链返回 403。
    /// 下载侧我们自己拼头所以无感，播放侧要靠本地代理补上。
    fn stream_headers(&self) -> &'static [(&'static str, &'static str)] {
        STREAM_HEADERS
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
        // 取链接用 media_mid（媒体文件 mid）而不是 songmid：两者在搜索结果里通常不同，
        // 用 songmid 拼文件名时服务端对无损档返回 101404。
        //
        // 只有真需要时才去查歌曲详情：母带 / 全景声 / Hi-Res 要 `vs` 数组，
        // 而歌单接口、部分老接口压根不给 media_mid。320k 与 128k 直接拼就能取到，
        // 多打一次详情只会拖慢播放起播。
        let needs_detail =
            variant.media_mid.is_none() || matches!(quality.code, "AIM0" | "Q0M0" | "RSM1");
        let info = if needs_detail {
            self.file_info(mid).await
        } else {
            FileInfo::default()
        };
        let media = variant
            .media_mid
            .clone()
            .filter(|value| !value.is_empty())
            .or_else(|| Some(info.media_mid.clone()).filter(|value| !value.is_empty()))
            // 最后才退回 songmid：老曲目三者一致时这条能救回来
            .unwrap_or_else(|| mid.to_string());

        let files = candidate_files(quality, mid, &media, &info);
        let batch = self.vkey_batch(mid, &files).await?;

        // 顶层 code 非 0 且一个 midurlinfo 都没有，说明整次请求被拒
        if batch.items.is_empty() {
            return Err(request_error(batch.code, self.logged_in()));
        }

        let logged_in = self.logged_in();
        let mut failure: Option<Reason> = None;
        for item in &batch.items {
            if item.url.is_empty() {
                // result=101404 是「没有这个文件」，属于正常的档位缺失，交给引擎降级；
                // 其它结果才值得记下来当失败原因。
                if item.result != 101404 {
                    failure = Some(reason_of(item, logged_in));
                }
                continue;
            }
            let format = extension_of(&item.url).unwrap_or_else(|| quality.format.to_string());
            // 服务端对没权限的档位有时不报错，而是静默回一个低码率替身文件。
            // 无损及以上必须是 flac/mflac/ape，否则当作这档拿不到，继续降级。
            if quality.level >= QualityLevel::Lossless && !is_lossless_format(&format) {
                failure = Some(Reason::QualityMissing);
                continue;
            }
            return Ok(PlayUrl {
                url: item.url.clone(),
                bitrate: quality.bitrate,
                format,
                size: 0,
                ekey: item.ekey.clone(),
            });
        }

        Err(Error::unavailable_with(
            Source::Qq,
            failure.unwrap_or(Reason::QualityMissing),
            format!(
                "{}候选均未取到链接（{}）",
                files.len(),
                if logged_in { "已登录" } else { "未登录" }
            ),
        ))
    }

    async fn lyric(&self, id: &str) -> Result<Option<Lyric>> {
        // nobase64=1 让服务端直接给明文 LRC，省掉一次 base64 解码；
        // 不带这个参数时返回的是 base64 后再被 JSON 转义的文本，很容易解出乱码。
        let url = format!("{LYRIC_API}?songmid={id}&format=json&nobase64=1&g_tk=5381");
        let headers = [("Referer", LYRIC_REFERER), ("Origin", REFERER)];
        let raw: RawLyric = self.net.json(&url, &headers).await?;
        if raw.retcode.unwrap_or(0) != 0 {
            return Ok(None);
        }
        let lrc = unescape_entities(&raw.lyric.unwrap_or_default());
        if lrc.trim().is_empty() {
            return Ok(None);
        }
        let translation = raw
            .trans
            .map(|text| unescape_entities(&text))
            .filter(|text| !text.trim().is_empty());
        Ok(Some(Lyric { lrc, translation }))
    }

    async fn playlist(&self, id: &str) -> Result<Option<PlaylistDetail>> {
        let url = format!(
            "https://c.y.qq.com/qzone/fcg-bin/fcg_ucc_getcdinfo_byids_cp.fcg\
             ?type=1&json=1&utf8=1&onlysong=0&disstid={id}&format=json&g_tk=5381"
        );
        let headers = [("Referer", REFERER), ("Origin", REFERER)];
        let raw: RawCdList = self.net.json(&url, &headers).await?;
        let Some(detail) = raw.cdlist.and_then(|list| list.into_iter().next()) else {
            return Ok(None);
        };
        let items: Vec<Variant> = detail
            .songlist
            .unwrap_or_default()
            .into_iter()
            .map(|song| song.into_variant())
            .collect();
        Ok(Some(PlaylistDetail {
            source: Source::Qq,
            id: id.to_string(),
            name: detail.dissname,
            cover: detail.logo,
            total: items.len() as u64,
            items,
        }))
    }

    async fn my_playlists(&self) -> Result<Vec<PlaylistSummary>> {
        let (uin, authst) = self.credentials();
        if uin.is_empty() || authst.is_empty() {
            return Err(Error::unavailable(Source::Qq, Reason::NeedLogin));
        }
        let body = json!({
            "comm": {
                "format": "json",
                "ct": 24,
                "cv": 0,
                "tmeAppID": "qqmusic",
                "uin": uin,
                "qq": uin,
                "authst": authst,
            },
            "req": {
                "module": "music.musicasset.PlaylistBaseRead",
                "method": "GetPlaylistByUin",
                "param": { "uin": uin, "sin": 0, "size": 200 },
            }
        });
        let decoded = self.musicu(&body).await?;
        let code = decoded
            .pointer("/req/code")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        // 80030 = 未登录/没拿到账号信息
        if code == 80030 || code == 1000 {
            return Err(Error::unavailable_with(
                Source::Qq,
                Reason::NeedLogin,
                format!("code={code}"),
            ));
        }
        let list = decoded
            .pointer("/req/data/v_playlist")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(list
            .iter()
            .filter_map(|item| {
                let id = text_of(item, &["dissid", "dirId", "tid"])?;
                let name = text_of(item, &["dissname", "dirName", "title"]).unwrap_or_default();
                if name.is_empty() {
                    return None;
                }
                Some(PlaylistSummary {
                    source: Source::Qq,
                    id,
                    name,
                    cover: text_of(item, &["logo", "picurl", "cover"]),
                    track_count: item
                        .get("song_cnt")
                        .or_else(|| item.get("songnum"))
                        .and_then(|v| v.as_u64())
                        .or_else(|| {
                            item.get("song_cnt")
                                .and_then(|v| v.as_str())
                                .and_then(|text| text.parse().ok())
                        })
                        .unwrap_or(0),
                    creator: text_of(item, &["nickname", "nick", "creator"]),
                })
            })
            .collect())
    }
}

/// 从 JSON 对象里按候选字段名取字符串（数字也接受），QQ 各接口字段名不统一。
fn text_of(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        let Some(found) = value.get(*key) else {
            continue;
        };
        let text = match found {
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Number(number) => number.to_string(),
            _ => continue,
        };
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

struct MidUrl {
    url: String,
    result: i64,
    ekey: Option<String>,
    tips: String,
    need_buy: i64,
}

struct VkeyBatch {
    code: i64,
    items: Vec<MidUrl>,
}

fn request_error(code: i64, logged_in: bool) -> Error {
    let reason = match code {
        104003 | 104009 => Reason::NeedLogin,
        104004 => Reason::Copyright,
        _ if !logged_in => Reason::NeedLogin,
        _ => Reason::Unknown,
    };
    Error::unavailable_with(Source::Qq, reason, format!("code={code}"))
}

fn reason_of(item: &MidUrl, logged_in: bool) -> Reason {
    if item.need_buy != 0 || !item.tips.is_empty() {
        return Reason::VipOnly;
    }
    match item.result {
        104003 | 104009 => Reason::NeedLogin,
        104004 => Reason::Copyright,
        _ if !logged_in => Reason::NeedLogin,
        _ => Reason::Unknown,
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
    sizeape: u64,
    /// 媒体文件 mid（取下载链接用，区别于 songmid）
    #[serde(default)]
    media_mid: Option<String>,
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

#[derive(Debug, Deserialize, Default)]
struct RawLyric {
    #[serde(default)]
    retcode: Option<i64>,
    #[serde(default)]
    lyric: Option<String>,
    #[serde(default)]
    trans: Option<String>,
}

/// `fcg_ucc_getcdinfo_byids_cp` 的歌单详情。
#[derive(Debug, Deserialize)]
struct RawCdList {
    #[serde(default)]
    cdlist: Option<Vec<RawCd>>,
}

#[derive(Debug, Deserialize)]
struct RawCd {
    #[serde(default)]
    dissname: String,
    #[serde(default)]
    logo: Option<String>,
    #[serde(default)]
    songlist: Option<Vec<RawSong>>,
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
        // 取链接用的 media_mid。**缺失时不要用 songmid 顶替**：
        // 两者的含义不同，顶替会让无损档一路 101404；留 None 交给 resolve
        // 去歌曲详情里补真实值。
        variant.media_mid = self.media_mid.filter(|mid| !mid.is_empty());
        if self.sizeflac > 0 {
            variant.bitrate = 999;
            variant.format = "flac".into();
            variant.size = self.sizeflac;
        } else if self.sizeape > 0 {
            variant.bitrate = 999;
            variant.format = "ape".into();
            variant.size = self.sizeape;
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

    #[test]
    fn parses_playlist_songlist_shape() {
        // 歌单详情接口的 songlist 与搜索结果同构，但通常不带 media_mid，
        // resolve 时会再去歌曲详情里补一次，所以这里只校验能解析出基础字段。
        let json = r#"{"cdlist":[{"dissname":"周杰伦全收录","logo":"https://y.qq.com/x.jpg",
            "songlist":[{"songmid":"0039MnYb0qxYhV","songname":"晴天","interval":269,
            "albumname":"叶惠美","albummid":"000MkMni19ClKG",
            "singer":[{"name":"周杰伦"}],"sizeflac":55397039}]}]}"#;
        let raw: RawCdList = serde_json::from_str(json).unwrap();
        let detail = raw.cdlist.unwrap().remove(0);
        assert_eq!(detail.dissname, "周杰伦全收录");
        let songlist = detail.songlist.unwrap();
        assert_eq!(songlist.len(), 1);
        let variant = songlist.into_iter().next().unwrap().into_variant();
        assert_eq!(variant.id, "0039MnYb0qxYhV");
        assert_eq!(variant.title, "晴天");
        // 歌单接口不给 media_mid，这里必须留空——留空才会让 resolve 去详情接口补真实值；
        // 用 songmid 顶替会让无损档一路 101404
        assert_eq!(variant.media_mid, None);
    }

    #[test]
    fn reads_playlist_fields_from_loose_json() {
        // GetPlaylistByUin 的字段名在不同版本里变过，text_of 要能按候选名兜住
        let item = serde_json::json!({
            "dissid": 7108054068i64,
            "dissname": "默认收藏",
            "logo": "https://y.qq.com/a.jpg",
            "song_cnt": 87,
            "nickname": "Luna",
        });
        assert_eq!(text_of(&item, &["dissid"]).as_deref(), Some("7108054068"));
        assert_eq!(
            text_of(&item, &["missing", "dissname"]).as_deref(),
            Some("默认收藏")
        );
        assert_eq!(text_of(&item, &["nope"]), None);
    }
}
