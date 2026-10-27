use serde::{Deserialize, Serialize};

/// 已接入的音源。新增音源时在这里加一个变体，并在 `platforms` 下实现
/// [`crate::platform::MusicPlatform`] 即可，聚合层与下载层无需改动。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Netease,
    Qq,
    Kuwo,
}

impl Source {
    pub const ALL: [Source; 3] = [Source::Netease, Source::Qq, Source::Kuwo];

    pub fn id(self) -> &'static str {
        match self {
            Source::Netease => "netease",
            Source::Qq => "qq",
            Source::Kuwo => "kuwo",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Source::Netease => "网易云音乐",
            Source::Qq => "QQ 音乐",
            Source::Kuwo => "酷我音乐",
        }
    }

    /// 界面上的品牌色，前端直接用。
    pub fn accent(self) -> &'static str {
        match self {
            Source::Netease => "#E23B3B",
            Source::Qq => "#2BAE67",
            Source::Kuwo => "#F5A623",
        }
    }

    pub fn from_id(raw: &str) -> Option<Self> {
        // 兼容前端传来的枚举名（如 "netease"/"Netease"）
        let lowered = raw.trim().to_ascii_lowercase();
        match lowered.as_str() {
            "netease" | "163" | "wyy" => Some(Source::Netease),
            "qq" | "qqmusic" => Some(Source::Qq),
            "kuwo" => Some(Source::Kuwo),
            _ => None,
        }
    }
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

/// 版权状态，仅用于界面提示，不参与是否可下载的判定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fee {
    Free,
    Vip,
    Paid,
    Unknown,
}

impl Fee {
    pub fn label(self) -> &'static str {
        match self {
            Fee::Free => "免费",
            Fee::Vip => "会员",
            Fee::Paid => "付费",
            Fee::Unknown => "未知",
        }
    }
}

/// 平台自己给出的「原唱 / 翻唱」判定。
///
/// 目前只有网易云在搜索结果里带 `originCoverType`（1 = 原唱，2 = 翻唱），
/// QQ 与酷我没有对应字段。拿不到就不填，前端再退回标题文本判断。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Original,
    Cover,
}

/// 某个音源上的一条具体记录，是「能被下载的最小单位」。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variant {
    pub source: Source,
    pub id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub duration_ms: u64,
    pub cover: Option<String>,
    pub bitrate: u32,
    pub format: String,
    pub size: u64,
    pub fee: Fee,
    /// 平台标注的原唱/翻唱；平台没给就是 None（绝大多数音源都是 None）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
    /// 媒体文件 mid（QQ 音乐取链接用，区别于歌曲 mid）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_mid: Option<String>,
    /// 平台给的热度（网易云 `pop` 为 0–100，其它音源可能没有，缺省 0）。
    /// 只用于「按热度排序」，不参与任何业务判断。
    #[serde(default)]
    pub popularity: u32,
}

impl Variant {
    pub fn new(source: Source, id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            source,
            id: id.into(),
            title: title.into(),
            artists: Vec::new(),
            album: String::new(),
            duration_ms: 0,
            cover: None,
            bitrate: 0,
            format: "mp3".into(),
            size: 0,
            fee: Fee::Unknown,
            origin: None,
            media_mid: None,
            popularity: 0,
        }
    }

    pub fn artist_line(&self) -> String {
        self.artists.join(" / ")
    }
}

/// 聚合后的一首歌：同一首歌在各音源上的记录被合并成一条，
/// 界面上一行展示，下载时再决定取哪条链接。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub fingerprint: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub duration_ms: u64,
    pub cover: Option<String>,
    pub bitrate: u32,
    pub format: String,
    pub fee: Fee,
    pub variants: Vec<Variant>,
    /// 各音源热度里的最大值，仅用于「按热度排序」
    #[serde(default)]
    pub popularity: u32,
}

