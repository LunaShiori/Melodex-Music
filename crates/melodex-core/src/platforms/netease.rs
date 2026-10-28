use std::collections::HashMap;

use async_trait::async_trait;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;

use crate::error::{Error, Reason, Result};
use crate::model::{
    Fee, Lyric, Origin, PlayUrl, PlaylistDetail, PlaylistSummary, SearchPage, Source, Variant,
};
use crate::net::{Net, ProxyFor};
use crate::platform::{CredStore, MusicPlatform, SearchQuery};
use crate::quality::{Quality, QualityLevel};

const ORIGIN: &str = "https://music.163.com";
/// 播放直链的附加请求头（本地代理用）。
const STREAM_HEADERS: &[(&str, &str)] = &[("Referer", ORIGIN)];

/// 网易云音乐：搜索 / 播放链接 / 歌词 / 歌单全部走公开 Web 接口。
pub struct Netease {
    net: Net,
    creds: CredStore,
}

impl Netease {
    pub fn new(net: Net, creds: CredStore) -> Self {
        Self { net, creds }
    }

    fn headers(&self) -> Vec<(&'static str, &str)> {
        vec![("Referer", ORIGIN), ("Origin", ORIGIN)]
    }

    fn cookie_header(&self) -> Option<String> {
        self.creds.cookie(Source::Netease)
    }

    /// 搜索接口只返回 picId，封面要再批量取一次详情（一次请求覆盖整页）。
    async fn covers(&self, ids: &[i64]) -> Result<HashMap<i64, (Option<String>, Option<String>)>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let joined = ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let url = format!("{ORIGIN}/api/song/detail?ids=%5B{joined}%5D");
        let raw: RawDetail = self.net.json(&url, &self.headers()).await?;
        let mut map = HashMap::new();
        for song in raw.songs.unwrap_or_default() {
            let album = song.album.unwrap_or_default();
            map.insert(song.id, (album.pic_url, album.name));
        }
        Ok(map)
    }

    /// 取指定档位的播放链接；服务端没给链接时返回 `Ok(None)`，交由调用方降级。
    async fn fetch_url(
        &self,
        id: &str,
        quality: &Quality,
        cookie: Option<&str>,
    ) -> Result<Option<PlayUrl>> {
        // encodeType 必须跟档位一致：固定写 mp3 会把无损档转成有损再返回
        let url = format!(
            "{ORIGIN}/api/song/enhance/player/url/v1?ids=%5B{id}%5D&level={}&encodeType={}",
            quality.code, quality.format
        );
        let mut headers = self.headers();
        if let Some(cookie) = cookie {
            headers.push(("Cookie", cookie));
        }
        let raw: RawSongUrl = self.net.json_for(ProxyFor::Link, &url, &headers).await?;
        let Some(item) = raw.data.into_iter().next() else {
            return Ok(None);
        };
        let Some(link) = item.url.filter(|value| !value.is_empty()) else {
            return Ok(None);
        };
        let br = item.br.unwrap_or(0);
        let kbps = if br > 0 {
            (br / 1000) as u32
        } else {
            quality.bitrate
        };
        // 没有会员时服务端不一定报错，而是静默回一个低码率的替身文件。
        // 无损及以上必须是 700kbps 以上，否则当作「这档拿不到」继续降级。
        if quality.level >= QualityLevel::Lossless && kbps < 700 {
            return Ok(None);
        }
        Ok(Some(PlayUrl {
            url: link,
            bitrate: kbps.min(999),
            format: item.kind.unwrap_or_else(|| quality.format.into()),
            size: item.size.unwrap_or(0.0) as u64,
            ekey: None,
        }))
    }

    /// 拿不到链接时，回头问一次这首歌的「权限档案」，判断到底是没登录、
    /// 缺会员、还是这首歌本身没有版权。
    ///
    /// `privileges[0]` 里几个字段的含义：
    /// - `st = -200`：无版权 / 已下架
    /// - `dlLevel = "none"`：当前账号没有下载权限（未登录时恒为 none）
    /// - `fee`：0 免费、1 会员、4 需付费、8 免费试听
    async fn diagnose(&self, id: &str) -> Option<Reason> {
        let url = format!("{ORIGIN}/api/v3/song/detail?c=%5B%7B%22id%22%3A{id}%7D%5D");
        let cookie = self.cookie_header();
        let mut headers = self.headers();
        if let Some(ref c) = cookie {
            headers.push(("Cookie", c.as_str()));
        }
        let raw: RawPrivilegeDetail = self.net.json(&url, &headers).await.ok()?;
        let fee = raw
            .songs
            .as_ref()
            .and_then(|list| list.first())
            .map(|song| song.fee)
            .unwrap_or(0);
        let privilege = raw.privileges.and_then(|list| list.into_iter().next());
        let st = privilege.as_ref().map(|p| p.st).unwrap_or(0);
        let dl_level = privilege
            .as_ref()
            .and_then(|p| p.dl_level.clone().or(p.download_max_level.clone()))
            .unwrap_or_default();
        // 未登录时 dlLevel 恒为 none、st 常为 -200，两个信号都不可信，
        // 只能给出「需要登录」这个结论——这也正是登录后才有意义去区分版权/会员。
        let logged_in = cookie
            .as_deref()
            .map(|value| crate::cookie::value_of(value, "MUSIC_U").is_some())
            .unwrap_or(false);

        Some(if !logged_in {
            Reason::NeedLogin
        } else if st == -200 {
            Reason::Copyright
        } else if fee == 1 || fee == 4 || dl_level == "none" {
            Reason::VipOnly
        } else {
            Reason::QualityMissing
        })
    }
}

