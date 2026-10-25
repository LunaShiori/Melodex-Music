//! Cookie 串的解析与拼接。
//!
//! 独立成模块（不依赖网络库）是为了能单独测试：扫码登录时从 `Set-Cookie`
//! 里拼 Cookie 串、从 Cookie 串里读 `MUSIC_U` / `uin` 这类键，都是纯字符串逻辑。

/// 把多条 `Set-Cookie` 头拼成一个可直接回传的 Cookie 串。
///
/// - 只取第一个 `;` 之前的部分，忽略 `Path` / `Domain` / `Expires` 等属性；
/// - 同名项后者覆盖前者（音源会先下发旧票据再用新的覆盖）；
/// - 空名或空值直接丢弃。
pub fn from_set_cookie<'a>(values: impl IntoIterator<Item = &'a str>) -> String {
    let mut pairs: Vec<(&str, &str)> = Vec::new();
    for value in values {
        let Some(first) = value.split(';').next() else {
            continue;
        };
        let first = first.trim();
        if first.is_empty() {
            continue;
        }
        let Some((name, val)) = first.split_once('=') else {
            continue;
        };
        let (name, val) = (name.trim(), val.trim());
        if name.is_empty() || val.is_empty() {
            continue;
        }
        match pairs.iter_mut().find(|(existing, _)| *existing == name) {
            Some(slot) => slot.1 = val,
            None => pairs.push((name, val)),
        }
    }
    pairs
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// 从 Cookie 串里读某个键的值。
pub fn value_of<'a>(cookie: &'a str, name: &str) -> Option<&'a str> {
    cookie.split(';').find_map(|pair| {
        let (key, value) = pair.trim().split_once('=')?;
        (key.trim() == name).then_some(value.trim())
    })
}

/// 列出 Cookie 里出现的键名。用于界面展示"存了哪些字段"，不暴露值。
pub fn names(cookie: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for pair in cookie.split(';') {
        let Some((key, value)) = pair.trim().split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if key.is_empty() || value.is_empty() {
            continue;
        }
        if !out.iter().any(|existing| existing == key) {
            out.push(key.to_string());
        }
    }
    out
}

/// 合并两段 Cookie（前段为基底，后段覆盖同名项）。
pub fn merge(base: &str, extra: &str) -> String {
    let mut out = base.to_string();
    for pair in extra.split(';') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let name = pair.split_once('=').map(|(k, _)| k.trim()).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        // 先删掉同名项，再追加，保证「后者覆盖前者」
        let kept: Vec<&str> = out
            .split(';')
            .map(str::trim)
            .filter(|item| {
                !item.is_empty()
                    && item
                        .split_once('=')
                        .map(|(k, _)| k.trim() != name)
                        .unwrap_or(true)
            })
            .collect();
        out = kept.join("; ");
        if !out.is_empty() {
            out.push_str("; ");
        }
        out.push_str(pair);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_name_value() {
        let cookie = from_set_cookie([
            "MUSIC_U=abc123; Max-Age=1296000; Expires=Wed, 01 Jan 2027 00:00:00 GMT; Path=/",
            "__csrf=def456; Path=/;; $Domain=.music.163.com",
        ]);
        assert_eq!(cookie, "MUSIC_U=abc123; __csrf=def456");
    }

    #[test]
    fn later_value_wins() {
        let cookie = from_set_cookie(["MUSIC_U=old", "MUSIC_U=new", "NMTID=n1"]);
        assert_eq!(cookie, "MUSIC_U=new; NMTID=n1");
        assert_eq!(value_of(&cookie, "MUSIC_U"), Some("new"));
    }

    #[test]
    fn drops_junk() {
        let cookie = from_set_cookie(["", "  ", "broken", "=nokey", "key="]);
        assert_eq!(cookie, "");
    }

    #[test]
    fn reads_value() {
        let cookie = "MUSIC_U=abc; __csrf=def; os=pc";
        assert_eq!(value_of(cookie, "MUSIC_U"), Some("abc"));
        assert_eq!(value_of(cookie, "os"), Some("pc"));
        assert_eq!(value_of(cookie, "missing"), None);
    }

    #[test]
    fn lists_names_only() {
        assert_eq!(
            names("MUSIC_U=a; __csrf=b; broken"),
            vec!["MUSIC_U", "__csrf"]
        );
        assert!(names("").is_empty());
    }

    #[test]
    fn merge_overrides() {
        assert_eq!(merge("a=1; b=2", "b=3; c=4"), "a=1; b=3; c=4");
        assert_eq!(merge("", "a=1"), "a=1");
    }
}
