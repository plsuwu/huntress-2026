//! JScript specimens: `recover_*` loads a byte source (int arrays, packed strings) and runs a
//! `decode_*` function N times; `inspect_*` evals the result. The decoder body is evaluated with a
//! tiny integer-expression evaluator (no JS engine). Port of `expansion_pack.py`'s JScript part.

use super::descriptor::Plan;
use crate::json::Value;
use crate::re;
use std::collections::HashMap;

#[derive(Clone, Debug)]
enum V {
    Int(i64),
    List(Vec<i64>),
}

/// (kind, name, body, full text) of the first function bound to `prefix*`.
fn js_function<'a>(text: &'a str, prefix: &str) -> Result<(&'static str, String, &'a str, &'a str), String> {
    let assigned = crate::rx::Regex::new(&format!(r"(?:var\s+)?\b({prefix}\w*)\s*=\s*function\s*(\w*)\s*\(")).unwrap();
    let declared = crate::rx::Regex::new(&format!(r"\bfunction\s+({prefix}\w*)\s*\(")).unwrap();
    let (kind, name, start, after) = if let Some(m) = assigned.search(text) {
        (if m.g(2).is_empty() { "anonymous_expression" } else { "named_expression" }, m.g(1).to_string(), m.start(), m.end())
    } else {
        let m = declared.search(text).ok_or_else(|| format!("no {prefix}_ function"))?;
        ("declaration", m.g(1).to_string(), m.start(), m.end())
    };
    let open = after + text[after..].find('{').ok_or("no function body")?;
    let b = text.as_bytes();
    let (mut depth, mut i, mut quote) = (0i32, open, None::<u8>);
    while i < b.len() {
        let c = b[i];
        if let Some(q) = quote {
            if c == b'\\' {
                i += 1;
            } else if c == q {
                quote = None;
            }
        } else if c == b'"' || c == b'\'' {
            quote = Some(c);
        } else if c == b'{' {
            depth += 1;
        } else if c == b'}' {
            depth -= 1;
            if depth == 0 {
                break;
            }
        }
        i += 1;
    }
    let end = i.min(text.len().saturating_sub(1));
    Ok((kind, name, &text[open + 1..end], &text[start..(end + 1).min(text.len())]))
}

fn parse_int0(s: &str) -> Option<i64> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()
    } else {
        s.parse().ok()
    }
}

fn num_list(s: &str) -> Vec<i64> {
    re!(r"\s*,\s*").split(s.trim()).into_iter().filter(|x| !x.trim().is_empty()).filter_map(parse_int0).collect()
}

struct Expr<'e> {
    toks: Vec<String>,
    i: usize,
    env: &'e HashMap<String, V>,
}

const PREC: &[&[&str]] = &[&["|"], &["^"], &["&"], &["<<", ">>", ">>>"], &["+", "-"], &["*", "/", "%"]];

