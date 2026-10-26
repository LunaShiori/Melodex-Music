//! 音质档位：每个平台一套自己的名字、格式与上限。
//!
//! 为什么不统一成一个 bitrate：QQ 音乐的「臻品母带」、网易云的「超清母带」
//! 和酷我的「320k」并不是同一件事，用裸数字对齐会把不同平台的档位硬凑成一样，
//! 界面上就会出现「用户选了 QQ 的无损，换到酷我后显示的也是无损，实际是 128k」
//! 这种错位。所以每个平台维护一张自己的表，跨平台只对齐「等级」这一层抽象。
//!
//! [`QualityLevel`] 用于跨源兜底（同一首歌换源时按等级取该平台上不高于该等级
//! 的最高档），[`Quality`] 则是给用户看的真实档位，名称照抄平台自己的叫法。

use serde::{Deserialize, Serialize};

use crate::model::Source;

/// 跨平台对齐用的等级。只表达「谁比谁高」，不承诺各平台同等级音质相同。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QualityLevel {
    /// 标准 / 流畅
    #[default]
    Standard,
    /// 较高（192k 左右）
    High,
    /// 极高 / HQ（320k）
    Exquisite,
    /// 无损（CD 级 FLAC）
    Lossless,
    /// Hi-Res（24bit / 96kHz 及以上）
    HiRes,
    /// 母带级（平台自家的顶配）
    Master,
}

impl QualityLevel {
    pub const ALL: [QualityLevel; 6] = [
        QualityLevel::Standard,
        QualityLevel::High,
        QualityLevel::Exquisite,
        QualityLevel::Lossless,
        QualityLevel::HiRes,
        QualityLevel::Master,
    ];

    pub fn id(self) -> &'static str {
        match self {
            QualityLevel::Standard => "standard",
            QualityLevel::High => "high",
            QualityLevel::Exquisite => "exquisite",
            QualityLevel::Lossless => "lossless",
            QualityLevel::HiRes => "hiRes",
            QualityLevel::Master => "master",
        }
    }

    /// 通用叫法（不带平台色彩），用于「未登录 / 平台不支持」这类兜底文案。
    pub fn label(self) -> &'static str {
        match self {
            QualityLevel::Standard => "标准音质",
            QualityLevel::High => "较高音质",
            QualityLevel::Exquisite => "极高音质",
            QualityLevel::Lossless => "无损音质",
            QualityLevel::HiRes => "Hi-Res 无损",
            QualityLevel::Master => "母带级音质",
        }
    }

    /// 由前端传来的字符串解析。兼容旧设置里 `bitrate` 的数字语义。
    pub fn parse(raw: &str) -> Option<Self> {
        let lowered = raw.trim().to_ascii_lowercase();
        let found = match lowered.as_str() {
            "standard" | "basic" | "128" => QualityLevel::Standard,
            "high" | "higher" | "192" => QualityLevel::High,
            "exquisite" | "exhigh" | "hq" | "320" => QualityLevel::Exquisite,
            "lossless" | "sq" | "flac" | "999" | "1000" => QualityLevel::Lossless,
            "hires" | "hi-res" | "hi_res" | "2000" => QualityLevel::HiRes,
            "master" | "jymaster" | "3000" => QualityLevel::Master,
            _ => return None,
        };
        Some(found)
    }

    /// 旧版本按裸 bitrate 存音质，迁移时用它换算。
    pub fn from_bitrate(bitrate: u32) -> Self {
        match bitrate {
            b if b >= 2_000 => QualityLevel::HiRes,
            b if b >= 700 => QualityLevel::Lossless,
            b if b >= 300 => QualityLevel::Exquisite,
            b if b >= 192 => QualityLevel::High,
            _ => QualityLevel::Standard,
        }
    }
}

/// 一个平台上的具体档位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Quality {
    /// 平台内部档位码（QQ 的文件名前缀、网易的 level 参数），仅供平台解析器使用
    pub code: &'static str,
    /// 界面展示名，照抄平台自己的叫法
    pub label: &'static str,
    /// 副标题：格式与规格
    pub detail: &'static str,
    pub level: QualityLevel,
    /// 归一化码率，仅用于排序与「音质」列展示，无损按 1000 记
    pub bitrate: u32,
    pub format: &'static str,
    /// 是否通常需要会员
    pub vip: bool,
}