impl Track {
    pub fn from_variant(v: Variant) -> Self {
        let fingerprint = fingerprint(&v.title, &v.artists, v.duration_ms);
        Self {
            fingerprint,
            title: v.title.clone(),
            artists: v.artists.clone(),
            album: v.album.clone(),
            duration_ms: v.duration_ms,
            cover: v.cover.clone(),
            bitrate: v.bitrate,
            format: v.format.clone(),
            fee: v.fee,
            popularity: v.popularity,
            variants: vec![v],
        }
    }

    /// 把一个同源/异源的同曲记录并入当前条目，同时刷新最优展示字段。
    pub fn absorb(&mut self, v: Variant) {
        if self.cover.is_none() {
            self.cover = v.cover.clone();
        }
        if self.album.is_empty() {
            self.album = v.album.clone();
        }
        if self.duration_ms == 0 {
            self.duration_ms = v.duration_ms;
        }
        if v.bitrate > self.bitrate {
            self.bitrate = v.bitrate;
            self.format = v.format.clone();
        }
        self.fee = merge_fee(self.fee, v.fee);
        // 热度取各源最大值：只要有一个源给得出人气，这条就该参与热度排序
        self.popularity = self.popularity.max(v.popularity);
        // 同一音源重复出现时保留音质更高的那条
        if let Some(existing) = self
            .variants
            .iter_mut()
            .find(|item| item.source == v.source)
        {
            if v.bitrate > existing.bitrate {
                *existing = v;
            }
            return;
        }
        self.variants.push(v);
    }

    pub fn artist_line(&self) -> String {
        self.artists.join(" / ")
    }

    pub fn sources(&self) -> Vec<Source> {
        self.variants.iter().map(|v| v.source).collect()
    }

    pub fn has_source(&self, source: Source) -> bool {
        self.variants.iter().any(|v| v.source == source)
    }

    /// 下载时优先选：指定音源 > 免费 > 音质高。
    pub fn best_variant(&self, prefer: Option<Source>) -> Option<&Variant> {
        let mut candidates: Vec<&Variant> = self.variants.iter().collect();
        candidates.sort_by_key(|v| {
            let source_rank = if Some(v.source) == prefer { 0 } else { 1 };
            let fee_rank = match v.fee {
                Fee::Free => 0,
                Fee::Unknown => 1,
                Fee::Vip => 2,
                Fee::Paid => 3,
            };
            (source_rank, fee_rank, std::cmp::Reverse(v.bitrate))
        });
        candidates.into_iter().next()
    }

    pub fn variant_of(&self, source: Source) -> Option<&Variant> {
        self.variants.iter().find(|v| v.source == source)
    }
}

fn merge_fee(a: Fee, b: Fee) -> Fee {
    fn rank(f: Fee) -> u8 {
        match f {
            Fee::Free => 0,
            Fee::Vip => 1,
            Fee::Paid => 2,
            Fee::Unknown => 3,
        }
    }
    if rank(b) < rank(a) {
        b
    } else {
        a
    }
}

/// 可直接交给播放器 / 下载器的链接。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayUrl {
    pub url: String,
    pub bitrate: u32,
    pub format: String,
    pub size: u64,
    /// 加密格式（.mflac/.mgg）的解密密钥；明文格式为 None。
    /// 下载器拿到非空 ekey 时，写入文件前需流式 QMCv2 解密。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ekey: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub source: Source,
    pub page: u32,
    pub total: u64,
    pub has_more: bool,
    pub items: Vec<Variant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDetail {
    pub source: Source,
    pub id: String,
    pub name: String,
    pub cover: Option<String>,
    pub total: u64,
    pub items: Vec<Variant>,
}

/// 「我的歌单」列表项。只是一个摘要，点进去再用 [`PlaylistDetail`] 拉曲目。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistSummary {
    pub source: Source,
    pub id: String,
    pub name: String,
    pub cover: Option<String>,
    pub track_count: u64,
    /// 创建者昵称：收藏别人的歌单时会显示原作者
    pub creator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lyric {
    /// 原始 LRC 文本
    pub lrc: String,
    /// 翻译歌词，可能为空
    pub translation: Option<String>,
}

