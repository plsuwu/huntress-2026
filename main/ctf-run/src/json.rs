//! Owned JSON values: parser, compact writer and pretty-printer.
//!
//! Numbers keep their source text so ids and large integers round-trip exactly.
//! Objects keep member order (the Game Master's order is meaningful for downloads).

use std::fmt::{self, Write};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<Value>),
    Obj(Vec<(String, Value)>),
}

#[derive(Debug)]
pub struct Error {
    pub at: usize,
    pub msg: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "JSON error at byte {}: {}", self.at, self.msg)
    }
}

impl std::error::Error for Error {}

impl Value {
    pub fn parse(text: &str) -> Result<Value, Error> {
        Self::parse_bytes(text.as_bytes())
    }

    /// serde_json (1.6x the hand-rolled parser on a 3.7 MB start response) driving a visitor that
    /// builds `Value` directly; `arbitrary_precision` hands numbers over as their source text.
    pub fn parse_bytes(src: &[u8]) -> Result<Value, Error> {
        let mut de = serde_json::Deserializer::from_slice(src);
        let v = serde::de::DeserializeSeed::deserialize(Seed, &mut de).and_then(|v| de.end().map(|_| v));
        v.map_err(|e| {
            // serde reports line/column (1-based); keep the byte offset this type has always given.
            let line_start: usize = src.split(|&b| b == b'\n').take(e.line().saturating_sub(1)).map(|l| l.len() + 1).sum();
            Error { at: line_start + e.column().saturating_sub(1), msg: e.to_string() }
        })
    }

    /// Member of an object (first match).
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Follows a path of object keys.
    pub fn at(&self, path: &[&str]) -> Option<&Value> {
        path.iter().try_fold(self, |v, k| v.get(k))
    }

    pub fn str_of(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Arr(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Obj(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Num(n) => n.parse().ok(),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Num(n) => n.parse().ok(),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Compact serialisation.
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    pub fn write(&self, out: &mut String) {
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Num(n) => out.push_str(n),
            Value::Str(s) => write_str(out, s),
            Value::Arr(a) => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write(out);
                }
                out.push(']');
            }
            Value::Obj(m) => {
                out.push('{');
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_str(out, k);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }

    /// Two-space indented serialisation, for humans.
    pub fn pretty(&self) -> String {
        let mut out = String::new();
        self.pretty_into(&mut out, 0);
        out
    }

    fn pretty_into(&self, out: &mut String, depth: usize) {
        let pad = |out: &mut String, d: usize| {
            out.push('\n');
            for _ in 0..d {
                out.push_str("  ");
            }
        };
        match self {
            Value::Arr(a) if !a.is_empty() => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    pad(out, depth + 1);
                    v.pretty_into(out, depth + 1);
                }
                pad(out, depth);
                out.push(']');
            }
            Value::Obj(m) if !m.is_empty() => {
                out.push('{');
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    pad(out, depth + 1);
                    write_str(out, k);
                    out.push_str(": ");
                    v.pretty_into(out, depth + 1);
                }
                pad(out, depth);
                out.push('}');
            }
            _ => self.write(out),
        }
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::Str(s.to_string())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::Str(s)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Value {
        Value::Num(n.to_string())
    }
}

impl From<usize> for Value {
    fn from(n: usize) -> Value {
        Value::Num(n.to_string())
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Value {
        if n.is_finite() {
            Value::Num(format!("{n}"))
        } else {
            Value::Null
        }
    }
}

/// Builds an object from `(key, value)` pairs: `obj([("a", 1.into())])`.
pub fn obj<const N: usize>(members: [(&str, Value); N]) -> Value {
    Value::Obj(members.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

/// Appends `s` as a JSON string literal.
pub fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Re-indents JSON text; returns the input unchanged when it does not parse.
pub fn pretty_text(src: &[u8]) -> String {
    match Value::parse_bytes(src) {
        Ok(v) => v.pretty(),
        Err(_) => String::from_utf8_lossy(src).into_owned(),
    }
}

/// Builds `Value`s from serde_json events. Objects keep member order and duplicate keys.
#[derive(Clone, Copy)]
struct Seed;

impl<'de> serde::de::DeserializeSeed<'de> for Seed {
    type Value = Value;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
        d.deserialize_any(Seed)
    }
}

/// The key serde_json's `arbitrary_precision` wraps a number's source text in.
const NUMBER_TOKEN: &str = "$serde_json::private::Number";

impl<'de> serde::de::Visitor<'de> for Seed {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a JSON value")
    }
    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_bool<E>(self, b: bool) -> Result<Value, E> {
        Ok(Value::Bool(b))
    }
    fn visit_u64<E>(self, n: u64) -> Result<Value, E> {
        Ok(Value::Num(n.to_string()))
    }
    fn visit_i64<E>(self, n: i64) -> Result<Value, E> {
        Ok(Value::Num(n.to_string()))
    }
    fn visit_f64<E>(self, n: f64) -> Result<Value, E> {
        Ok(Value::Num(n.to_string()))
    }
    fn visit_str<E>(self, s: &str) -> Result<Value, E> {
        Ok(Value::Str(s.to_string()))
    }
    fn visit_string<E>(self, s: String) -> Result<Value, E> {
        Ok(Value::Str(s))
    }
    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(v) = a.next_element_seed(Seed)? {
            items.push(v);
        }
        Ok(Value::Arr(items))
    }
    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        let mut members = Vec::new();
        while let Some(k) = a.next_key::<String>()? {
            if members.is_empty() && k == NUMBER_TOKEN {
                return Ok(Value::Num(a.next_value()?));
            }
            members.push((k, a.next_value_seed(Seed)?));
        }
        Ok(Value::Obj(members))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let src = r#"{"a":[1,-2.5e3,true,false,null],"b":"x\"y\\z\n\u00e9\ud83d\ude00","c":{}}"#;
        let v = Value::parse(src).unwrap();
        assert_eq!(v.get("b").unwrap().as_str().unwrap(), "x\"y\\z\né😀");
        assert_eq!(v.at(&["a"]).unwrap().as_array().unwrap()[1].as_f64(), Some(-2500.0));
        let again = Value::parse(&v.to_json()).unwrap();
        assert_eq!(v, again);
        assert_eq!(Value::parse(&v.pretty()).unwrap(), v);
    }

    #[test]
    fn numbers_keep_source_text() {
        // Exponents as Python/JS write them (serde_json would normalise `1e5`/`2E-3` to `1e+5`/`2e-3`).
        let v = Value::parse(r#"[1.50, 12345678901234567890123, -0, 1e+20, 1e-07, 1.5e-10]"#).unwrap();
        assert_eq!(v.to_json(), "[1.50,12345678901234567890123,-0,1e+20,1e-07,1.5e-10]");
        let e = Value::parse("{\n  \"a\": x}").unwrap_err();
        assert_eq!(e.at, 9);
    }

    #[test]
    fn rejects_garbage() {
        for bad in ["", "{", "[1,]", "{\"a\" 1}", "tru", "\"\\x\"", "1 2", "01x"] {
            assert!(Value::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn writes_control_chars() {
        let mut s = String::new();
        write_str(&mut s, "a\u{1}b");
        assert_eq!(s, "\"a\\u0001b\"");
    }
}