impl<'e> Expr<'e> {
    fn new(src: &str, env: &'e HashMap<String, V>) -> Result<Self, String> {
        let mut toks = Vec::new();
        let t = re!(r"\s*(0x[0-9a-fA-F]+|\d+|[A-Za-z_]\w*|<<|>>>|>>|[-+*/%&|^~()\[\].?:])");
        let src = src.trim();
        let mut pos = 0;
        while pos < src.len() {
            let m = t.match_at(src, pos).ok_or_else(|| format!("js expr token at {:?}", &src[pos..src.len().min(pos + 20)]))?;
            toks.push(m.g(1).to_string());
            pos = m.end();
            if m.end() == m.start() {
                break;
            }
        }
        Ok(Expr { toks, i: 0, env })
    }
    fn peek(&self) -> Option<&str> {
        self.toks.get(self.i).map(String::as_str)
    }
    fn take(&mut self) -> Option<String> {
        let t = self.toks.get(self.i).cloned();
        self.i += 1;
        t
    }
    fn parse(mut self) -> Result<i64, String> {
        let v = self.binary(0)?;
        if self.i != self.toks.len() {
            return Err("trailing js expr tokens".into());
        }
        v.as_int()
    }
    fn binary(&mut self, level: usize) -> Result<V, String> {
        if level == PREC.len() {
            return self.unary();
        }
        let mut left = self.binary(level + 1)?;
        while let Some(op) = self.peek().filter(|t| PREC[level].contains(t)).map(str::to_string) {
            self.i += 1;
            let right = self.binary(level + 1)?;
            left = V::Int(apply(&op, left.as_int()?, right.as_int()?)?);
        }
        Ok(left)
    }
    fn unary(&mut self) -> Result<V, String> {
        match self.peek() {
            Some("-") => {
                self.i += 1;
                Ok(V::Int(-self.unary()?.as_int()?))
            }
            Some("~") => {
                self.i += 1;
                Ok(V::Int(!self.unary()?.as_int()?))
            }
            _ => self.postfix(),
        }
    }
    fn postfix(&mut self) -> Result<V, String> {
        let t = self.take().ok_or("eof in expr")?;
        if t == "(" {
            let v = self.binary(0)?;
            if self.take().as_deref() != Some(")") {
                return Err("paren".into());
            }
            return Ok(v);
        }
        if let Some(n) = parse_int0(&t).filter(|_| t.chars().next().unwrap().is_ascii_digit()) {
            return Ok(V::Int(n));
        }
        let mut v = self.env.get(&t).cloned().ok_or_else(|| format!("unknown js identifier {t}"))?;
        while matches!(self.peek(), Some("[") | Some(".")) {
            if self.take().as_deref() == Some("[") {
                let idx = self.binary(0)?.as_int()?;
                if self.take().as_deref() != Some("]") {
                    return Err("bracket".into());
                }
                v = match v {
                    V::List(l) => V::Int(if idx >= 0 && (idx as usize) < l.len() { l[idx as usize] } else { 0 }),
                    V::Int(_) => return Err("index into a number".into()),
                };
            } else {
                let prop = self.take().unwrap_or_default();
                if prop != "length" {
                    return Err(format!("property {prop}"));
                }
                v = V::Int(match v {
                    V::List(l) => l.len() as i64,
                    V::Int(_) => return Err("length of a number".into()),
                });
            }
        }
        Ok(v)
    }
}

impl V {
    fn as_int(&self) -> Result<i64, String> {
        match self {
            V::Int(i) => Ok(*i),
            V::List(_) => Err("array used as a number".into()),
        }
    }
}

fn apply(op: &str, a: i64, b: i64) -> Result<i64, String> {
    Ok(match op {
        "|" => a | b,
        "^" => a ^ b,
        "&" => a & b,
        "<<" => (a << (b & 31)) & 0xFFFF_FFFF,
        ">>" => a >> (b & 31),
        ">>>" => (a & 0xFFFF_FFFF) >> (b & 31),
        "+" => a + b,
        "-" => a - b,
        "*" => a * b,
        // Python floor division (the reference evaluated with //).
        "/" => {
            if b == 0 {
                0
            } else {
                let q = a / b;
                if a % b != 0 && ((a < 0) != (b < 0)) { q - 1 } else { q }
            }
        }
        "%" => {
            if b == 0 {
                return Err("modulo by zero".into());
            }
            let r = a.abs() % b.abs();
            if a < 0 { -r } else { r }
        }
        _ => return Err(op.to_string()),
    })
}

fn eval(src: &str, env: &HashMap<String, V>) -> Result<i64, String> {
    Expr::new(src, env)?.parse()
}

