use std::collections::HashMap;

use async_trait::async_trait;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::model::{Fee, Lyric, PlayUrl, PlaylistDetail, SearchPage, Source, Variant};
use crate::net::Net;
use crate::platform::{CredStore, MusicPlatform, SearchQuery};

const ORIGIN: &str = "https://music.163.com";

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
}

#[async_trait]
impl MusicPlatform for Netease {
    fn source(&self) -> Source {
        Source::Netease
    }

    async fn search(&self, query: &SearchQuery) -> Result<SearchPage> {
        let keyword = utf8_percent_encode(&query.keyword, NON_ALPHANUMERIC).to_string();
        let url = format!(
            "{ORIGIN}/api/search/get/web?s={keyword}&type=1&limit={}&offset={}",
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

    async fn resolve(&self, variant: &Variant, bitrate: u32) -> Result<PlayUrl> {
        let br = (bitrate.clamp(64, 999) as u64) * 1000;
        let url = format!(
            "{ORIGIN}/api/song/enhance/player/url?ids=%5B{}%5D&br={br}",
            variant.id
        );
        let cookie = self.cookie_header();
        let mut headers = self.headers();
        if let Some(ref c) = cookie {
            headers.push(("Cookie", c.as_str()));
        }
        let raw: RawSongUrl = self.net.json(&url, &headers).await?;
        let item = raw.data.into_iter().next().ok_or(Error::NoPlayableUrl)?;
        let link = item
            .url
            .filter(|value| !value.is_empty())
            .ok_or(Error::NoPlayableUrl)?;
        Ok(PlayUrl {
            url: link,
            bitrate: item.br.unwrap_or(br).max(1) as u32 / 1000,
            format: item.kind.unwrap_or_else(|| "mp3".into()),
            size: item.size.unwrap_or(0.0) as u64,
        })
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
        let raw: RawPlaylist = self.net.json(&url, &self.headers()).await?;
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