impl Quality {
    pub fn needs_login(&self) -> bool {
        self.vip
    }
}

pub const fn q(
    code: &'static str,
    label: &'static str,
    detail: &'static str,
    level: QualityLevel,
    bitrate: u32,
    format: &'static str,
    vip: bool,
) -> Quality {
    Quality {
        code,
        label,
        detail,
        level,
        bitrate,
        format,
        vip,
    }
}

/// 网易云音乐：档位名与接口 `level` 参数一一对应。
pub const NETEASE: &[Quality] = &[
    q(
        "jymaster",
        "超清母带",
        "FLAC 24bit/192kHz",
        QualityLevel::Master,
        3_000,
        "flac",
        true,
    ),
    q(
        "hires",
        "Hi-Res 无损",
        "FLAC 24bit/96kHz",
        QualityLevel::HiRes,
        2_000,
        "flac",
        true,
    ),
    q(
        "lossless",
        "无损音质",
        "FLAC 44.1kHz",
        QualityLevel::Lossless,
        1_000,
        "flac",
        true,
    ),
    q(
        "exhigh",
        "极高音质",
        "MP3 320kbps",
        QualityLevel::Exquisite,
        320,
        "mp3",
        false,
    ),
    q(
        "higher",
        "较高音质",
        "MP3 192kbps",
        QualityLevel::High,
        192,
        "mp3",
        false,
    ),
    q(
        "standard",
        "标准音质",
        "MP3 128kbps",
        QualityLevel::Standard,
        128,
        "mp3",
        false,
    ),
];

/// QQ 音乐：`code` 是 `CgiGetVkey` 的文件名前缀。
///
/// 高两档（`RS01` / `AI00`）走的是同一套 vkey 接口，只是文件名不同。
/// 服务端对不认识的档位会回 `result=101404`，引擎据此继续降级，
/// 所以这里可以放心把两档列出来——真正拿不到时会退到 SQ 而不是报错。
pub const QQ: &[Quality] = &[
    q(
        "AI00",
        "臻品母带",
        "FLAC 24bit/192kHz",
        QualityLevel::Master,
        3_000,
        "flac",
        true,
    ),
    q(
        "RS01",
        "Hi-Res 无损",
        "FLAC 24bit/96kHz",
        QualityLevel::HiRes,
        2_000,
        "flac",
        true,
    ),
    q(
        "F000",
        "SQ 无损",
        "FLAC 44.1kHz",
        QualityLevel::Lossless,
        1_000,
        "flac",
        true,
    ),
    q(
        "M800",
        "HQ 高品质",
        "MP3 320kbps",
        QualityLevel::Exquisite,
        320,
        "mp3",
        false,
    ),
    q(
        "M500",
        "标准音质",
        "MP3 128kbps",
        QualityLevel::Standard,
        128,
        "mp3",
        false,
    ),
];

/// 酷我音乐：公开的 `convert_url` 接口只稳定给 128k。
///
/// 实测传 `br=320kmp3` 或 `flac` 返回的都是同一个 128k mp3 文件，
/// 所以这里只列一档——列出来却下不到，比不列更糟。
pub const KUWO: &[Quality] = &[q(
    "128kmp3",
    "标准音质",
    "MP3 128kbps（该接口上限）",
    QualityLevel::Standard,
    128,
    "mp3",
    false,
)];

/// 某个平台的全部档位，从高到低。
pub fn table(source: Source) -> &'static [Quality] {
    match source {
        Source::Netease => NETEASE,
        Source::Qq => QQ,
        Source::Kuwo => KUWO,
    }
}

/// 该平台上不高于 `level` 的最高档。目标超出平台能力时自动落到它的上限。
pub fn find(source: Source, level: QualityLevel) -> Option<&'static Quality> {
    table(source).iter().find(|item| item.level <= level)
}

/// 降级链：从「不高于目标等级的最高档」开始，一路向下到最低档。
///
/// 引擎按这个顺序逐个向平台要链接，第一个成功的档位即最终音质。
pub fn ladder(source: Source, level: QualityLevel) -> Vec<&'static Quality> {
    let table = table(source);
    match table.iter().position(|item| item.level <= level) {
        Some(start) => table[start..].iter().collect(),
        // 目标等级低于该平台全部档位时，直接给它最低档
        None => table.last().map(|item| vec![item]).unwrap_or_default(),
    }
}