fn split_statements(body: &str) -> Vec<String> {
    let (mut out, mut cur, mut quote, mut depth) = (Vec::new(), String::new(), None::<char>, 0i32);
    for c in body.chars() {
        if let Some(q) = quote {
            cur.push(c);
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        if c == ';' && depth == 0 {
            out.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn match_block(text: &str, open: usize) -> Result<usize, String> {
    let mut depth = 0;
    for (i, c) in text[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(open + i);
                }
            }
            _ => {}
        }
    }
    Err("unbalanced braces".into())
}

/// Evaluates a `decode_*` function over `data`.
fn run_decoder(src: &str, data: &[i64]) -> Result<Vec<i64>, String> {
    let m = re!(r"function\s*(?:\w+)?\s*\(\s*(\w+)\s*\)\s*\{").search(src).ok_or("decoder signature")?;
    let param = m.g(1).to_string();
    let body = &src[m.end()..src.len() - 1];
    let mut env: HashMap<String, V> = HashMap::new();
    env.insert(param.clone(), V::List(data.to_vec()));
    let mut out_name: Option<String> = None;
    let slice0 = crate::rx::Regex::new(&format!(r"{}\.slice\(\s*0\s*\)", regex_escape(&param))).unwrap();
    for d in re!(r"var\s+(\w+)\s*=\s*([^;]+);").find_iter(body) {
        let (name, init) = (d.g(1).to_string(), d.g(2).trim());
        if init == "[]" {
            out_name.get_or_insert(name.clone());
            env.insert(name, V::List(Vec::new()));
        } else if re!(r"\[[\d\sx,a-fA-F]*\]").fullmatch(init).is_some() {
            env.insert(name, V::List(num_list(&init[1..init.len() - 1])));
        } else if re!(r"-?(0x[0-9a-fA-F]+|\d+)").fullmatch(init).is_some() {
            let neg = init.starts_with('-');
            let v = parse_int0(init.trim_start_matches('-')).unwrap_or(0);
            env.insert(name, V::Int(if neg { -v } else { v }));
        } else if slice0.fullmatch(init).is_some() {
            env.insert(name.clone(), V::List(data.to_vec()));
            out_name = Some(name);
        }
    }
    let lp = re!(r"for\s*\(\s*var\s+(\w+)\s*=\s*0\s*;\s*\1\s*<\s*(\w+)\.length\s*;\s*\1\+\+\s*\)\s*\{").search(body);
    let rev_at = body.find(".reverse()");
    let mut result: Vec<i64>;
    if let Some(lp) = &lp {
        let (idx, src_name) = (lp.g(1).to_string(), lp.g(2).to_string());
        let lb = lp.end() - 1;
        let block = &body[lb + 1..match_block(body, lb)?];
        let stmts = split_statements(block);
        let pre_reverse = rev_at.is_some_and(|r| r < lp.start());
        if pre_reverse {
            let target = match &out_name {
                Some(o) if matches!(env.get(o), Some(V::List(l)) if !l.is_empty()) => o.clone(),
                _ => src_name.clone(),
            };
            if let Some(V::List(l)) = env.get_mut(&target) {
                l.reverse();
            }
        }
        let src_list = match env.get(&src_name) {
            Some(V::List(l)) => l.clone(),
            _ => data.to_vec(),
        };
        let out_key = out_name.clone().unwrap_or_else(|| "__out".into());
        env.entry(out_key.clone()).or_insert(V::List(Vec::new()));
        let mut outputs_name = out_key.clone();
        for i in 0..src_list.len() {
            env.insert(idx.clone(), V::Int(i as i64));
            for st in &stmts {
                let pm = re!(r"(\w+)\.push\((.*)\)", "s").fullmatch(st);
                let im = re!(r"(\w+)\[([^\]]+)\]\s*=\s*(.*)", "s").fullmatch(st);
                let am = re!(r"(?:var\s+)?(\w+)\s*=\s*(.*)", "s").fullmatch(st);
                if let Some(im) = im.filter(|im| matches!(env.get(im.g(1)), Some(V::List(_)))) {
                    let pos = eval(im.g(2), &env)?;
                    let val = eval(im.g(3), &env)? & 0xFF;
                    if let Some(V::List(arr)) = env.get_mut(im.g(1)) {
                        if pos < 0 || pos as usize >= arr.len() {
                            return Err("indexed assignment out of range".into());
                        }
                        arr[pos as usize] = val;
                    }
                } else if let Some(pm) = pm {
                    let val = eval(pm.g(2), &env)? & 0xFF;
                    match env.entry(pm.g(1).to_string()).or_insert(V::List(Vec::new())) {
                        V::List(l) => l.push(val),
                        V::Int(_) => return Err("push onto a number".into()),
                    }
                    outputs_name = pm.g(1).to_string();
                } else if let Some(am) = am {
                    let v = eval(am.g(2), &env)?;
                    env.insert(am.g(1).to_string(), V::Int(v));
                } else {
                    return Err(format!("decoder statement {:?}", st.chars().take(40).collect::<String>()));
                }
            }
        }
        result = match env.get(&outputs_name) {
            Some(V::List(l)) => l.clone(),
            _ => Vec::new(),
        };
    } else if let Some(o) = &out_name {
        result = match env.get(o) {
            Some(V::List(l)) => l.clone(),
            _ => Vec::new(),
        };
    } else {
        return Err("decoder without loop".into());
    }
    if let Some(r) = rev_at {
        if !lp.as_ref().is_some_and(|lp| r < lp.start()) {
            result.reverse();
        }
    }
    Ok(result)
}

fn regex_escape(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == '_' { c.to_string() } else { format!("\\{c}") }).collect()
}

