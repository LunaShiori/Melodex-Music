//! QQ 音乐的文件名拼装与响应字段识别。
//!
//! 这块逻辑刻意不依赖网络层：文件名拼错只会在运行时表现为「服务端回 101404」，
//! 很难在真机上反复试，所以全部抽成纯函数并配单测，`cargo test --no-default-features`
//! 就能覆盖（与 [`crate::cookie`]、[`crate::lyric`] 的分层思路一致）。
//!
//! 为什么要「一个档位多个候选文件名」：QQ 的曲库跨了十几年，同一档位在不同年代的
//! 曲目上用了不同的文件名前缀与 mid 来源——
//!
//! | 档位 | 新前缀 + media_mid | 旧前缀 + songmid |
//! | --- | --- | --- |
//! | 臻品母带 | `AIM0{vs[3]}.mflac` | `AI00{songmid}.flac` |
//! | 全景声 | `Q0M0{vs[4]}.mflac` | `Q000{vs[4]}.flac` |
//! | Hi-Res | `RSM1{media_mid}.mflac` | `RS01{songmid}.flac` |
//! | SQ 无损 | `F0M0{media_mid}.mflac` | `F000{songmid}.flac` |
//! | 320k / 128k | `M800` / `M500` + `media_mid` 或 `songmid` |
//!
//! 服务端对不认识的文件名只回 `result=101404`，不会提示正确拼法，
//! 因此一次请求把这些候选全递上去，按返回顺序取第一个拿到链接的。

use crate::quality::Quality;

/// 歌曲详情里取链接需要的字段（搜索结果里没有，要单独查一次）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileInfo {
    pub media_mid: String,
    /// 不同音质对应的媒体 mid 列表：母带取 `vs[3]`、全景声取 `vs[4]`。
    pub vs: Vec<String>,
}

/// 按档位生成候选文件名，越靠前越可信。
pub fn candidate_files(
    quality: &Quality,
    songmid: &str,
    media_mid: &str,
    info: &FileInfo,
) -> Vec<String> {
    let at = |index: usize| info.vs.get(index).map(String::as_str).unwrap_or_default();
    let vs_master = at(3);
    let vs_atmos = at(4);
    let mut out: Vec<String> = Vec::new();

    match quality.code {
        // 臻品母带：vs[3] 是母带专用的媒体 mid
        "AIM0" => {
            for source in [vs_master, media_mid, songmid] {
                push_candidate(&mut out, "AIM0", source, "mflac");
                push_candidate(&mut out, "AI00", source, "flac");
            }
        }
        // 臻品全景声：vs[4]
        "Q0M0" => {
            for source in [vs_atmos, media_mid, songmid] {
                push_candidate(&mut out, "Q0M0", source, "mflac");
                push_candidate(&mut out, "Q000", source, "flac");
            }
        }
        // Hi-Res：media_mid 优先，vs[3] 作为后备（部分曲目 hi-res 与母带共用媒体 mid）
        "RSM1" => {
            for source in [media_mid, vs_master, songmid] {
                push_candidate(&mut out, "RSM1", source, "mflac");
                push_candidate(&mut out, "RS01", source, "flac");
            }
        }
        // SQ 无损
        "F0M0" => {
            for source in [media_mid, songmid] {
                push_candidate(&mut out, "F0M0", source, "mflac");
                push_candidate(&mut out, "F000", source, "flac");
            }
        }
        // 有损档位（M500 / M800）沿用档位自己的扩展名
        _ => {
            for source in [media_mid, songmid] {
                push_candidate(&mut out, quality.code, source, quality.format);
            }
        }
    }
    out
}

fn push_candidate(out: &mut Vec<String>, prefix: &str, mid: &str, ext: &str) {
    if mid.is_empty() {
        return;
    }
    let name = format!("{prefix}{mid}.{ext}");
    if !out.contains(&name) {
        out.push(name);
    }
}

