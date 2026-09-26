
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(HashMap<String, Json>),
}

#[derive(Debug)]
pub(crate) struct ParseError;

type Result<T> = std::result::Result<T, ParseError>;

const MAX_NESTING_DEPTH: usize = 64;

impl Json {
    pub(crate) fn parse(text: &str) -> Result<Json> {
        let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
        let mut p = Parser { text, pos: 0, depth: 0 };
        let value = p.parse_value()?;
        p.skip_ws();
        if p.pos != text.len() {
            return Err(ParseError);
        }
        Ok(value)
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub(crate) fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(m) => m.get(key),
            _ => None,
        }
    }
}

struct Parser<'a> {
    text: &'a str,
    pos: usize,
    depth: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek();
        if b.is_some() {
            self.pos += 1;
        }
        b
    }

    fn next_char(&mut self) -> Option<char> {
        let c = self.text[self.pos..].chars().next()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn skip_digits(&mut self) {
        while matches!(self.peek(), Some(b) if b.is_ascii_digit()) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, b: u8) -> Result<()> {
        if self.bump() == Some(b) { Ok(()) } else { Err(ParseError) }
    }

    fn literal(&mut self, lit: &str, value: Json) -> Result<Json> {
        if self.text[self.pos..].starts_with(lit) {
            self.pos += lit.len();
            Ok(value)
        } else {
            Err(ParseError)
        }
    }

    fn parse_value(&mut self) -> Result<Json> {
        self.skip_ws();
        match self.peek().ok_or(ParseError)? {
            b @ (b'{' | b'[') => {
                self.depth += 1;
                if self.depth > MAX_NESTING_DEPTH {
                    return Err(ParseError);
                }
                let result = if b == b'{' { self.parse_object() } else { self.parse_array() };
                self.depth -= 1;
                result
            }
            b'"' => self.parse_string().map(Json::String),
            b't' => self.literal("true", Json::Bool(true)),
            b'f' => self.literal("false", Json::Bool(false)),
            b'n' => self.literal("null", Json::Null),
            b'-' | b'0'..=b'9' => self.parse_number(),
            _ => Err(ParseError),
        }
    }

    fn parse_object(&mut self) -> Result<Json> {
        self.expect(b'{')?;
        let mut map = HashMap::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Json::Object(map));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect(b':')?;
            map.insert(key, self.parse_value()?);
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => return Ok(Json::Object(map)),
                _ => return Err(ParseError),
            }
        }
    }

    fn parse_array(&mut self) -> Result<Json> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => return Ok(Json::Array(items)),
                _ => return Err(ParseError),
            }
        }
    }

    fn parse_string(&mut self) -> Result<String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            match self.next_char().ok_or(ParseError)? {
                '"' => return Ok(out),
                '\\' => match self.next_char().ok_or(ParseError)? {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    '/' => out.push('/'),
                    'b' => out.push('\u{0008}'),
                    'f' => out.push('\u{000C}'),
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    'u' => {
                        let hi = self.parse_hex4()?;
                        let code = if (0xD800..=0xDBFF).contains(&hi) {
                            if self.next_char() != Some('\\') || self.next_char() != Some('u') {
                                return Err(ParseError);
                            }
                            let lo = self.parse_hex4()?;
                            if !(0xDC00..=0xDFFF).contains(&lo) {
                                return Err(ParseError);
                            }
                            0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                        } else {
                            hi
                        };
                        out.push(char::from_u32(code).ok_or(ParseError)?);
                    }
                    _ => return Err(ParseError),
                },
                c => out.push(c),
            }
        }
    }

    fn parse_hex4(&mut self) -> Result<u32> {
        let mut value = 0;
        for _ in 0..4 {
            value = value * 16 + self.next_char().and_then(|c| c.to_digit(16)).ok_or(ParseError)?;
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<Json> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        self.skip_digits();
        if self.peek() == Some(b'.') {
            self.pos += 1;
            self.skip_digits();
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            self.skip_digits();
        }
        self.text[start..self.pos].parse().map(Json::Number).map_err(|_| ParseError)
    }
}

pub(crate) fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_un_objet_plat() {
        let v = Json::parse(r#"{"hotkey": "ctrl+space", "n": 42, "ok": true, "nil": null}"#).unwrap();
        assert_eq!(v.get("hotkey").unwrap().as_str(), Some("ctrl+space"));
        assert_eq!(v.get("n").unwrap().as_f64(), Some(42.0));
        assert_eq!(v.get("ok").unwrap(), &Json::Bool(true));
        assert_eq!(v.get("nil").unwrap(), &Json::Null);
    }

    #[test]
    fn parse_echappements_et_unicode() {
        let v = Json::parse(r#""line1\nline2\té😀\ud83d\ude00""#).unwrap();
        assert_eq!(v.as_str().unwrap(), "line1\nline2\t\u{00e9}\u{1F600}\u{1F600}");
    }

    #[test]
    fn parse_ignore_un_bom_utf8_en_tete() {
        let v = Json::parse("\u{FEFF}{\"quality\": 90}").unwrap();
        assert_eq!(v.get("quality").unwrap().as_f64(), Some(90.0));
    }

    #[test]
    fn rejette_json_mal_forme() {
        assert!(Json::parse("{").is_err());
        assert!(Json::parse("not json").is_err());
        assert!(Json::parse(r#"{"a": 1,}"#).is_err());
        assert!(Json::parse("[1] 2").is_err());
    }

    #[test]
    fn rejette_une_imbrication_trop_profonde_sans_planter() {
        let depth = 200_000;
        let text = "[".repeat(depth) + &"]".repeat(depth);
        assert!(Json::parse(&text).is_err());
    }

    #[test]
    fn quote_puis_parse_restitue_la_chaine() {
        let s = "C:\\Users\\\"Test\"\n\u{1}é";
        assert_eq!(Json::parse(&quote(s)).unwrap().as_str(), Some(s));
    }
}