fn alphabet(text: &str) -> String {
    re!(r#"alphabet\s*=\s*"([^"]{64})""#).search(text).map(|m| m.g(1).to_string()).unwrap_or_else(|| "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".into())
}

fn b64_custom(s: &str, alpha: &str) -> Vec<i64> {
    let a: Vec<char> = alpha.chars().collect();
    let clean: Vec<char> = s.chars().filter(|c| a.contains(c) || *c == '=').collect();
    let mut out = Vec::new();
    for chunk in clean.chunks(4) {
        let idx: Vec<i64> = (0..4).map(|k| chunk.get(k).map(|c| if *c == '=' { -1 } else { a.iter().position(|x| x == c).map(|p| p as i64).unwrap_or(-1) }).unwrap_or(-1)).collect();
        let (a0, b0, c0, d0) = (idx[0], idx[1], idx[2], idx[3]);
        out.push(((a0 << 2) | (b0 >> 4)) & 255);
        if c0 >= 0 {
            out.push((((b0 & 15) << 4) | (c0 >> 2)) & 255);
        }
        if d0 >= 0 {
            out.push((((c0 & 3) << 6) | d0) & 255);
        }
    }
    out
}

/// Byte source of the `recover_*` function.
fn text_data(recover: &str, whole: &str) -> Result<Vec<i64>, String> {
    let mut arrays: Vec<(String, Vec<i64>)> = Vec::new();
    for m in re!(r"var\s+(\w+)\s*=\s*\[([\d\sx,a-fA-F]*)\](?!\s*\.\s*slice)\s*;").find_iter(recover) {
        arrays.push((m.g(1).to_string(), num_list(m.g(2))));
    }
    let mut strings: Vec<(String, String)> = Vec::new();
    for m in re!(r#"var\s+(\w+)\s*=\s*((?:"[^"\n]*"\s*\+\s*)*"[^"\n]*")\s*;"#).find_iter(recover) {
        strings.push((m.g(1).to_string(), re!(r#""([^"\n]*)""#).findall(m.g(2)).concat()));
    }
    let get = |n: &str| arrays.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone());
    if let Some(c) = re!(r"var\s+\w+\s*=\s*(\w+)((?:\s*\.concat\(\s*\w+(?:\s*,\s*\w+)*\s*\))+)").search(recover) {
        let mut names = vec![c.g(1).to_string()];
        names.extend(re!(r"\w+").findall(&c.g(2).replace("concat", "")).into_iter().map(str::to_string));
        if names.iter().all(|n| get(n).is_some()) {
            return Ok(names.iter().flat_map(|n| get(n).unwrap()).collect());
        }
    }
    let big: Vec<&(String, Vec<i64>)> = arrays.iter().filter(|(_, v)| v.len() >= 16).collect();
    if big.len() == 1 {
        return Ok(big[0].1.clone());
    }
    if big.len() > 1 {
        return Err(format!("ambiguous byte arrays {:?}", big.iter().map(|x| &x.0).collect::<Vec<_>>()));
    }
    let looks_packed = |v: &str| -> bool {
        re!(r"(?:%[0-9a-fA-F]{2})+").fullmatch(v).is_some()
            || re!(r"[0-9A-Za-z]{1,3}([^\w\s])[0-9A-Za-z]{1,3}(?:\1[0-9A-Za-z]{1,3})+").fullmatch(v).is_some()
            || re!(r"(?:[0-9a-fA-F]{2})+").fullmatch(v).is_some()
            || (v.len() % 4 == 0 && re!(r"[A-Za-z0-9+/]+={0,2}").fullmatch(v).is_some())
    };
    let s = strings.iter().filter(|(_, v)| v.len() >= 16 && looks_packed(v)).map(|(_, v)| v.clone()).max_by_key(|v| v.len()).ok_or("cannot locate packed byte source")?;
    if re!(r"(?:%[0-9a-fA-F]{2})+").fullmatch(&s).is_some() {
        return Ok((0..s.len()).step_by(3).map(|i| i64::from_str_radix(&s[i + 1..i + 3], 16).unwrap()).collect());
    }
    if let Some(sp) = re!(r#"\.split\(\s*"(.)"\s*\)"#).search(recover) {
        let sep = sp.g(1);
        if s.contains(sep) {
            let radix = re!(r"parseInt\([^)]*,\s*(\d+)\s*\)").search(recover).and_then(|m| m.g(1).parse().ok()).unwrap_or(10);
            return s.split(sep).filter(|t| !t.is_empty()).map(|t| i64::from_str_radix(t, radix).map(|v| v & 0xFF).map_err(|e| e.to_string())).collect();
        }
    }
    if re!(r"(?:[0-9a-fA-F]{2})+").fullmatch(&s).is_some() && recover.contains("parseInt") {
        return Ok(crate::codec::hex(&s).ok_or("bad hex")?.into_iter().map(i64::from).collect());
    }
    Ok(b64_custom(&s, &alphabet(whole)))
}

/// JSON, or a JS object literal with unquoted keys / single quotes.
fn parse_js_object(text: &str) -> Result<Value, String> {
    if let Ok(v) = Value::parse(text) {
        return Ok(v);
    }
    let t = re!(r"([{,]\s*)([A-Za-z_]\w*)\s*:").sub(text, r#"\1"\2":"#);
    let t = re!(r"'((?:[^'\\]|\\.)*)'").replace_all(&t, |m| {
        let mut s = String::new();
        crate::json::write_str(&mut s, m.g(1));
        s
    });
    Value::parse(&t).map_err(|e| format!("descriptor is not JSON: {e}"))
}

#[derive(Clone, Debug)]
pub struct Js {
    pub plan: Plan,
    pub function_style: &'static str,
    pub invocation_style: String,
    pub property_style: &'static str,
}

fn value_text(v: &Value) -> String {
    v.as_str().map(str::to_string).unwrap_or_else(|| v.to_json())
}

pub fn analyse(text: &str) -> Result<Js, String> {
    let (_, rname, rbody, _) = js_function(text, "recover")?;
    let (kind, _iname, ibody, _) = js_function(text, "inspect")?;
    let data = text_data(rbody, text)?;
    let dname = re!(r"\b(decode_\w+)\b").search(rbody).ok_or("no decoder call in recover function")?.g(1).to_string();
    let (_, _, _, dfull) = js_function(text, &dname)?;
    let mut passes = 1;
    let mut chained = false;
    if let Some(pm) = re!(r"for\s*\(\s*var\s+(\w+)\s*=\s*0\s*;\s*\1\s*<\s*(\d+)\s*;").search(rbody) {
        passes = pm.g(2).parse().unwrap_or(1);
        let call = crate::rx::Regex::new(&format!(r"(\w+)\s*=\s*{}(?:\.call\(\s*null\s*,\s*|\.apply\(\s*null\s*,\s*\[\s*|\(\s*)(\w+)", regex_escape(&dname))).unwrap();
        chained = call.search(&rbody[pm.end()..]).is_some_and(|c| c.g(1) == c.g(2));
    }
    let call_at = crate::rx::Regex::new(&format!(r"\b{}\b", regex_escape(&dname))).unwrap().search(rbody).map(|m| m.start());
    let rev = re!(r"\.reverse\s*\(\s*\)").search(rbody).map(|m| m.start());
    let mut out = data;
    if let (Some(r), Some(c)) = (rev, call_at) {
        if r < c {
            out.reverse();
        }
    }
    for _ in 0..if chained { passes } else { 1 } {
        out = run_decoder(dfull, &out)?;
    }
    if let (Some(r), Some(c)) = (rev, call_at) {
        if r > c {
            out.reverse();
        }
    }
    let descriptor_text: String = out.iter().map(|&b| char::from_u32(b as u32).unwrap_or('\u{fffd}')).collect();
    let d = parse_js_object(&descriptor_text)?;
    let steps: Vec<String> = d.get("steps").and_then(Value::as_array).ok_or("descriptor has no steps")?.iter().map(value_text).collect();
    let plan = Plan {
        steps,
        adapter: d.str_of("adapter").ok_or("descriptor has no adapter")?.to_string(),
        stage_count: d.get("stageCount").and_then(Value::as_i64),
        graph: None,
        values: d.get("values").and_then(Value::as_object).map(|m| m.iter().map(|(k, v)| (k.clone(), value_text(v))).collect()).unwrap_or_default(),
    };
    let rn = regex_escape(&rname);
    let em = re!(r"var\s+(\w+)\s*=\s*eval\s*\(").search(ibody).map(|m| m.g(1).to_string()).or_else(|| {
        crate::rx::Regex::new(&format!(r"var\s+(\w+)\s*=\s*[^;]*\b{rn}\b")).unwrap().search(ibody).map(|m| m.g(1).to_string())
    });
    let var = regex_escape(&em.ok_or("no descriptor variable in inspect function")?);
    let fields = r"(?:steps|adapter|values|stageCount)";
    let dots = crate::rx::Regex::new(&format!(r"\b{var}\s*\.\s*{fields}\b")).unwrap().find_iter(ibody).len();
    let brackets = crate::rx::Regex::new(&format!(r#"\b{var}\s*\[\s*["']{fields}["']\s*\]"#)).unwrap().find_iter(ibody).len();
    let property_style = match (dots > 0, brackets > 0) {
        (true, true) => "mixed",
        (true, false) => "dot",
        (false, true) => "bracket_literal",
        _ => return Err("no descriptor property accesses".into()),
    };
    let call = crate::rx::Regex::new(&format!(r"\b{rn}\b\s*(\.\s*call\b|\.\s*apply\b|\()")).unwrap().search(ibody).ok_or("recover call not found in inspect")?;
    let invocation_style = if call.g(1) == "(" { "direct".to_string() } else { call.g(1).replace('.', "").trim().to_string() };
    Ok(Js { plan, function_style: kind, invocation_style, property_style })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_decoder() {
        let desc = r#"{"steps":["acquire_remote","dispatch_memory"],"adapter":"WinHttp","values":{"campaign_id":"c"},"stageCount":1}"#;
        let arr: Vec<String> = desc.bytes().map(|b| b.to_string()).collect();
        let t = format!(
            "function decode_1(v_a) {{\n  var v_b = v_a.slice(0);\n  return v_b;\n}}\nfunction recover_2() {{\n  var v_a = [{}];\n  var v_b = [];\n  for (var pass = 0; pass < 1; pass++) {{ v_b = decode_1(v_a); }}\n  return v_b;\n}}\nfunction inspect_3() {{\n  var v_d = eval(\"(\" + recover_2() + \")\");\n  return {{ steps: v_d.steps, adapter: v_d[\"adapter\"] }};\n}}\n",
            arr.join(",")
        );
        let js = analyse(&t).unwrap();
        assert_eq!(js.plan.steps, ["acquire_remote", "dispatch_memory"]);
        assert_eq!((js.function_style, js.invocation_style.as_str(), js.property_style), ("declaration", "direct", "mixed"));
        assert_eq!(js.plan.values["campaign_id"], "c");
        // A per-byte xor loop.
        let enc: Vec<String> = desc.bytes().map(|b| (b ^ 0x21).to_string()).collect();
        let t2 = t.replace(&arr.join(","), &enc.join(",")).replace("var v_b = v_a.slice(0);", "var v_b = []; for (var i = 0; i < v_a.length; i++) { v_b.push(v_a[i] ^ 33); }");
        assert_eq!(analyse(&t2).unwrap().plan.adapter, "WinHttp");
    }
}