/// 该平台档位表里的最高一档，用于界面展示「最高可用」。
pub fn top(source: Source) -> Option<&'static Quality> {
    table(source).first()
}

/// 前端渲染音质选项用。顺序与 [`Source::ALL`] 一致，保证界面稳定。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformQuality {
    pub source: Source,
    pub label: &'static str,
    /// 平台级说明，比如「会员账号才能取到无损」
    pub note: &'static str,
    pub items: &'static [Quality],
}

pub fn catalog() -> Vec<PlatformQuality> {
    Source::ALL
        .iter()
        .map(|source| PlatformQuality {
            source: *source,
            label: source.label(),
            note: note_of(*source),
            items: table(*source),
        })
        .collect()
}

fn note_of(source: Source) -> &'static str {
    match source {
        Source::Netease => "登录后可用无损及以上；超清母带需要会员账号",
        Source::Qq => "无损及以上需要会员；未登录只能取到标准音质",
        Source::Kuwo => "该接口只提供标准音质，选更高档位会自动落到这里",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_table_is_sorted_high_to_low() {
        for source in Source::ALL {
            let table = table(source);
            assert!(!table.is_empty(), "{source} 的档位表不能为空");
            for pair in table.windows(2) {
                assert!(
                    pair[0].level > pair[1].level,
                    "{source} 档位必须从高到低且不重复：{:?} / {:?}",
                    pair[0].label,
                    pair[1].label
                );
            }
        }
    }

    #[test]
    fn find_caps_at_platform_ceiling() {
        // 酷我没有无损，要 master 也只能拿到它唯一的那档
        let found = find(Source::Kuwo, QualityLevel::Master).unwrap();
        assert_eq!(found.level, QualityLevel::Standard);
        // QQ 有母带，要 master 就给母带
        assert_eq!(
            find(Source::Qq, QualityLevel::Master).unwrap().level,
            QualityLevel::Master
        );
        // 要 192k（较高）时 QQ 没有这一档，应落到不高于它的最高档
        assert_eq!(
            find(Source::Qq, QualityLevel::High).unwrap().level,
            QualityLevel::Standard
        );
    }

    #[test]
    fn ladder_walks_down_from_target() {
        let levels: Vec<_> = ladder(Source::Netease, QualityLevel::Lossless)
            .iter()
            .map(|item| item.level)
            .collect();
        assert_eq!(
            levels,
            vec![
                QualityLevel::Lossless,
                QualityLevel::Exquisite,
                QualityLevel::High,
                QualityLevel::Standard,
            ]
        );
        // 最低档的降级链只有它自己
        assert_eq!(ladder(Source::Netease, QualityLevel::Standard).len(), 1);
    }

    #[test]
    fn parses_ids_and_legacy_bitrates() {
        assert_eq!(QualityLevel::parse("lossless"), Some(QualityLevel::Lossless));
        assert_eq!(QualityLevel::parse("SQ"), Some(QualityLevel::Lossless));
        assert_eq!(QualityLevel::parse("hiRes"), Some(QualityLevel::HiRes));
        assert_eq!(QualityLevel::parse("nope"), None);
        assert_eq!(QualityLevel::from_bitrate(320), QualityLevel::Exquisite);
        assert_eq!(QualityLevel::from_bitrate(999), QualityLevel::Lossless);
        assert_eq!(QualityLevel::from_bitrate(128), QualityLevel::Standard);
    }

    #[test]
    fn catalog_covers_every_source() {
        let catalog = catalog();
        assert_eq!(catalog.len(), Source::ALL.len());
        for entry in catalog {
            assert_eq!(entry.items, table(entry.source));
            assert!(!entry.note.is_empty());
        }
    }

    #[test]
    fn codes_are_unique_within_a_platform() {
        for source in Source::ALL {
            let mut codes: Vec<_> = table(source).iter().map(|item| item.code).collect();
            codes.sort_unstable();
            let before = codes.len();
            codes.dedup();
            assert_eq!(before, codes.len(), "{source} 有重复档位码");
        }
    }
}
