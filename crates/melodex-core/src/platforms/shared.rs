//! 各音源解析时的公共小工具。
//!
//! 四个音源的搜索响应结构各不相同，但「取字符串字段」「拆歌手」「解 HTML 实体」
//! 这类活儿是一样的——放在这里，避免每个平台各写一份略有出入的实现。

use serde_json::Value;

/// 取一个字符串字段并 trim；缺字段或不是字符串时返回空串。
pub fn str_field(item: &Value, key: &str) -> String {
    item.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// 取一个整数字段；也接受字符串形式的数字（部分接口混着给）。
pub fn num_field(item: &Value, key: &str) -> u64 {
    match item.get(key) {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(Value::String(raw)) => raw.trim().parse::<u64>().unwrap_or(0),
        _ => 0,
    }
}

/// 拆多歌手。各平台用 `、` `/` `&` `,` `;` 混着分隔，统一都认。
pub fn split_artists(raw: &str) -> Vec<String> {
    raw.split(['、', '/', '&', ',', ';', '；'])
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

/// 解开接口里常见的 HTML 实体（搜索标题、歌名里很常见）。
pub fn decode_entities(raw: &str) -> String {
    raw.replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_string_and_number_fields() {
        let item = json!({ "name": " 晴天 ", "play": 12345, "size": "42" });
        assert_eq!(str_field(&item, "name"), "晴天");
        assert_eq!(str_field(&item, "missing"), "");
        assert_eq!(num_field(&item, "play"), 12345);
        assert_eq!(num_field(&item, "size"), 42);
        assert_eq!(num_field(&item, "missing"), 0);
    }

    #[test]
    fn splits_artists_across_separators() {
        assert_eq!(split_artists("周杰伦、叶惠美"), vec!["周杰伦", "叶惠美"]);
        assert_eq!(split_artists("A/B&C"), vec!["A", "B", "C"]);
        assert_eq!(split_artists("  "), Vec::<String>::new());
    }

    #[test]
    fn decodes_common_entities() {
        assert_eq!(decode_entities("A &amp; B"), "A & B");
        assert_eq!(decode_entities("&quot;x&quot;"), "\"x\"");
        assert_eq!(decode_entities("  a&nbsp;b  "), "a b");
    }
}
