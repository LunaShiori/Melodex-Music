use std::path::{Path, PathBuf};

use crate::settings::DuplicatePolicy;

/// 命名模板上下文。模板占位符见 [`render`]。
pub struct NamingContext<'a> {
    pub title: &'a str,
    pub artists: &'a [String],
    pub album: &'a str,
    pub source: &'a str,
    pub bitrate: u32,
    pub index: usize,
    pub artist_separator: &'a str,
}

/// 渲染命名模板。
///
/// 支持的占位符：`{title}` `{artist}` `{album}` `{source}` `{bitrate}` `{index}`
/// `{index:02}`。未知占位符原样保留，方便用户自行排查模板错误。
pub fn render(template: &str, ctx: &NamingContext<'_>) -> String {
    let artists = ctx.artists.join(ctx.artist_separator);
    let mut out = String::with_capacity(template.len() + 32);
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '{' {
            out.push(ch);
            continue;
        }
        let mut name = String::new();
        let mut closed = false;
        for next in chars.by_ref() {
            if next == '}' {
                closed = true;
                break;
            }
            name.push(next);
        }
        if !closed {
            out.push('{');
            out.push_str(&name);
            continue;
        }
        let (key, width) = match name.split_once(':') {
            Some((key, spec)) => (key, spec.parse::<usize>().ok()),
            None => (name.as_str(), None),
        };
        match key {
            "title" => out.push_str(ctx.title),
            "artist" => out.push_str(&artists),
            "album" => out.push_str(ctx.album),
            "source" => out.push_str(ctx.source),
            "bitrate" => out.push_str(&ctx.bitrate.to_string()),
            "index" => match width {
                Some(w) => out.push_str(&format!("{:0w$}", ctx.index, w = w)),
                None => out.push_str(&ctx.index.to_string()),
            },
            _ => {
                out.push('{');
                out.push_str(&name);
                out.push('}');
            }
        }
    }
    sanitize(&out)
}

/// 去掉各平台文件名非法字符，并限制长度。
/// Windows 保留名（CON/PRN/...）加下划线前缀避免创建失败。
pub fn sanitize(raw: &str) -> String {
    let mut cleaned = String::with_capacity(raw.len());
    for ch in raw.chars() {
        let illegal = matches!(
            ch,
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0'
        ) || (ch as u32) < 0x20;
        if illegal {
            cleaned.push(' ');
        } else {
            cleaned.push(ch);
        }
    }
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed
        .trim_matches(|c: char| c == '.' || c == ' ')
        .to_string();
    let mut limited: String = trimmed.chars().take(120).collect();
    if limited.is_empty() {
        limited = "未命名".to_string();
    }
    const RESERVED: [&str; 9] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "LPT1",
    ];
    if RESERVED
        .iter()
        .any(|name| limited.eq_ignore_ascii_case(name))
    {
        limited = format!("_{limited}");
    }
    limited
}

#[derive(Debug, PartialEq)]
pub enum Resolution {
    /// 直接写入该路径
    Write(PathBuf),
    /// 文件已存在，按策略跳过
    Skip(PathBuf),
}

/// 依据重复策略决定目标路径。
pub fn resolve_target(dir: &Path, stem: &str, ext: &str, policy: DuplicatePolicy) -> Resolution {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return Resolution::Write(first);
    }
    match policy {
        DuplicatePolicy::Skip => Resolution::Skip(first),
        DuplicatePolicy::Overwrite => Resolution::Write(first),
        DuplicatePolicy::KeepBoth => {
            for index in 2..1000 {
                let candidate = dir.join(format!("{stem} ({index}).{ext}"));
                if !candidate.exists() {
                    return Resolution::Write(candidate);
                }
            }
            Resolution::Write(dir.join(format!("{stem} (1000).{ext}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>(artists: &'a [String]) -> NamingContext<'a> {
        NamingContext {
            title: "晴天",
            artists,
            album: "叶惠美",
            source: "netease",
            bitrate: 320,
            index: 7,
            artist_separator: " / ",
        }
    }

    #[test]
    fn renders_all_placeholders() {
        let artists = vec!["周杰伦".to_string()];
        let c = ctx(&artists);
        assert_eq!(render("{artist} - {title}", &c), "周杰伦 - 晴天");
        assert_eq!(render("{index:02}. {title}", &c), "07. 晴天");
        assert_eq!(
            render("{album}/{source}/{bitrate}", &c),
            "叶惠美 netease 320"
        );
    }

    #[test]
    fn keeps_unknown_placeholder() {
        let artists: Vec<String> = vec![];
        let c = ctx(&artists);
        assert_eq!(render("{unknown}", &c), "{unknown}");
    }

    #[test]
    fn sanitizes_illegal_chars() {
        assert_eq!(sanitize("a/b:c*d?"), "a b c d");
        assert_eq!(sanitize("  ..trim..  "), "trim");
        assert_eq!(sanitize("CON"), "_CON");
        assert_eq!(sanitize(""), "未命名");
    }

    #[test]
    fn limits_length() {
        let long = "音".repeat(300);
        assert_eq!(sanitize(&long).chars().count(), 120);
    }
}