#[async_trait]
impl MusicPlatform for Netease {
    fn source(&self) -> Source {
        Source::Netease
    }

    fn stream_headers(&self) -> &'static [(&'static str, &'static str)] {
        STREAM_HEADERS
    }

    async fn search(&self, query: &SearchQuery) -> Result<SearchPage> {
        let keyword = utf8_percent_encode(&query.keyword, NON_ALPHANUMERIC).to_string();
        // cloudsearch/pc 比老的 search/get/web 返回更完整（含 dt 时长、更准的 songCount），
        // 且登录态（MUSIC_U）下原唱/热门排序更贴近 Web 端。
        let url = format!(
            "{ORIGIN}/api/cloudsearch/pc?s={keyword}&type=1&limit={}&offset={}&total=true",
            query.limit,
            query.offset()
        );
        let cookie = self.cookie_header();
        let mut headers = self.headers();
        if let Some(ref c) = cookie {
            headers.push(("Cookie", c.as_str()));
        }
        let raw: RawSearch = self.net.json(&url, &headers).await?;
        let result = raw
            .result
            .ok_or_else(|| Error::decode("网易云搜索返回空 result"))?;
        let total = result.song_count.unwrap_or(0);
        let songs = result.songs.unwrap_or_default();

        let ids: Vec<i64> = songs.iter().map(|s| s.id).collect();
        let covers = self.covers(&ids).await.unwrap_or_default();

        let items: Vec<Variant> = songs
            .into_iter()
            .enumerate()
            .map(|(index, song)| {
                let mut variant = song.into_variant();
                if let Some((pic, album)) = covers.get(&ids[index]) {
                    if variant.cover.is_none() {
                        variant.cover = pic.clone();
                    }
                    if variant.album.is_empty() {
                        if let Some(name) = album {
                            variant.album = name.clone();
                        }
                    }
                }
                variant
            })
            .collect();

        let has_more = (query.offset() as u64 + items.len() as u64) < total;
        Ok(SearchPage {
            source: Source::Netease,
            page: query.page,
            total,
            has_more,
            items,
        })
    }

    async fn resolve(&self, variant: &Variant, quality: &'static Quality) -> Result<PlayUrl> {
        let cookie = self.cookie_header();
        if let Some(play) = self
            .fetch_url(&variant.id, quality, cookie.as_deref())
            .await?
        {
            return Ok(play);
        }
        // 这一档拿不到，回头查权限档案，给出「要登录」还是「要会员」的准确原因。
        // 是否继续降级由引擎决定，这里只负责把原因说清楚。
        let reason = self.diagnose(&variant.id).await.unwrap_or(Reason::Unknown);
        Err(Error::unavailable(Source::Netease, reason))
    }

    async fn lyric(&self, id: &str) -> Result<Option<Lyric>> {
        let url = format!("{ORIGIN}/api/song/lyric?id={id}&lv=1&kv=1&tv=-1");
        let raw: RawLyric = self.net.json(&url, &self.headers()).await?;
        let translation = raw.tlyric.and_then(|t| {
            if t.lyric.trim().is_empty() {
                None
            } else {
                Some(t.lyric)
            }
        });
        Ok(raw.lrc.map(|l| Lyric {
            lrc: l.lyric,
            translation,
        }))
    }

    async fn playlist(&self, id: &str) -> Result<Option<PlaylistDetail>> {
        let url = format!("{ORIGIN}/api/v6/playlist/detail?id={id}");
        // 自己的私密 / 收藏歌单要带登录态才给 tracks，匿名只有公开歌单能读。
        let cookie = self.cookie_header().unwrap_or_default();
        let mut headers = self.headers();
        if !cookie.is_empty() {
            headers.push(("Cookie", cookie.as_str()));
        }
        let raw: RawPlaylist = self.net.json(&url, &headers).await?;
        let Some(playlist) = raw.playlist else {
            return Ok(None);
        };
        let items: Vec<Variant> = playlist
            .tracks
            .unwrap_or_default()
            .into_iter()
            .map(|song| song.into_variant())
            .collect();
        Ok(Some(PlaylistDetail {
            source: Source::Netease,
            id: id.to_string(),
            name: playlist.name,
            cover: playlist.cover_img_url,
            total: playlist.track_count.unwrap_or(items.len() as u64),
            items,
        }))
    }

    async fn my_playlists(&self) -> Result<Vec<PlaylistSummary>> {
        let Some(cookie) = self.cookie_header() else {
            return Err(Error::unavailable(Source::Netease, Reason::NeedLogin));
        };
        let mut headers = self.headers();
        headers.push(("Cookie", cookie.as_str()));

        // 账号接口能直接给出 profile.userId，省去从 Cookie 里猜 uid 的麻烦。
        let account: RawAccount = self
            .net
            .json(&format!("{ORIGIN}/api/nuser/account/get"), &headers)
            .await?;
        let uid = account
            .profile
            .and_then(|profile| profile.user_id)
            .ok_or_else(|| Error::unavailable(Source::Netease, Reason::NeedLogin))?;

        let url = format!("{ORIGIN}/api/user/playlist?uid={uid}&limit=200&offset=0");
        let raw: RawUserPlaylists = self.net.json(&url, &headers).await?;
        Ok(raw
            .playlist
            .unwrap_or_default()
            .into_iter()
            .filter(|item| !item.id.is_empty() && !item.name.is_empty())
            .map(|item| PlaylistSummary {
                source: Source::Netease,
                id: item.id,
                name: item.name,
                cover: item.cover_img_url,
                track_count: item.track_count.unwrap_or(0),
                creator: item.creator.map(|creator| creator.nickname),
            })
            .collect())
    }
}