/// 从直链里取扩展名，取不到或明显异常时返回 `None`。
///
/// 长度上限放到 6：QQ 的无损扩展名是 `mflac`（5 个字符），
/// 按 4 来卡会把最关键的这一档判成「没有扩展名」。
pub fn extension_of(url: &str) -> Option<String> {
    let clean = url.split('?').next().unwrap_or(url);
    let ext = clean.rsplit('.').next()?;
    if ext.is_empty() || ext.len() > 6 || ext.contains('/') {
        return None;
    }
    Some(ext.to_ascii_lowercase())
}

/// 无损及以上的合法扩展名。服务端对没权限的档位可能静默回低码率替身，
/// 用它把「假无损」挡下来，交给引擎继续降级。
pub fn is_lossless_format(format: &str) -> bool {
    matches!(format, "flac" | "mflac" | "ape")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quality_at(code: &str) -> &'static Quality {
        crate::quality::QQ
            .iter()
            .find(|item| item.code == code)
            .expect("档位应存在于 QQ 表里")
    }

    fn info() -> FileInfo {
        FileInfo {
            media_mid: "003Qui1q2u1Zho".into(),
            vs: vec![
                "064YyRNb0gDV4P".into(),
                String::new(),
                String::new(),
                "001diEF43yBtZk".into(),
                "001MvKtb0pqYPR".into(),
            ],
        }
    }

    #[test]
    fn master_uses_vs_index_three() {
        let files = candidate_files(
            quality_at("AIM0"),
            "0039MnYb0qxYhV",
            "003Qui1q2u1Zho",
            &info(),
        );
        assert_eq!(files[0], "AIM0001diEF43yBtZk.mflac");
        assert!(files.contains(&"AI00001diEF43yBtZk.flac".to_string()));
        assert!(files.contains(&"AIM0003Qui1q2u1Zho.mflac".to_string()));
    }

    #[test]
    fn atmos_uses_vs_index_four() {
        let files = candidate_files(
            quality_at("Q0M0"),
            "0039MnYb0qxYhV",
            "003Qui1q2u1Zho",
            &info(),
        );
        assert_eq!(files[0], "Q0M0001MvKtb0pqYPR.mflac");
    }

    #[test]
    fn hires_prefers_media_mid() {
        let files = candidate_files(quality_at("RSM1"), "song", "media", &info());
        assert_eq!(files[0], "RSM1media.mflac");
        // media 之后才是 vs[3]、songmid 这两层后备
        assert!(files.contains(&"RSM1001diEF43yBtZk.mflac".to_string()));
        assert!(files.contains(&"RSM1song.mflac".to_string()));
    }

    #[test]
    fn candidates_skip_empty_mids_and_dedupe() {
        let empty = FileInfo::default();
        // media_mid 与 songmid 相同时不应出现重复候选
        let files = candidate_files(quality_at("M500"), "abc", "abc", &empty);
        assert_eq!(files, vec!["M500abc.mp3"]);
        // vs 全空时母带档仍然给出基于 media/songmid 的候选
        let files = candidate_files(quality_at("AIM0"), "abc", "abc", &empty);
        assert_eq!(files, vec!["AIM0abc.mflac", "AI00abc.flac"]);
    }

    #[test]
    fn detects_extension() {
        assert_eq!(
            extension_of("http://ws.stream.qqmusic.qq.com/M500abc.mp3?guid=1"),
            Some("mp3".into())
        );
        assert_eq!(extension_of("http://host/a.mflac"), Some("mflac".into()));
        assert_eq!(extension_of("http://host/a.tooolong"), None);
    }

    #[test]
    fn lossless_formats() {
        assert!(is_lossless_format("flac"));
        assert!(is_lossless_format("mflac"));
        assert!(is_lossless_format("ape"));
        assert!(!is_lossless_format("mp3"));
        assert!(!is_lossless_format("m4a"));
    }
}