/// 单个音源在一次请求中的表现，用于界面上展示「哪个源挂了」。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceReport {
    pub source: Source,
    pub ok: bool,
    pub count: usize,
    pub elapsed_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregatedPage {
    pub keyword: String,
    pub page: u32,
    pub tracks: Vec<Track>,
    pub reports: Vec<SourceReport>,
}

/// 归一化标题：去掉括号内的版本信息、空白与常见修饰，用于跨源判重。
pub fn normalize_title(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut depth = 0i32;
    for ch in raw.chars() {
        match ch {
            '(' | '（' | '[' | '【' | '{' => depth += 1,
            ')' | '）' | ']' | '】' | '}' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            _ if depth > 0 => {}
            _ => {
                let lower = ch.to_ascii_lowercase();
                if lower.is_whitespace() {
                    continue;
                }
                if matches!(
                    lower,
                    '-' | '_'
                        | '.'
                        | '\''
                        | '"'
                        | ','
                        | '，'
                        | '、'
                        | '·'
                        | '!'
                        | '！'
                        | '?'
                        | '？'
                        | ':'
                        | '：'
                        | ';'
                        | '；'
                        | '/'
                        | '\\'
                        | '+'
                        | '&'
                        | '《'
                        | '》'
                        | '〈'
                        | '〉'
                        | '「'
                        | '」'
                        | '『'
                        | '』'
                ) {
                    continue;
                }
                for c in lower.to_string().chars() {
                    out.push(c);
                }
            }
        }
    }
    out
}

/// 判重指纹：标题 + 首位歌手 + 时长（按 3 秒取整，容忍各源元数据偏差）。
pub fn fingerprint(title: &str, artists: &[String], duration_ms: u64) -> String {
    let artist = artists
        .first()
        .map(|a| normalize_title(a))
        .unwrap_or_default();
    // 时长按 3 秒就近分桶：各源元数据常有 1~2 秒偏差，取整方式直接影响能否合并
    format!(
        "{}|{}|{}",
        normalize_title(title),
        artist,
        (duration_ms + 1_500) / 3_000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_bracket_suffix_and_punct() {
        assert_eq!(normalize_title("晴天 (Live)"), "晴天");
        assert_eq!(normalize_title("《七里香》"), "七里香");
        assert_eq!(normalize_title("Hello, World!"), "helloworld");
    }

    #[test]
    fn fingerprint_ignores_duration_jitter() {
        let a = fingerprint("晴天", &["周杰伦".into()], 269_000);
        let b = fingerprint("晴天", &["周杰伦".into()], 270_500);
        assert_eq!(a, b);
        let c = fingerprint("晴天", &["周杰伦".into()], 300_000);
        assert_ne!(a, c);
    }

    #[test]
    fn absorb_prefers_higher_bitrate_per_source() {
        let mut low = Variant::new(Source::Qq, "1", "晴天");
        low.bitrate = 128;
        low.fee = Fee::Vip;
        let mut track = Track::from_variant(low);

        let mut high = Variant::new(Source::Qq, "1", "晴天");
        high.bitrate = 320;
        high.fee = Fee::Free;
        track.absorb(high);

        assert_eq!(track.variants.len(), 1);
        assert_eq!(track.bitrate, 320);
        assert_eq!(track.fee, Fee::Free);
    }

    #[test]
    fn best_variant_respects_preference() {
        let mut a = Variant::new(Source::Netease, "1", "x");
        a.bitrate = 320;
        let mut b = Variant::new(Source::Qq, "2", "x");
        b.bitrate = 128;
        let mut track = Track::from_variant(a);
        track.absorb(b);
        assert_eq!(
            track.best_variant(Some(Source::Qq)).map(|v| v.source),
            Some(Source::Qq)
        );
        assert_eq!(
            track.best_variant(None).map(|v| v.source),
            Some(Source::Netease)
        );
    }
}