#[derive(Debug, Deserialize)]
struct RawSearch {
    result: Option<RawResult>,
}

#[derive(Debug, Deserialize)]
struct RawResult {
    #[serde(default)]
    songs: Option<Vec<RawSong>>,
    #[serde(default, rename = "songCount")]
    song_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RawSong {
    id: i64,
    #[serde(default)]
    name: String,
    #[serde(default, alias = "ar")]
    artists: Vec<RawArtist>,
    #[serde(default, alias = "al")]
    album: Option<RawAlbum>,
    #[serde(default, alias = "dt")]
    duration: u64,
    #[serde(default)]
    fee: i32,
    /// 热度，0–100 的浮点数；`cloudsearch` 叫 `pop`，老接口叫 `popularity`
    #[serde(default, alias = "popularity")]
    pop: f64,
    /// 1 = 原唱，2 = 翻唱，0/3 = 平台自己也没标（实测 3 里混着翻唱，不能当原唱）
    #[serde(default, rename = "originCoverType")]
    origin_cover_type: i32,
}

impl RawSong {
    fn into_variant(self) -> Variant {
        let album = self.album.unwrap_or_default();
        let mut variant = Variant::new(Source::Netease, self.id.to_string(), self.name);
        variant.artists = self.artists.into_iter().map(|a| a.name).collect();
        variant.album = album.name.unwrap_or_default();
        variant.cover = album.pic_url;
        variant.duration_ms = self.duration;
        variant.fee = fee_of(self.fee);
        variant.bitrate = 0;
        variant.size = 0;
        variant.format = "mp3".into();
        variant.popularity = self.pop.clamp(0.0, 1000.0) as u32;
        variant.origin = match self.origin_cover_type {
            1 => Some(Origin::Original),
            2 => Some(Origin::Cover),
            _ => None,
        };
        variant
    }
}

#[derive(Debug, Deserialize)]
struct RawArtist {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Default, Deserialize)]
struct RawAlbum {
    #[serde(default)]
    name: Option<String>,
    #[serde(default, rename = "picUrl")]
    pic_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawDetail {
    #[serde(default)]
    songs: Option<Vec<RawDetailSong>>,
}

#[derive(Debug, Deserialize)]
struct RawDetailSong {
    id: i64,
    #[serde(default)]
    album: Option<RawAlbum>,
}

#[derive(Debug, Deserialize)]
struct RawSongUrl {
    #[serde(default)]
    data: Vec<RawSongUrlItem>,
}

#[derive(Debug, Deserialize)]
struct RawSongUrlItem {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    br: Option<u64>,
    #[serde(default)]
    size: Option<f64>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

/// `/api/v3/song/detail` 的响应：`songs` 给版权状态，`privileges` 给当前账号的权限档位。
#[derive(Debug, Deserialize)]
struct RawPrivilegeDetail {
    #[serde(default)]
    songs: Option<Vec<RawPrivilegeSong>>,
    #[serde(default)]
    privileges: Option<Vec<RawPrivilege>>,
}

#[derive(Debug, Deserialize)]
struct RawPrivilegeSong {
    #[serde(default)]
    fee: i32,
}

#[derive(Debug, Deserialize)]
struct RawPrivilege {
    /// 歌曲状态，-200 表示无版权
    #[serde(default)]
    st: i32,
    /// 当前账号可下载的最高档位（none / standard / higher / exhigh / lossless）
    #[serde(default, rename = "dlLevel")]
    dl_level: Option<String>,
    #[serde(default, rename = "downloadMaxBrLevel")]
    download_max_level: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawLyric {
    #[serde(default)]
    lrc: Option<RawLyricBody>,
    #[serde(default)]
    tlyric: Option<RawLyricBody>,
}

#[derive(Debug, Deserialize)]
struct RawLyricBody {
    #[serde(default)]
    lyric: String,
}

#[derive(Debug, Deserialize)]
struct RawPlaylist {
    #[serde(default)]
    playlist: Option<RawPlaylistBody>,
}

#[derive(Debug, Deserialize)]
struct RawPlaylistBody {
    #[serde(default)]
    name: String,
    #[serde(default, rename = "coverImgUrl")]
    cover_img_url: Option<String>,
    #[serde(default, rename = "trackCount")]
    track_count: Option<u64>,
    #[serde(default)]
    tracks: Option<Vec<RawSong>>,
}

/// `/api/nuser/account/get`：确认登录态并拿到 uid。
#[derive(Debug, Deserialize)]
struct RawAccount {
    #[serde(default)]
    profile: Option<RawProfile>,
}

#[derive(Debug, Deserialize)]
struct RawProfile {
    #[serde(default, rename = "userId")]
    user_id: Option<u64>,
}

/// `/api/user/playlist`：当前账号的自建 + 收藏歌单。
#[derive(Debug, Deserialize)]
struct RawUserPlaylists {
    #[serde(default)]
    playlist: Option<Vec<RawUserPlaylist>>,
}

#[derive(Debug, Deserialize)]
struct RawUserPlaylist {
    /// 歌单 id 在 JSON 里既可能是数字也可能被转成字符串，统一按字符串读
    #[serde(default, deserialize_with = "id_as_string")]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default, rename = "coverImgUrl")]
    cover_img_url: Option<String>,
    #[serde(default, rename = "trackCount")]
    track_count: Option<u64>,
    #[serde(default)]
    creator: Option<RawCreator>,
}

#[derive(Debug, Deserialize)]
struct RawCreator {
    #[serde(default)]
    nickname: String,
}

fn id_as_string<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::Number(number) => number.to_string(),
        serde_json::Value::String(text) => text,
        _ => String::new(),
    })
}

