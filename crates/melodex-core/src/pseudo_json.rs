//! 宽容的 JSON 解析器。
//!
//! 部分音源（酷我 `search.kuwo.cn/r.s`）返回的是「单引号 JSON」——键和值都用
//! `'` 包裹，标准 `serde_json` 无法解析。这里实现一个只服务于该场景的递归下降
//! 解析器：同时接受单/双引号字符串，其余按 JSON 语法处理，并丢弃未知结构。

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Value>),
    Obj(Vec<(String, Value)>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            Value::Str(s) => s.parse::<f64>().ok(),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Arr(items) => Some(items.as_slice()),
            _ => None,
        }
    }

    /// 便利方法：按 key 取字符串，缺失时返回空串。
    pub fn str_of(&self, key: &str) -> String {
        self.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    }

    pub fn num_of(&self, key: &str) -> f64 {
        self.get(key).and_then(|v| v.as_f64()).unwrap_or(0.0)
    }
}

pub fn parse(input: &str) -> Result<Value, String> {
    let mut parser = Parser {
        chars: input.chars().collect(),
        pos: 0,
    };
    parser.skip_ws();
    let value = parser.parse_value()?;
    Ok(value)
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek();
        if ch.is_some() {
            self.pos += 1;
        }
        ch
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        self.skip_ws();
        match self.peek() {
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('\'') | Some('"') => Ok(Value::Str(self.parse_string()?)),
            Some('t') | Some('f') | Some('n') => self.parse_literal(),
            Some(_) => self.parse_number(),
            None => Err("意外的输入结束".to_string()),
        }
    }

    fn parse_object(&mut self) -> Result<Value, String> {
        self.bump(); // {
        let mut pairs = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some('}') => {
                    self.bump();
                    break;
                }
                None => return Err("对象未闭合".to_string()),
                _ => {}
            }
            let key = self.parse_string()?;
            self.skip_ws();
            if self.peek() != Some(':') {
                return Err(format!("键 {key} 缺少冒号"));
            }
            self.bump();
            let value = self.parse_value()?;
            pairs.push((key, value));
            self.skip_ws();
            if self.peek() == Some(',') {
                self.bump();
            }
        }
        Ok(Value::Obj(pairs))
    }

    fn parse_array(&mut self) -> Result<Value, String> {
        self.bump(); // [
        let mut items = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(']') => {
                    self.bump();
                    break;
                }
                None => return Err("数组未闭合".to_string()),
                _ => {}
            }
            items.push(self.parse_value()?);
            self.skip_ws();
            if self.peek() == Some(',') {
                self.bump();
            }
        }
        Ok(Value::Arr(items))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        let quote = self
            .bump()
            .ok_or_else(|| "期望字符串起始引号".to_string())?;
        if quote != '\'' && quote != '"' {
            return Err(format!("非法引号 {quote}"));
        }
        let mut out = String::new();
        loop {
            match self.bump() {
                None => return Err("字符串未闭合".to_string()),
                Some(ch) if ch == quote => break,
                Some('\\') => {
                    let escaped = self.bump().ok_or_else(|| "转义符后缺字符".to_string())?;
                    match escaped {
                        'n' => out.push('\n'),
                        't' => out.push('\t'),
                        'r' => out.push('\r'),
                        'u' => {
                            let mut code = String::new();
                            for _ in 0..4 {
                                if let Some(c) = self.bump() {
                                    code.push(c);
                                }
                            }
                            if let Ok(cp) = u32::from_str_radix(&code, 16) {
                                if let Some(c) = char::from_u32(cp) {
                                    out.push(c);
                                }
                            }
                        }
                        other => out.push(other),
                    }
                }
                Some(ch) => out.push(ch),
            }
        }
        Ok(out)
    }

    fn parse_literal(&mut self) -> Result<Value, String> {
        let mut word = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphabetic() {
                word.push(ch);
                self.bump();
            } else {
                break;
            }
        }
        match word.as_str() {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            "null" => Ok(Value::Null),
            other => Err(format!("未知字面量 {other}")),
        }
    }

    fn parse_number(&mut self) -> Result<Value, String> {
        let mut raw = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.' | 'e' | 'E') {
                raw.push(ch);
                self.bump();
            } else {
                break;
            }
        }
        raw.parse::<f64>()
            .map(Value::Num)
            .map_err(|_| format!("非法数字 {raw}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_quoted_payload() {
        let body = "{'TOTAL':'2','abslist':[{'SONGNAME':'晴天','DURATION':'269'},{\"SONGNAME\":\"七里香\",'DURATION':'299'}]}";
        let value = parse(body).unwrap();
        let list = value.get("abslist").and_then(|v| v.as_array()).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].str_of("SONGNAME"), "晴天");
        assert_eq!(list[1].str_of("SONGNAME"), "七里香");
        assert_eq!(list[1].num_of("DURATION"), 299.0);
    }

    #[test]
    fn parses_nested_objects() {
        let body = "{'a':{'b':'c'},'n':1}";
        let value = parse(body).unwrap();
        assert_eq!(
            value
                .get("a")
                .and_then(|v| v.get("b"))
                .and_then(|v| v.as_str()),
            Some("c")
        );
        assert_eq!(value.num_of("n"), 1.0);
    }

    #[test]
    fn handles_escapes() {
        let body = "{'name':'it\\'s ok'}";
        let value = parse(body).unwrap();
        assert_eq!(value.str_of("name"), "it's ok");
    }
}
