use serde::{Deserialize, Serialize};

/// 逐行歌词。`translation` 为可选的翻译文本（对应同一时间点）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub time_ms: u64,
    pub text: String,
    pub translation: Option<String>,
}

/// 解析 LRC 文本。支持一行多个时间戳（`[00:01.00][00:05.00]歌词`）。
pub fn parse_lrc(lrc: &str) -> Vec<LyricLine> {
    let mut lines: Vec<LyricLine> = Vec::new();
    for raw in lrc.lines() {
        let raw = raw.trim_end();
        if raw.is_empty() {
            continue;
        }
        let mut rest = raw;
        let mut stamps: Vec<u64> = Vec::new();
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some(end) = stripped.find(']') else { break };
            let tag = &stripped[..end];
            rest = &stripped[end + 1..];
            if let Some(ms) = parse_stamp(tag) {
                stamps.push(ms);
            }
            // 元信息标签（ti/ar/al/by/offset）跳过，继续找时间戳
        }
        let text = rest.trim().to_string();
        if stamps.is_empty() || text.is_empty() {
            continue;
        }
        for ms in stamps {
            lines.push(LyricLine {
                time_ms: ms,
                text: text.clone(),
                translation: None,
            });
        }
    }
    lines.sort_by_key(|line| line.time_ms);
    lines
}

fn parse_stamp(tag: &str) -> Option<u64> {
    let (minutes, rest) = tag.split_once(':')?;
    let minutes: u64 = minutes.trim().parse().ok()?;
    let (seconds, fraction) = match rest.split_once('.') {
        Some((s, f)) => (s, Some(f)),
        None => (rest, None),
    };
    let seconds: u64 = seconds.trim().parse().ok()?;
    let millis = match fraction {
        Some(f) => {
            let digits: String = f.chars().filter(|c| c.is_ascii_digit()).collect();
            let value: u64 = digits.parse().unwrap_or(0);
            match digits.len() {
                0 => 0,
                1 => value * 100,
                2 => value * 10,
                _ => value / 10u64.pow(digits.len() as u32 - 3),
            }
        }
        None => 0,
    };
    Some(minutes * 60_000 + seconds * 1_000 + millis)
}

/// 还原歌词文本里的 HTML 实体。
///
/// QQ 音乐的歌词接口会把时间轴里的冒号、换行写成数字实体（`&#58;`、`&#10;`），
/// 不还原的话整份 LRC 会被解析成一堆取不到时间戳的垃圾行。
pub fn unescape_entities(raw: &str) -> String {
    if !raw.contains('&') {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(index) = rest.find('&') {
        out.push_str(&rest[..index]);
        let tail = &rest[index..];
        // 找最近的 `;`，中间的词就是实体名；解析失败就把它当普通字符吐出去
        let parsed = tail.find(';').and_then(|end| {
            let entity = &tail[1..end];
            let ch = if let Some(number) = entity.strip_prefix('#') {
                number.parse::<u32>().ok().and_then(char::from_u32)
            } else {
                match entity {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "nbsp" => Some(' '),
                    _ => None,
                }
            };
            ch.map(|ch| (ch, end + 1))
        });
        match parsed {
            Some((ch, consumed)) => {
                out.push(ch);
                rest = &tail[consumed..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// 把翻译歌词按时间点并入主歌词。
pub fn merge_translation(lines: &mut [LyricLine], translation: &str) {
    let translated = parse_lrc(translation);
    for line in lines.iter_mut() {
        if let Some(found) = translated
            .iter()
            .find(|t| t.time_ms.abs_diff(line.time_ms) < 50)
        {
            line.translation = Some(found.text.clone());
        }
    }
}

/// 当前播放位置应高亮的行号。
pub fn current_index(lines: &[LyricLine], position_ms: u64) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }
    let mut index = 0usize;
    for (i, line) in lines.iter().enumerate() {
        if line.time_ms <= position_ms {
            index = i;
        } else {
            break;
        }
    }
    Some(index)
}

/// 生成 LRC 文本，用于 `保存 .lrc` 功能。
pub fn to_lrc(lines: &[LyricLine], with_translation: bool) -> String {
    let mut out = String::new();
    for line in lines {
        let stamp = format!(
            "[{:02}:{:02}.{:02}]",
            line.time_ms / 60_000,
            (line.time_ms / 1_000) % 60,
            (line.time_ms % 1_000) / 10
        );
        out.push_str(&stamp);
        out.push_str(&line.text);
        out.push('\n');
        if with_translation {
            if let Some(ref t) = line.translation {
                out.push_str(&stamp);
                out.push_str(t);
                out.push('\n');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_lrc() {
        // 元信息标签（ti/ar）没有时间戳，不应被当成歌词行
        let lrc = "[ti:晴天]\n[ar:周杰伦]\n[00:12.34]故事的小黄花\n[01:02.5]从出生那年就飘着\n";
        let lines = parse_lrc(lrc);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].time_ms, 12_340);
        assert_eq!(lines[0].text, "故事的小黄花");
        assert_eq!(lines[1].time_ms, 62_500);
    }

    #[test]
    fn expands_multi_stamp_lines() {
        let lines = parse_lrc("[00:01.00][00:05.00]重复句");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].time_ms, 5_000);
    }

    #[test]
    fn merges_translation() {
        let mut lines = parse_lrc("[00:10.00]Hello");
        merge_translation(&mut lines, "[00:10.00]你好");
        assert_eq!(lines[0].translation.as_deref(), Some("你好"));
    }

    #[test]
    fn current_index_follows_position() {
        let lines = parse_lrc("[00:01.00]a\n[00:05.00]b\n[00:09.00]c");
        assert_eq!(current_index(&lines, 0), Some(0));
        assert_eq!(current_index(&lines, 6_000), Some(1));
        assert_eq!(current_index(&lines, 60_000), Some(2));
    }

    #[test]
    fn roundtrip_lrc() {
        let lines = parse_lrc("[00:12.34]故事的小黄花");
        let text = to_lrc(&lines, false);
        assert_eq!(text.trim(), "[00:12.34]故事的小黄花");
    }

    #[test]
    fn unescapes_entities() {
        // QQ 的 LRC 里冒号/换行常被写成数字实体
        assert_eq!(unescape_entities("[00&#58;12.00]hi"), "[00:12.00]hi");
        assert_eq!(unescape_entities("a&#10;b"), "a\nb");
        assert_eq!(unescape_entities("A &amp; B"), "A & B");
        assert_eq!(unescape_entities("no entity"), "no entity");
        // 不完整的实体保持原样，不能吞字符
        assert_eq!(unescape_entities("50% & more"), "50% & more");
        assert_eq!(unescape_entities("&unknown;"), "&unknown;");
        // 还原之后要能被 LRC 解析器正常吃掉
        let lines = parse_lrc(&unescape_entities("[00&#58;01.00]a"));
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].time_ms, 1_000);
    }
}