/// 网易云的 fee 位含义：0 免费、1 会员、4 付费专辑、8 低音质免费。
fn fee_of(fee: i32) -> Fee {
    match fee {
        0 => Fee::Free,
        1 => Fee::Vip,
        4 => Fee::Paid,
        8 => Fee::Free,
        _ => Fee::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_payload() {
        let json = r#"{"result":{"songs":[
            {"id":2652820720,"name":"晴天(深情版)","duration":278961,"fee":8,
             "album":{"name":"晴天(深情版)","picId":109951173569626660},
             "artists":[{"name":"Lucky小爱"}]}
        ],"songCount":3599}}"#;
        let raw: RawSearch = serde_json::from_str(json).unwrap();
        let result = raw.result.unwrap();
        assert_eq!(result.song_count, Some(3599));
        let variant = result.songs.unwrap().remove(0).into_variant();
        assert_eq!(variant.id, "2652820720");
        assert_eq!(variant.title, "晴天(深情版)");
        assert_eq!(variant.artists, vec!["Lucky小爱".to_string()]);
        assert_eq!(variant.duration_ms, 278_961);
        assert_eq!(variant.fee, Fee::Free);
    }

    #[test]
    fn parses_playlist_track_shape() {
        // 歌单接口用的是 ar / al / dt 这组字段
        let json = r#"{"playlist":{"name":"热歌榜","coverImgUrl":"https://p1.music.126.net/x.jpg",
            "trackCount":200,"tracks":[{"id":1,"name":"晴天","dt":269000,
            "ar":[{"name":"周杰伦"}],"al":{"name":"叶惠美","picUrl":"https://p1.music.126.net/y.jpg"}}]}}"#;
        let raw: RawPlaylist = serde_json::from_str(json).unwrap();
        let body = raw.playlist.unwrap();
        let variant = body.tracks.unwrap().remove(0).into_variant();
        assert_eq!(variant.title, "晴天");
        assert_eq!(variant.duration_ms, 269_000);
        assert_eq!(variant.album, "叶惠美");
        assert_eq!(
            variant.cover.as_deref(),
            Some("https://p1.music.126.net/y.jpg")
        );
    }

    #[test]
    fn fee_mapping() {
        assert_eq!(fee_of(1), Fee::Vip);
        assert_eq!(fee_of(4), Fee::Paid);
        assert_eq!(fee_of(8), Fee::Free);
        assert_eq!(fee_of(99), Fee::Unknown);
    }
}
