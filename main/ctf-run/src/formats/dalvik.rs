//! A small Dalvik bytecode interpreter over `formats::dex`: integer/long/float arithmetic, branches, switches,
//! arrays, static and instance fields of the DEX's own classes (`<clinit>` runs on first use), and a handful of
//! `java.lang` builtins (String, StringBuilder, Long/Integer hex and rotate helpers, Math). Any other external
//! call is an error - the interpreter never guesses what a framework API returns.

use super::dex::{Dex, Encoded, ACC_STATIC};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Val {
    #[default]
    Null,
    N(u32),
    Ref(usize),
}

impl Val {
    fn n(self) -> u32 {
        match self {
            Val::N(x) => x,
            _ => 0,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Obj {
    Str(String),
    Builder(String),
    /// Elements as raw bits; `kind` is the element descriptor (`I`, `J`, `B`, `C`, `Ljava/lang/String;`, ..).
    Arr(String, Vec<u64>),
    Inst(String, HashMap<String, (Val, Val)>),
    Boxed(char, u64),
    /// A `java.lang.Class` by descriptor (`J` for `Long.TYPE`).
    Class(String),
    Method(String, String, Vec<String>),
    Field(String, String),
    Loader,
    Assets,
    Stream(Vec<u8>, usize),
    Buffer(Vec<u8>),
}

pub struct Vm<'a> {
    dexes: Vec<Rc<Dex>>,
    pub heap: Vec<Obj>,
    statics: HashMap<(String, String), (Val, Val)>,
    inited: HashSet<String>,
    /// Class descriptor -> (dex, class index); the first definition wins (parent-first loading).
    classes: HashMap<String, (usize, usize)>,
    pub steps: u64,
    pub limit: u64,
    depth: u32,
    /// Native method implementation: `(JNI symbol, integer arguments after env/class) -> result`.
    pub native: Option<&'a dyn Fn(&str, &[u64]) -> R<u64>>,
    /// APK member reader (`assets/...`) for `AssetManager.open` and file-based class loaders.
    pub files: Option<&'a dyn Fn(&str) -> Option<Vec<u8>>>,
}

pub const ACC_NATIVE: u32 = 0x100;

/// The JNI short symbol name of `class->method` (`Java_` + mangled class + `_` + mangled method).
pub fn jni_name(class: &str, method: &str) -> String {
    let mangle = |s: &str| -> String {
        s.chars()
            .map(|c| match c {
                '/' => "_".to_string(),
                '_' => "_1".to_string(),
                ';' => "_2".to_string(),
                '[' => "_3".to_string(),
                c if c.is_ascii_alphanumeric() => c.to_string(),
                c => format!("_0{:04x}", c as u32),
            })
            .collect()
    };
    let cls = class.strip_prefix('L').and_then(|c| c.strip_suffix(';')).unwrap_or(class);
    format!("Java_{}_{}", mangle(cls), mangle(method))
}

type R<T> = Result<T, String>;

fn wide(lo: Val, hi: Val) -> u64 {
    lo.n() as u64 | (hi.n() as u64) << 32
}

fn split(v: u64) -> (Val, Val) {
    (Val::N(v as u32), Val::N((v >> 32) as u32))
}

fn is_wide(t: &str) -> bool {
    t == "J" || t == "D"
}

fn is_ref(t: &str) -> bool {
    t.starts_with('L') || t.starts_with('[')
}

/// Java's `String.hashCode`.
fn java_hash(s: &str) -> i32 {
    s.encode_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
}

fn radix(mut v: u64, bits: u32, shift: u32) -> String {
    if bits < 64 {
        v &= (1u64 << bits) - 1;
    }
    let digits = b"0123456789abcdef";
    let mask = (1u64 << shift) - 1;
    let mut out = Vec::new();
    loop {
        out.push(digits[(v & mask) as usize]);
        v >>= shift;
        if v == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).unwrap()
}

fn to_radix(v: i64, r: u32) -> String {
    if !(2..=36).contains(&r) || r == 10 {
        return v.to_string();
    }
    let neg = v < 0;
    let mut u = v.unsigned_abs();
    let mut out = Vec::new();
    loop {
        out.push(std::char::from_digit((u % r as u64) as u32, r).unwrap());
        u /= r as u64;
        if u == 0 {
            break;
        }
    }
    if neg {
        out.push('-');
    }
    out.iter().rev().collect()
}

fn fbits(v: u32) -> f32 {
    f32::from_bits(v)
}
fn dbits(v: u64) -> f64 {
    f64::from_bits(v)
}

fn set_reg(v: &mut [Val], i: usize, x: Val) -> R<()> {
    *v.get_mut(i).ok_or_else(|| format!("register v{i} out of range"))? = x;
    Ok(())
}

fn set_wide(v: &mut [Val], i: usize, x: u64) -> R<()> {
    let (lo, hi) = split(x);
    set_reg(v, i, lo)?;
    set_reg(v, i + 1, hi)
}

macro_rules! set {
    ($v:ident, $i:expr, $x:expr) => {{
        let (i, x) = ($i, $x);
        set_reg(&mut $v, i, x)
    }};
}

macro_rules! setw {
    ($v:ident, $i:expr, $x:expr) => {{
        let (i, x) = ($i, $x);
        set_wide(&mut $v, i, x)
    }};
}

impl<'a> Vm<'a> {
    pub fn new(dex: Rc<Dex>) -> Self {
        let mut vm = Vm { dexes: Vec::new(), heap: Vec::new(), statics: HashMap::new(), inited: HashSet::new(), classes: HashMap::new(), steps: 0, limit: 50_000_000, depth: 0, native: None, files: None };
        vm.load(dex);
        vm
    }

    /// Adds a DEX to the class path; returns its index.
    pub fn load(&mut self, dex: Rc<Dex>) -> usize {
        let d = self.dexes.len();
        for (i, c) in dex.classes.iter().enumerate() {
            self.classes.entry(c.name.clone()).or_insert((d, i));
        }
        self.dexes.push(dex);
        d
    }

    pub fn alloc(&mut self, o: Obj) -> Val {
        self.heap.push(o);
        Val::Ref(self.heap.len() - 1)
    }

    pub fn string(&self, v: Val) -> Option<&str> {
        match v {
            Val::Ref(i) => match &self.heap[i] {
                Obj::Str(s) | Obj::Builder(s) => Some(s),
                _ => None,
            },
            _ => None,
        }
    }

    fn obj(&mut self, v: Val) -> R<&mut Obj> {
        match v {
            Val::Ref(i) => Ok(&mut self.heap[i]),
            _ => Err("null dereference".into()),
        }
    }

    fn str_arg(&self, v: Val) -> String {
        match v {
            Val::Null => "null".into(),
            Val::Ref(i) => match &self.heap[i] {
                Obj::Str(s) | Obj::Builder(s) => s.clone(),
                Obj::Boxed('J', x) => (*x as i64).to_string(),
                Obj::Boxed('I', x) => (*x as i32).to_string(),
                Obj::Boxed(_, x) => x.to_string(),
                o => format!("{o:?}"),
            },
            Val::N(x) => x.to_string(),
        }
    }

    /// Runs `<clinit>` and applies `static_values` the first time a DEX class is touched.
    fn init_class(&mut self, name: &str) -> R<()> {
        let Some(&(d, ci)) = self.classes.get(name) else { return Ok(()) };
        if !self.inited.insert(name.to_string()) {
            return Ok(());
        }
        let dex = self.dexes[d].clone();
        let c = &dex.classes[ci];
        if let Some(s) = &c.superclass {
            self.init_class(s)?;
        }
        for (f, v) in &c.static_fields {
            let val = match v {
                Encoded::Prim(x) => split(*x),
                Encoded::Str(i) => (self.alloc(Obj::Str(dex.strings.get(*i as usize).cloned().unwrap_or_default())), Val::Null),
                _ => (Val::Null, Val::Null),
            };
            let fr = &dex.fields[*f as usize];
            self.statics.insert((fr.class.clone(), fr.name.clone()), val);
        }
        if let Some(m) = c.methods.iter().find(|m| dex.methods[m.idx as usize].name == "<clinit>") {
            self.call(d, m.idx, &[])?;
        }
        Ok(())
    }

    /// The method implementing `class->name(params)` (exact class, then up the superclass chain), with code or
    /// declared `native`: `(dex, method index, has code)`.
    fn find_method(&self, class: &str, name: &str, params: &[String]) -> Option<(usize, u32, bool)> {
        let mut cls = Some(class.to_string());
        while let Some(cn) = cls {
            let &(d, ci) = self.classes.get(&cn)?;
            let dex = &self.dexes[d];
            let c = &dex.classes[ci];
            for m in &c.methods {
                let mr = &dex.methods[m.idx as usize];
                if mr.name == name && dex.protos[mr.proto as usize].params == params && (m.code.is_some() || m.access & ACC_NATIVE != 0) {
                    return Some((d, m.idx, m.code.is_some()));
                }
            }
            cls = c.superclass.clone();
        }
        None
    }

    /// Calls method `idx` of dex `d` with raw argument slots (wide values take two). Returns the result pair.
    pub fn call(&mut self, d: usize, idx: u32, args: &[Val]) -> R<(Val, Val)> {
        let dex = self.dexes[d].clone();
        let r = &dex.methods[idx as usize];
        self.init_class(&r.class)?;
        let params = &dex.protos[r.proto as usize].params;
        match self.find_method(&r.class, &r.name, params) {
            Some((td, t, true)) => {
                let tdex = self.dexes[td].clone();
                let tr = &tdex.methods[t as usize];
                let c = &tdex.classes[self.classes[&tr.class].1];
                let m = c.methods.iter().find(|m| m.idx == t).unwrap();
                let code = m.code.as_ref().unwrap();
                if self.depth > 200 {
                    return Err("call depth exceeded".into());
                }
                self.depth += 1;
                let res = self.exec(td, code.registers as usize, code.ins as usize, &code.insns, args);
                self.depth -= 1;
                res.map_err(|e| format!("{}: {e}", tdex.method_sig(t)))
            }
            Some((td, t, false)) => self.call_native(td, t, args).map_err(|e| format!("{}: {e}", dex.method_sig(idx))),
            None => self.builtin(d, idx, args),
        }
    }

    /// A `native` method declared by a DEX class, run through `self.native` (primitive arguments only).
    fn call_native(&mut self, d: usize, idx: u32, args: &[Val]) -> R<(Val, Val)> {
        let dex = self.dexes[d].clone();
        let r = &dex.methods[idx as usize];
        let c = &dex.classes[self.classes[&r.class].1];
        let m = c.methods.iter().find(|m| m.idx == idx).ok_or("native method not found")?;
        let p = &dex.protos[r.proto as usize];
        let sym = jni_name(&r.class, &r.name);
        let Some(native) = self.native else { return Err(format!("native {sym} without a native library")) };
        let mut ints = Vec::new();
        let mut k = if m.access & ACC_STATIC == 0 { 1 } else { 0 };
        for t in &p.params {
            match t.as_str() {
                "J" => {
                    ints.push(wide(args[k], args[k + 1]));
                    k += 2
                }
                "I" | "S" | "B" | "C" | "Z" => {
                    let x = args[k].n();
                    ints.push(match t.as_str() {
                        "I" => x as i32 as i64 as u64,
                        _ => x as u64,
                    });
                    k += 1
                }
                _ => return Err(format!("native {sym}: unsupported parameter {t}")),
            }
        }
        native(&sym, &ints).and_then(|v| match p.ret.as_str() {
            "J" => Ok(split(v)),
            "I" | "S" | "B" | "C" | "Z" => Ok((Val::N(v as u32), Val::Null)),
            "V" => Ok((Val::Null, Val::Null)),
            t => Err(format!("native {sym}: unsupported return {t}")),
        })
    }

    fn builtin(&mut self, d: usize, idx: u32, a: &[Val]) -> R<(Val, Val)> {
        let dex = self.dexes[d].clone();
        let r = &dex.methods[idx as usize];
        let p = &dex.protos[r.proto as usize];
        let sig = format!("({}){}", p.params.concat(), p.ret);
        let none = (Val::Null, Val::Null);
        let int = |x: i32| (Val::N(x as u32), Val::Null);
        let long = |x: u64| split(x);
        let w = |i: usize| wide(a[i], a[i + 1]);
        let out = match (r.class.as_str(), r.name.as_str(), sig.as_str()) {
            ("Ljava/lang/Object;", "<init>", _) => none,
            // Native code is resolved by symbol through `self.native`.
            ("Ljava/lang/System;", "loadLibrary", "(Ljava/lang/String;)V") => none,
            ("Ljava/lang/StringBuilder;", "<init>", "()V" | "(I)V") => {
                *self.obj(a[0])? = Obj::Builder(String::new());
                none
            }
            ("Ljava/lang/StringBuilder;", "<init>", "(Ljava/lang/String;)V" | "(Ljava/lang/CharSequence;)V") => {
                let s = self.str_arg(a[1]);
                *self.obj(a[0])? = Obj::Builder(s);
                none
            }
            ("Ljava/lang/StringBuilder;", "append", _) => {
                let s = match p.params[0].as_str() {
                    "C" => char::from_u32(a[1].n() & 0xffff).unwrap_or('\u{fffd}').to_string(),
                    "I" | "S" | "B" => (a[1].n() as i32).to_string(),
                    "J" => (w(1) as i64).to_string(),
                    "Z" => (a[1].n() != 0).to_string(),
                    "F" => format!("{:?}", fbits(a[1].n())),
                    "D" => format!("{:?}", dbits(w(1))),
                    "[C" => self.chars_of(a[1])?,
                    _ => self.str_arg(a[1]),
                };
                match self.obj(a[0])? {
                    Obj::Builder(b) => b.push_str(&s),
                    _ => return Err("append on non-builder".into()),
                }
                (a[0], Val::Null)
            }
            ("Ljava/lang/StringBuilder;", "reverse", _) => {
                if let Obj::Builder(b) = self.obj(a[0])? {
                    *b = b.chars().rev().collect();
                }
                (a[0], Val::Null)
            }
            ("Ljava/lang/StringBuilder;" | "Ljava/lang/String;", "toString" | "intern", "()Ljava/lang/String;") => {
                let s = self.str_arg(a[0]);
                (self.alloc(Obj::Str(s)), Val::Null)
            }
            ("Ljava/lang/StringBuilder;" | "Ljava/lang/String;", "length", "()I") => int(self.str_arg(a[0]).encode_utf16().count() as i32),
            ("Ljava/lang/StringBuilder;" | "Ljava/lang/String;", "charAt", "(I)C") => {
                let s = self.str_arg(a[0]);
                let i = a[1].n() as i32;
                let c = usize::try_from(i).ok().and_then(|i| s.encode_utf16().nth(i)).ok_or_else(|| format!("charAt({i}) out of range"))?;
                int(c as i32)
            }
            ("Ljava/lang/String;", "equals", _) => {
                let eq = self.string(a[1]).is_some_and(|o| Some(o) == self.string(a[0]));
                int(eq as i32)
            }
            ("Ljava/lang/String;", "equalsIgnoreCase", _) => {
                let eq = self.string(a[1]).is_some_and(|o| self.string(a[0]).is_some_and(|s| s.to_lowercase() == o.to_lowercase()));
                int(eq as i32)
            }
            ("Ljava/lang/String;", "hashCode", "()I") => int(java_hash(&self.str_arg(a[0]))),
            ("Ljava/lang/String;", "isEmpty", "()Z") => int(self.str_arg(a[0]).is_empty() as i32),
            ("Ljava/lang/String;", n @ ("toUpperCase" | "toLowerCase" | "trim" | "concat" | "substring"), _) => {
                let s = self.str_arg(a[0]);
                let units: Vec<u16> = s.encode_utf16().collect();
                let res = match (n, sig.as_str()) {
                    ("toUpperCase", _) => s.to_uppercase(),
                    ("toLowerCase", _) => s.to_lowercase(),
                    ("trim", _) => s.trim_matches(|c: char| c <= ' ').to_string(),
                    ("concat", _) => s + &self.str_arg(a[1]),
                    ("substring", "(I)Ljava/lang/String;") => String::from_utf16_lossy(units.get(a[1].n() as usize..).ok_or("substring out of range")?),
                    ("substring", "(II)Ljava/lang/String;") => String::from_utf16_lossy(units.get(a[1].n() as usize..a[2].n() as usize).ok_or("substring out of range")?),
                    _ => return Err(format!("unsupported {}", dex.method_sig(idx))),
                };
                (self.alloc(Obj::Str(res)), Val::Null)
            }
            ("Ljava/lang/String;", "toCharArray", _) => {
                let v = self.str_arg(a[0]).encode_utf16().map(|c| c as u64).collect();
                (self.alloc(Obj::Arr("C".into(), v)), Val::Null)
            }
            ("Ljava/lang/String;", "getBytes", "()[B") => {
                let v = self.str_arg(a[0]).bytes().map(|c| c as i8 as i64 as u64).collect();
                (self.alloc(Obj::Arr("B".into(), v)), Val::Null)
            }
            ("Ljava/lang/String;", "<init>", "([C)V") => {
                let s = self.chars_of(a[1])?;
                *self.obj(a[0])? = Obj::Str(s);
                none
            }
            ("Ljava/lang/String;", "<init>", "([B)V") => {
                let b: Vec<u8> = match self.obj(a[1])? {
                    Obj::Arr(_, v) => v.iter().map(|&x| x as u8).collect(),
                    _ => return Err("String(byte[]) on non-array".into()),
                };
                *self.obj(a[0])? = Obj::Str(String::from_utf8_lossy(&b).into_owned());
                none
            }
            ("Ljava/lang/String;", "valueOf", _) => {
                let s = match p.params[0].as_str() {
                    "C" => char::from_u32(a[0].n() & 0xffff).unwrap_or('\u{fffd}').to_string(),
                    "I" => (a[0].n() as i32).to_string(),
                    "J" => (w(0) as i64).to_string(),
                    "Z" => (a[0].n() != 0).to_string(),
                    "[C" => self.chars_of(a[0])?,
                    _ => self.str_arg(a[0]),
                };
                (self.alloc(Obj::Str(s)), Val::Null)
            }
            ("Ljava/lang/Long;", "toHexString", "(J)Ljava/lang/String;") => (self.alloc(Obj::Str(radix(w(0), 64, 4))), Val::Null),
            ("Ljava/lang/Long;", "toBinaryString", "(J)Ljava/lang/String;") => (self.alloc(Obj::Str(radix(w(0), 64, 1))), Val::Null),
            ("Ljava/lang/Long;", "toOctalString", "(J)Ljava/lang/String;") => (self.alloc(Obj::Str(radix(w(0), 64, 3))), Val::Null),
            ("Ljava/lang/Integer;", "toHexString", "(I)Ljava/lang/String;") => (self.alloc(Obj::Str(radix(a[0].n() as u64, 32, 4))), Val::Null),
            ("Ljava/lang/Integer;", "toBinaryString", "(I)Ljava/lang/String;") => (self.alloc(Obj::Str(radix(a[0].n() as u64, 32, 1))), Val::Null),
            ("Ljava/lang/Long;", "toString", "(J)Ljava/lang/String;") => (self.alloc(Obj::Str((w(0) as i64).to_string())), Val::Null),
            ("Ljava/lang/Long;", "toString", "(JI)Ljava/lang/String;") => (self.alloc(Obj::Str(to_radix(w(0) as i64, a[2].n()))), Val::Null),
            ("Ljava/lang/Integer;", "toString", "(I)Ljava/lang/String;") => (self.alloc(Obj::Str((a[0].n() as i32).to_string())), Val::Null),
            ("Ljava/lang/Integer;", "toString", "(II)Ljava/lang/String;") => (self.alloc(Obj::Str(to_radix(a[0].n() as i32 as i64, a[1].n()))), Val::Null),
            ("Ljava/lang/Long;", "toUnsignedString", "(J)Ljava/lang/String;") => (self.alloc(Obj::Str(w(0).to_string())), Val::Null),
            ("Ljava/lang/Long;", "rotateLeft", "(JI)J") => long(w(0).rotate_left(a[2].n() & 63)),
            ("Ljava/lang/Long;", "rotateRight", "(JI)J") => long(w(0).rotate_right(a[2].n() & 63)),
            ("Ljava/lang/Integer;", "rotateLeft", "(II)I") => int(a[0].n().rotate_left(a[1].n() & 31) as i32),
            ("Ljava/lang/Integer;", "rotateRight", "(II)I") => int(a[0].n().rotate_right(a[1].n() & 31) as i32),
            ("Ljava/lang/Long;", "reverse", "(J)J") => long(w(0).reverse_bits()),
            ("Ljava/lang/Long;", "reverseBytes", "(J)J") => long(w(0).swap_bytes()),
            ("Ljava/lang/Integer;", "reverse", "(I)I") => int(a[0].n().reverse_bits() as i32),
            ("Ljava/lang/Integer;", "reverseBytes", "(I)I") => int(a[0].n().swap_bytes() as i32),
            ("Ljava/lang/Long;", "bitCount", "(J)I") => int(w(0).count_ones() as i32),
            ("Ljava/lang/Integer;", "bitCount", "(I)I") => int(a[0].n().count_ones() as i32),
            ("Ljava/lang/Long;", "valueOf", "(J)Ljava/lang/Long;") => (self.alloc(Obj::Boxed('J', w(0))), Val::Null),
            ("Ljava/lang/Integer;", "valueOf", "(I)Ljava/lang/Integer;") => (self.alloc(Obj::Boxed('I', a[0].n() as u64)), Val::Null),
            ("Ljava/lang/Long;" | "Ljava/lang/Integer;" | "Ljava/lang/Number;", "longValue" | "intValue", _) => {
                let x = match self.obj(a[0])? {
                    Obj::Boxed('I', x) => *x as u32 as i32 as i64 as u64,
                    Obj::Boxed(_, x) => *x,
                    _ => return Err("unboxing a non-number".into()),
                };
                if p.ret == "J" {
                    long(x)
                } else {
                    int(x as i32)
                }
            }
            ("Ljava/lang/Long;", "parseLong", _) | ("Ljava/lang/Integer;", "parseInt", _) => {
                let s = self.str_arg(a[0]);
                let rdx = if p.params.len() > 1 { a[1].n() } else { 10 };
                let v = i64::from_str_radix(&s, rdx).map_err(|e| format!("parse {s:?}: {e}"))?;
                if p.ret == "J" {
                    long(v as u64)
                } else {
                    int(v as i32)
                }
            }
            ("Ljava/lang/Math;", "abs", "(I)I") => int((a[0].n() as i32).wrapping_abs()),
            ("Ljava/lang/Math;", "abs", "(J)J") => long((w(0) as i64).wrapping_abs() as u64),
            ("Ljava/lang/Math;", "min", "(II)I") => int((a[0].n() as i32).min(a[1].n() as i32)),
            ("Ljava/lang/Math;", "max", "(II)I") => int((a[0].n() as i32).max(a[1].n() as i32)),
            ("Ljava/lang/Math;", "min", "(JJ)J") => long((w(0) as i64).min(w(2) as i64) as u64),
            ("Ljava/lang/Math;", "max", "(JJ)J") => long((w(0) as i64).max(w(2) as i64) as u64),
            _ => match self.reflect(r.class.as_str(), r.name.as_str(), &sig, a)? {
                Some(x) => x,
                None => return Err(format!("unsupported external call {}", dex.method_sig(idx))),
            },
        };
        Ok(out)
    }

    fn bytes_of(&mut self, v: Val) -> R<Vec<u8>> {
        match self.obj(v)? {
            Obj::Arr(_, x) => Ok(x.iter().map(|&b| b as u8).collect()),
            Obj::Buffer(b) => Ok(b.clone()),
            _ => Err("expected byte[]".into()),
        }
    }

    /// The references stored in an object array.
    fn refs_of(&mut self, v: Val) -> R<Vec<Val>> {
        if v == Val::Null {
            return Ok(Vec::new());
        }
        match self.obj(v)? {
            Obj::Arr(_, x) => Ok(x.iter().map(|&e| if e >> 63 == 1 { Val::Ref((e & !(1 << 63)) as usize) } else { Val::Null }).collect()),
            _ => Err("expected Object[]".into()),
        }
    }

    fn class_of(&mut self, v: Val) -> R<String> {
        match self.obj(v)? {
            Obj::Class(c) => Ok(c.clone()),
            _ => Err("expected a Class".into()),
        }
    }

    fn bytes_arr(&mut self, b: &[u8]) -> Val {
        self.alloc(Obj::Arr("B".into(), b.iter().map(|&x| x as i8 as i64 as u64).collect()))
    }

    /// Defines a DEX from bytes (in-memory and file class loaders).
    fn define(&mut self, b: Vec<u8>) -> R<()> {
        let dex = Dex::parse(&b)?;
        self.load(Rc::new(dex));
        Ok(())
    }

    /// Class loading, reflection and asset I/O. `Ok(None)` when the call is not one of them.
    fn reflect(&mut self, class: &str, name: &str, sig: &str, a: &[Val]) -> R<Option<(Val, Val)>> {
        let none = (Val::Null, Val::Null);
        let dotted = |s: &str| format!("L{};", s.replace('.', "/"));
        let out = match (class, name, sig) {
            ("Landroid/content/Context;" | "Landroid/content/ContextWrapper;" | "Landroid/app/Activity;", "getAssets", _) => (self.alloc(Obj::Assets), Val::Null),
            ("Landroid/content/res/AssetManager;", "open", _) => {
                let n = self.str_arg(a[1]);
                let files = self.files.ok_or("no APK file access")?;
                let b = files(&format!("assets/{n}")).ok_or_else(|| format!("asset {n} not found"))?;
                (self.alloc(Obj::Stream(b, 0)), Val::Null)
            }
            ("Ljava/io/InputStream;", "read", "([BII)I") => {
                let (off, len) = (a[2].n() as usize, a[3].n() as usize);
                let chunk = match self.obj(a[0])? {
                    Obj::Stream(b, pos) => {
                        let c = b[*pos..].iter().take(len).copied().collect::<Vec<_>>();
                        *pos += c.len();
                        c
                    }
                    _ => return Err("read on non-stream".into()),
                };
                if chunk.is_empty() && len > 0 {
                    return Ok(Some((Val::N(-1i32 as u32), Val::Null)));
                }
                match self.obj(a[1])? {
                    Obj::Arr(_, x) if off + chunk.len() <= x.len() => {
                        for (k, &c) in chunk.iter().enumerate() {
                            x[off + k] = c as i8 as i64 as u64;
                        }
                    }
                    _ => return Err("read into bad buffer".into()),
                }
                (Val::N(chunk.len() as u32), Val::Null)
            }
            ("Ljava/io/InputStream;", "readAllBytes", _) => {
                let rest = match self.obj(a[0])? {
                    Obj::Stream(b, pos) => {
                        let c = b[*pos..].to_vec();
                        *pos = b.len();
                        c
                    }
                    _ => return Err("read on non-stream".into()),
                };
                (self.bytes_arr(&rest), Val::Null)
            }
            ("Ljava/io/InputStream;", "close", _) => none,
            ("Ljava/util/Arrays;", "copyOf", "([BI)[B") => {
                let mut b = self.bytes_of(a[0])?;
                b.resize(a[1].n() as usize, 0);
                (self.bytes_arr(&b), Val::Null)
            }
            ("Ljava/util/Arrays;", "copyOfRange", "([BII)[B") => {
                let b = self.bytes_of(a[0])?;
                let s = b.get(a[1].n() as usize..a[2].n() as usize).ok_or("copyOfRange out of range")?.to_vec();
                (self.bytes_arr(&s), Val::Null)
            }
            ("Ljava/nio/ByteBuffer;", "wrap", "([B)Ljava/nio/ByteBuffer;") => {
                let b = self.bytes_of(a[0])?;
                (self.alloc(Obj::Buffer(b)), Val::Null)
            }
            ("Ljava/nio/ByteBuffer;", "wrap", "([BII)Ljava/nio/ByteBuffer;") => {
                let b = self.bytes_of(a[0])?;
                let s = b.get(a[1].n() as usize..(a[1].n() + a[2].n()) as usize).ok_or("wrap out of range")?.to_vec();
                (self.alloc(Obj::Buffer(s)), Val::Null)
            }
            ("Ldalvik/system/InMemoryDexClassLoader;", "<init>", _) => {
                let b = self.bytes_of(a[1])?;
                self.define(b)?;
                *self.obj(a[0])? = Obj::Loader;
                none
            }
            ("Ldalvik/system/DexClassLoader;" | "Ldalvik/system/PathClassLoader;" | "Ldalvik/system/BaseDexClassLoader;", "<init>", _) => {
                let path = self.str_arg(a[1]);
                let files = self.files.ok_or("no APK file access")?;
                let base = path.rsplit('/').next().unwrap_or(&path).to_string();
                let b = files(&path).or_else(|| files(&format!("assets/{base}"))).ok_or_else(|| format!("class loader path {path} not in the APK"))?;
                self.define(b)?;
                *self.obj(a[0])? = Obj::Loader;
                none
            }
            ("Ljava/lang/Class;", "getClassLoader", _) | ("Ljava/lang/ClassLoader;", "getSystemClassLoader" | "getParent", _) => (self.alloc(Obj::Loader), Val::Null),
            ("Ljava/lang/ClassLoader;" | "Ldalvik/system/InMemoryDexClassLoader;" | "Ldalvik/system/DexClassLoader;" | "Ldalvik/system/PathClassLoader;", "loadClass", _) | ("Ljava/lang/Class;", "forName", _) => {
                let n = dotted(&self.str_arg(if class == "Ljava/lang/Class;" { a[0] } else { a[1] }));
                if !self.classes.contains_key(&n) {
                    return Err(format!("ClassNotFoundException: {n}"));
                }
                if class == "Ljava/lang/Class;" {
                    self.init_class(&n)?;
                }
                (self.alloc(Obj::Class(n)), Val::Null)
            }
            ("Ljava/lang/Class;", "getName" | "getSimpleName", _) => {
                let c = self.class_of(a[0])?;
                let full = c.strip_prefix('L').and_then(|c| c.strip_suffix(';')).unwrap_or(&c).replace('/', ".");
                let n = if name == "getSimpleName" { full.rsplit('.').next().unwrap_or(&full).to_string() } else { full };
                (self.alloc(Obj::Str(n)), Val::Null)
            }
            ("Ljava/lang/Class;", "getDeclaredMethod" | "getMethod", _) => {
                let c = self.class_of(a[0])?;
                let n = self.str_arg(a[1]);
                let params = self.refs_of(a[2])?.into_iter().map(|p| self.class_of(p)).collect::<R<Vec<_>>>()?;
                if self.find_method(&c, &n, &params).is_none() {
                    return Err(format!("NoSuchMethodException: {c}->{n}({})", params.concat()));
                }
                (self.alloc(Obj::Method(c, n, params)), Val::Null)
            }
            ("Ljava/lang/Class;", "getDeclaredField" | "getField", _) => {
                let c = self.class_of(a[0])?;
                let n = self.str_arg(a[1]);
                (self.alloc(Obj::Field(c, n)), Val::Null)
            }
            ("Ljava/lang/reflect/Method;" | "Ljava/lang/reflect/Field;" | "Ljava/lang/reflect/AccessibleObject;", "setAccessible", _) => none,
            ("Ljava/lang/reflect/Method;", "invoke", _) => {
                let Obj::Method(c, n, params) = self.obj(a[0])?.clone() else { return Err("invoke on non-method".into()) };
                let (d, m, _) = self.find_method(&c, &n, &params).ok_or("method vanished")?;
                let dex = self.dexes[d].clone();
                let mut slots = Vec::new();
                let static_m = dex.classes[self.classes[&c].1].methods.iter().find(|x| x.idx == m).is_some_and(|x| x.access & ACC_STATIC != 0);
                if !static_m {
                    slots.push(a[1]);
                }
                let args = self.refs_of(a[2])?;
                if args.len() != params.len() {
                    return Err("IllegalArgumentException: wrong argument count".into());
                }
                for (t, v) in params.iter().zip(args) {
                    if is_ref(t) {
                        slots.push(v);
                        continue;
                    }
                    let x = match self.obj(v)? {
                        Obj::Boxed('I' | 'S' | 'B' | 'C' | 'Z', x) => *x as u32 as i32 as i64 as u64,
                        Obj::Boxed(_, x) => *x,
                        _ => return Err("IllegalArgumentException: unboxing".into()),
                    };
                    if is_wide(t) {
                        let (lo, hi) = split(x);
                        slots.extend([lo, hi]);
                    } else {
                        slots.push(Val::N(x as u32));
                    }
                }
                let ret = dex.proto_of(m).map(|p| p.ret.clone()).unwrap_or_default();
                let r = self.call(d, m, &slots)?;
                let boxed = match ret.as_str() {
                    "V" => Val::Null,
                    "J" | "D" => self.alloc(Obj::Boxed(ret.chars().next().unwrap(), wide(r.0, r.1))),
                    t if !is_ref(t) => self.alloc(Obj::Boxed(t.chars().next().unwrap(), r.0.n() as u64)),
                    _ => r.0,
                };
                (boxed, Val::Null)
            }
            ("Ljava/lang/reflect/Field;", g @ ("get" | "getLong" | "getInt"), _) => {
                let Obj::Field(c, n) = self.obj(a[0])?.clone() else { return Err("get on non-field".into()) };
                self.init_class(&c)?;
                let (&(d, _), key) = (self.classes.get(&c).ok_or("field class unknown")?, (c.clone(), n.clone()));
                let ty = self.dexes[d].fields.iter().find(|f| f.class == c && f.name == n).map(|f| f.ty.clone()).ok_or_else(|| format!("NoSuchFieldException: {n}"))?;
                let val = match a[1] {
                    Val::Null => self.statics.get(&key).copied().unwrap_or_default(),
                    o => match self.obj(o)? {
                        Obj::Inst(_, m) => m.get(&n).copied().unwrap_or_default(),
                        _ => return Err("field get on non-instance".into()),
                    },
                };
                match (g, ty.as_str()) {
                    ("getLong", "J") | ("getLong", _) if !is_ref(&ty) => {
                        let x = if is_wide(&ty) { wide(val.0, val.1) } else { val.0.n() as i32 as i64 as u64 };
                        split(x)
                    }
                    ("getInt", _) => (Val::N(val.0.n()), Val::Null),
                    ("get", t) if is_ref(t) => (val.0, Val::Null),
                    ("get", t) => {
                        let x = if is_wide(t) { wide(val.0, val.1) } else { val.0.n() as u64 };
                        (self.alloc(Obj::Boxed(t.chars().next().unwrap(), x)), Val::Null)
                    }
                    _ => return Err(format!("unsupported Field.{g} on {ty}")),
                }
            }
            ("Ljava/lang/String;", "indexOf", "(I)I") => {
                let s: Vec<u16> = self.str_arg(a[0]).encode_utf16().collect();
                (Val::N(s.iter().position(|&c| c as u32 == a[1].n()).map_or(-1, |p| p as i32) as u32), Val::Null)
            }
            ("Ljava/lang/String;", "indexOf" | "lastIndexOf", "(Ljava/lang/String;)I") => {
                let (h, n) = (self.str_arg(a[0]), self.str_arg(a[1]));
                let p = if name == "indexOf" { h.find(&n) } else { h.rfind(&n) };
                (Val::N(p.map_or(-1, |p| h[..p].encode_utf16().count() as i32) as u32), Val::Null)
            }
            ("Ljava/lang/String;", "lastIndexOf", "(I)I") => {
                let s: Vec<u16> = self.str_arg(a[0]).encode_utf16().collect();
                (Val::N(s.iter().rposition(|&c| c as u32 == a[1].n()).map_or(-1, |p| p as i32) as u32), Val::Null)
            }
            ("Ljava/lang/String;", "split", "(Ljava/lang/String;)[Ljava/lang/String;") => {
                let (h, sep) = (self.str_arg(a[0]), self.str_arg(a[1]));
                if sep.chars().any(|c| ".$|()[{^?*+\\".contains(c)) {
                    return Err(format!("String.split with regex {sep:?}"));
                }
                let mut parts: Vec<String> = h.split(sep.as_str()).map(str::to_string).collect();
                while parts.len() > 1 && parts.last().is_some_and(|p| p.is_empty()) {
                    parts.pop();
                }
                let refs: Vec<u64> = parts.into_iter().map(|p| match self.alloc(Obj::Str(p)) {
                    Val::Ref(r) => r as u64 | 1 << 63,
                    _ => 0,
                }).collect();
                (self.alloc(Obj::Arr("Ljava/lang/String;".into(), refs)), Val::Null)
            }
            ("Ljava/lang/String;", "startsWith" | "endsWith" | "contains", _) => {
                let (h, n) = (self.str_arg(a[0]), self.str_arg(a[1]));
                let r = match name {
                    "startsWith" => h.starts_with(&n),
                    "endsWith" => h.ends_with(&n),
                    _ => h.contains(&n),
                };
                (Val::N(r as u32), Val::Null)
            }
            _ => return Ok(None),
        };
        Ok(Some(out))
    }

    fn chars_of(&mut self, v: Val) -> R<String> {
        match self.obj(v)? {
            Obj::Arr(_, x) => Ok(String::from_utf16_lossy(&x.iter().map(|&c| c as u16).collect::<Vec<_>>())),
            _ => Err("expected char[]".into()),
        }
    }

    fn exec(&mut self, d: usize, nregs: usize, ins: usize, code: &[u16], args: &[Val]) -> R<(Val, Val)> {
        let dex_rc = self.dexes[d].clone();
        let dex: &Dex = &dex_rc;
        let mut v = vec![Val::Null; nregs.max(ins)];
        let base = nregs.checked_sub(ins).ok_or("ins > registers")?;
        if args.len() != ins {
            return Err(format!("expected {ins} argument slots, got {}", args.len()));
        }
        v[base..base + ins].copy_from_slice(args);
        let mut res = (Val::Null, Val::Null);
        let mut pc = 0usize;
        macro_rules! u {
            ($i:expr) => {
                *code.get(pc + $i).ok_or("pc out of range")? as usize
            };
        }
        let g = |v: &Vec<Val>, i: usize| -> R<Val> { v.get(i).copied().ok_or_else(|| format!("register v{i} out of range")) };
        loop {
            self.steps += 1;
            if self.steps > self.limit {
                return Err("step limit exceeded".into());
            }
            let w0 = u!(0);
            let op = w0 & 0xff;
            let aa = w0 >> 8;
            let (a4, b4) = (aa & 0xf, aa >> 4);
            let getw = |v: &Vec<Val>, i: usize| -> R<u64> { Ok(wide(g(v, i)?, g(v, i + 1)?)) };
            let rel = |off: i64| -> R<usize> { usize::try_from(pc as i64 + off).map_err(|_| "branch out of range".to_string()) };
            let mut next = pc;
            match op {
                0x00 => next += 1,
                0x01 | 0x07 => {
                    set!(v, a4, g(&v, b4)?)?;
                    next += 1
                }
                0x02 | 0x08 => {
                    set!(v, aa, g(&v, u!(1))?)?;
                    next += 2
                }
                0x03 | 0x09 => {
                    set!(v, u!(1), g(&v, u!(2))?)?;
                    next += 3
                }
                0x04 => {
                    let x = getw(&v, b4)?;
                    setw!(v, a4, x)?;
                    next += 1
                }
                0x05 => {
                    let x = getw(&v, u!(1))?;
                    setw!(v, aa, x)?;
                    next += 2
                }
                0x06 => {
                    let x = getw(&v, u!(2))?;
                    setw!(v, u!(1), x)?;
                    next += 3
                }
                0x0a | 0x0c => {
                    set!(v, aa, res.0)?;
                    next += 1
                }
                0x0b => {
                    set!(v, aa, res.0)?;
                    set!(v, aa + 1, res.1)?;
                    next += 1
                }
                0x0e => return Ok((Val::Null, Val::Null)),
                0x0f | 0x11 => return Ok((g(&v, aa)?, Val::Null)),
                0x10 => return Ok((g(&v, aa)?, g(&v, aa + 1)?)),
                0x12 => {
                    set!(v, a4, Val::N(((b4 as i32) << 28 >> 28) as u32))?;
                    next += 1
                }
                0x13 => {
                    set!(v, aa, Val::N(u!(1) as i16 as i32 as u32))?;
                    next += 2
                }
                0x14 => {
                    set!(v, aa, Val::N((u!(1) | u!(2) << 16) as u32))?;
                    next += 3
                }
                0x15 => {
                    set!(v, aa, Val::N((u!(1) as u32) << 16))?;
                    next += 2
                }
                0x16 => {
                    setw!(v, aa, u!(1) as i16 as i64 as u64)?;
                    next += 2
                }
                0x17 => {
                    setw!(v, aa, (u!(1) | u!(2) << 16) as u32 as i32 as i64 as u64)?;
                    next += 3
                }
                0x18 => {
                    let x = (0..4).try_fold(0u64, |acc, k| Ok::<_, String>(acc | (u!(1 + k) as u64) << (16 * k)))?;
                    setw!(v, aa, x)?;
                    next += 5
                }
                0x19 => {
                    setw!(v, aa, (u!(1) as u64) << 48)?;
                    next += 2
                }
                0x1a | 0x1b => {
                    let (i, len) = if op == 0x1a { (u!(1), 2) } else { (u!(1) | u!(2) << 16, 3) };
                    let s = dex.strings.get(i).cloned().ok_or("bad string index")?;
                    let r = self.alloc(Obj::Str(s));
                    set!(v, aa, r)?;
                    next += len
                }
                0x1c => {
                    let t = dex.types.get(u!(1)).cloned().ok_or("bad type")?;
                    let r = self.alloc(Obj::Class(t));
                    set!(v, aa, r)?;
                    next += 2
                }
                0x1d | 0x1e => next += 1,
                0x1f => next += 2,
                0x21 => {
                    let n = match self.obj(g(&v, b4)?)? {
                        Obj::Arr(_, x) => x.len(),
                        _ => return Err("array-length on non-array".into()),
                    };
                    set!(v, a4, Val::N(n as u32))?;
                    next += 1
                }
                0x22 => {
                    let t = dex.types.get(u!(1)).cloned().ok_or("bad type")?;
                    self.init_class(&t)?;
                    let r = self.alloc(Obj::Inst(t, HashMap::new()));
                    set!(v, aa, r)?;
                    next += 2
                }
                0x23 => {
                    let t = dex.types.get(u!(1)).cloned().ok_or("bad type")?;
                    let n = g(&v, b4)?.n() as i32;
                    if !(0..=1 << 24).contains(&n) {
                        return Err(format!("new-array size {n}"));
                    }
                    let r = self.alloc(Obj::Arr(t[1..].to_string(), vec![0; n as usize]));
                    set!(v, a4, r)?;
                    next += 2
                }
                0x24 | 0x25 => {
                    let t = dex.types.get(u!(1)).cloned().ok_or("bad type")?;
                    let regs: Vec<usize> = if op == 0x24 {
                        let w2 = u!(2);
                        [w2 & 0xf, w2 >> 4 & 0xf, w2 >> 8 & 0xf, w2 >> 12, a4].into_iter().take(b4).collect()
                    } else {
                        (u!(2)..u!(2) + aa).collect()
                    };
                    let vals = regs.iter().map(|&r| g(&v, r).map(|x| x.n() as u64)).collect::<R<Vec<_>>>()?;
                    res = (self.alloc(Obj::Arr(t[1..].to_string(), vals)), Val::Null);
                    next += 3
                }
                0x26 => {
                    let off = (u!(1) | u!(2) << 16) as u32 as i32 as i64;
                    let p = rel(off)?;
                    let (width, n) = (*code.get(p + 1).ok_or("bad payload")? as usize, (*code.get(p + 2).ok_or("bad payload")? as usize) | (*code.get(p + 3).ok_or("bad payload")? as usize) << 16);
                    let bytes: Vec<u8> = code.get(p + 4..).ok_or("bad payload")?.iter().flat_map(|w| w.to_le_bytes()).collect();
                    let raw = bytes.get(..width * n).ok_or("bad payload")?;
                    let vals: Vec<u64> = raw
                        .chunks_exact(width.max(1))
                        .map(|c| {
                            let x = c.iter().rev().fold(0u64, |acc, &b| acc << 8 | b as u64);
                            let sh = 64 - 8 * width as u32;
                            ((x << sh) as i64 >> sh) as u64
                        })
                        .collect();
                    match self.obj(g(&v, aa)?)? {
                        Obj::Arr(_, arr) if arr.len() >= vals.len() => arr[..vals.len()].copy_from_slice(&vals),
                        _ => return Err("fill-array-data mismatch".into()),
                    }
                    next += 3
                }
                0x27 => return Err("throw".into()),
                0x28 => next = rel((aa as u8 as i8) as i64)?,
                0x29 => next = rel(u!(1) as i16 as i64)?,
                0x2a => next = rel((u!(1) | u!(2) << 16) as u32 as i32 as i64)?,
                0x2b | 0x2c => {
                    let p = rel((u!(1) | u!(2) << 16) as u32 as i32 as i64)?;
                    let at = |k: usize| -> R<i32> { Ok((*code.get(p + k).ok_or("bad switch")? as u32 | (*code.get(p + k + 1).ok_or("bad switch")? as u32) << 16) as i32) };
                    let n = *code.get(p + 1).ok_or("bad switch")? as usize;
                    let key = g(&v, aa)?.n() as i32;
                    let mut target = None;
                    if op == 0x2b {
                        let first = at(2)?;
                        let k = key.wrapping_sub(first);
                        if (0..n as i32).contains(&k) {
                            target = Some(at(4 + 2 * k as usize)?);
                        }
                    } else {
                        for k in 0..n {
                            if at(2 + 2 * k)? == key {
                                target = Some(at(2 + 2 * n + 2 * k)?);
                            }
                        }
                    }
                    next = match target {
                        Some(t) => rel(t as i64)?,
                        None => pc + 3,
                    }
                }
                0x2d..=0x31 => {
                    let (b, c) = (u!(1) & 0xff, u!(1) >> 8);
                    let ord = match op {
                        0x31 => Some((getw(&v, b)? as i64).cmp(&(getw(&v, c)? as i64))),
                        0x2d | 0x2e => fbits(g(&v, b)?.n()).partial_cmp(&fbits(g(&v, c)?.n())),
                        _ => dbits(getw(&v, b)?).partial_cmp(&dbits(getw(&v, c)?)),
                    };
                    let r = match ord {
                        Some(o) => o as i32,
                        None => {
                            if op == 0x2d || op == 0x2f {
                                -1
                            } else {
                                1
                            }
                        }
                    };
                    set!(v, aa, Val::N(r as u32))?;
                    next += 2
                }
                0x32..=0x3d => {
                    let (x, y, off) = if op <= 0x37 { (g(&v, a4)?, g(&v, b4)?, u!(1)) } else { (g(&v, aa)?, Val::N(0), u!(1)) };
                    let (xi, yi) = (x.n() as i32, y.n() as i32);
                    // References compare as identities; null and int 0 are both "zero".
                    let same = match (x, y) {
                        (Val::Ref(p), Val::Ref(q)) => p == q,
                        (Val::Ref(_), _) | (_, Val::Ref(_)) => false,
                        _ => xi == yi,
                    };
                    let take = match (op - 0x32) % 6 {
                        0 => same,
                        1 => !same,
                        2 => xi < yi,
                        3 => xi >= yi,
                        4 => xi > yi,
                        _ => xi <= yi,
                    };
                    next = if take { rel(off as i16 as i64)? } else { pc + 2 };
                }
                0x44..=0x51 => {
                    let (b, c) = (u!(1) & 0xff, u!(1) >> 8);
                    let i = g(&v, c)?.n() as i32;
                    let put = op >= 0x4b;
                    let k = if put { op - 0x4b } else { op - 0x44 };
                    let src = if put { if k == 1 { Some(getw(&v, aa)?) } else { Some(g(&v, aa)?.n() as u64) } } else { None };
                    let srcref = g(&v, aa)?;
                    let arr = g(&v, b)?;
                    let o = self.obj(arr)?;
                    let Obj::Arr(_, x) = o else { return Err("aget/aput on non-array".into()) };
                    let slot = usize::try_from(i).ok().and_then(|i| x.get_mut(i)).ok_or_else(|| format!("array index {i} out of range"))?;
                    if put {
                        *slot = match (k, srcref) {
                            (2, Val::Ref(r)) => r as u64 | 1 << 63,
                            (2, _) => 0,
                            _ => src.unwrap(),
                        };
                    } else {
                        let e = *slot;
                        match k {
                            1 => setw!(v, aa, e)?,
                            2 => set!(v, aa, if e >> 63 == 1 { Val::Ref((e & !(1 << 63)) as usize) } else { Val::Null })?,
                            3 => set!(v, aa, Val::N((e != 0) as u32))?,
                            4 => set!(v, aa, Val::N(e as u8 as i8 as i32 as u32))?,
                            5 => set!(v, aa, Val::N(e as u16 as u32))?,
                            6 => set!(v, aa, Val::N(e as u16 as i16 as i32 as u32))?,
                            _ => set!(v, aa, Val::N(e as u32))?,
                        }
                    }
                    next += 2
                }
                0x52..=0x5f => {
                    let f = u!(1) as u32;
                    let wide_f = op == 0x53 || op == 0x5a;
                    let objv = g(&v, b4)?;
                    if op <= 0x58 {
                        let fname = dex.fields[f as usize].name.clone();
                        let val = match self.obj(objv)? {
                            Obj::Inst(_, m) => m.get(&fname).copied().unwrap_or_default(),
                            _ => return Err("iget on non-instance".into()),
                        };
                        let val = if val == (Val::Null, Val::Null) && !is_ref(&dex.fields[f as usize].ty) { (Val::N(0), Val::N(0)) } else { val };
                        set!(v, a4, val.0)?;
                        if wide_f {
                            set!(v, a4 + 1, val.1)?;
                        }
                    } else {
                        let val = (g(&v, a4)?, if wide_f { g(&v, a4 + 1)? } else { Val::Null });
                        let fname = dex.fields[f as usize].name.clone();
                        match self.obj(objv)? {
                            Obj::Inst(_, m) => m.insert(fname, val),
                            _ => return Err("iput on non-instance".into()),
                        };
                    }
                    next += 2
                }
                0x60..=0x6d => {
                    let f = u!(1) as u32;
                    let fr = dex.fields.get(f as usize).ok_or("bad field")?;
                    let wide_f = op == 0x61 || op == 0x68;
                    if !self.classes.contains_key(&fr.class) {
                        // Primitive class constants (`Long.TYPE`) for reflection lookups.
                        let prim = match (fr.class.as_str(), fr.name.as_str()) {
                            ("Ljava/lang/Long;", "TYPE") => "J",
                            ("Ljava/lang/Integer;", "TYPE") => "I",
                            ("Ljava/lang/Short;", "TYPE") => "S",
                            ("Ljava/lang/Byte;", "TYPE") => "B",
                            ("Ljava/lang/Character;", "TYPE") => "C",
                            ("Ljava/lang/Boolean;", "TYPE") => "Z",
                            ("Ljava/lang/Void;", "TYPE") => "V",
                            _ => return Err(format!("static field of external class {}.{}", fr.class, fr.name)),
                        };
                        if op != 0x62 {
                            return Err(format!("write to {}.{}", fr.class, fr.name));
                        }
                        let r = self.alloc(Obj::Class(prim.into()));
                        set!(v, aa, r)?;
                        next += 2;
                        pc = next;
                        continue;
                    }
                    self.init_class(&fr.class)?;
                    let key = (fr.class.clone(), fr.name.clone());
                    if op <= 0x66 {
                        let val = self.statics.get(&key).copied().unwrap_or((if is_ref(&fr.ty) { Val::Null } else { Val::N(0) }, Val::N(0)));
                        set!(v, aa, val.0)?;
                        if wide_f {
                            set!(v, aa + 1, val.1)?;
                        }
                    } else {
                        let val = (g(&v, aa)?, if wide_f { g(&v, aa + 1)? } else { Val::Null });
                        self.statics.insert(key, val);
                    }
                    next += 2
                }
                0x6e..=0x72 | 0x74..=0x78 => {
                    let m = u!(1) as u32;
                    let regs: Vec<usize> = if op <= 0x72 {
                        let w2 = u!(2);
                        [w2 & 0xf, w2 >> 4 & 0xf, w2 >> 8 & 0xf, w2 >> 12, a4].into_iter().take(b4).collect()
                    } else {
                        (u!(2)..u!(2) + aa).collect()
                    };
                    let args = regs.iter().map(|&r| g(&v, r)).collect::<R<Vec<_>>>()?;
                    res = self.call(d, m, &args)?;
                    next += 3
                }
                0x7b..=0x8f => {
                    let (a, b) = (a4, b4);
                    match op {
                        0x7b => set!(v, a, Val::N((g(&v, b)?.n() as i32).wrapping_neg() as u32))?,
                        0x7c => set!(v, a, Val::N(!g(&v, b)?.n()))?,
                        0x7d => setw!(v, a, (getw(&v, b)? as i64).wrapping_neg() as u64)?,
                        0x7e => setw!(v, a, !getw(&v, b)?)?,
                        0x7f => set!(v, a, Val::N(g(&v, b)?.n() ^ 0x8000_0000))?,
                        0x80 => setw!(v, a, getw(&v, b)? ^ 1 << 63)?,
                        0x81 => setw!(v, a, g(&v, b)?.n() as i32 as i64 as u64)?,
                        0x82 => set!(v, a, Val::N((g(&v, b)?.n() as i32 as f32).to_bits()))?,
                        0x83 => setw!(v, a, (g(&v, b)?.n() as i32 as f64).to_bits())?,
                        0x84 => set!(v, a, Val::N(getw(&v, b)? as u32))?,
                        0x85 => set!(v, a, Val::N((getw(&v, b)? as i64 as f32).to_bits()))?,
                        0x86 => setw!(v, a, (getw(&v, b)? as i64 as f64).to_bits())?,
                        0x87 => set!(v, a, Val::N(fbits(g(&v, b)?.n()) as i32 as u32))?,
                        0x88 => setw!(v, a, fbits(g(&v, b)?.n()) as i64 as u64)?,
                        0x89 => setw!(v, a, (fbits(g(&v, b)?.n()) as f64).to_bits())?,
                        0x8a => set!(v, a, Val::N(dbits(getw(&v, b)?) as i32 as u32))?,
                        0x8b => setw!(v, a, dbits(getw(&v, b)?) as i64 as u64)?,
                        0x8c => set!(v, a, Val::N((dbits(getw(&v, b)?) as f32).to_bits()))?,
                        0x8d => set!(v, a, Val::N(g(&v, b)?.n() as u8 as i8 as i32 as u32))?,
                        0x8e => set!(v, a, Val::N(g(&v, b)?.n() & 0xffff))?,
                        _ => set!(v, a, Val::N(g(&v, b)?.n() as u16 as i16 as i32 as u32))?,
                    }
                    next += 1
                }
                0x90..=0xe2 => {
                    // (kind, dest, lhs reg, rhs operand)
                    enum Rhs {
                        Reg(usize),
                        Lit(i32),
                    }
                    let (k, d, l, rhs, len) = match op {
                        0x90..=0xaf => (op - 0x90, aa, u!(1) & 0xff, Rhs::Reg(u!(1) >> 8), 2),
                        0xb0..=0xcf => (op - 0xb0, a4, a4, Rhs::Reg(b4), 1),
                        0xd0..=0xd7 => ([0, 0xff, 2, 3, 4, 5, 6, 7][op - 0xd0], a4, b4, Rhs::Lit(u!(1) as i16 as i32), 2),
                        _ => ([0, 0xff, 2, 3, 4, 5, 6, 7, 8, 9, 10][op - 0xd8], aa, u!(1) & 0xff, Rhs::Lit((u!(1) >> 8) as u8 as i8 as i32), 2),
                    };
                    if k == 0xff {
                        // rsub-int: lit - v
                        let Rhs::Lit(c) = rhs else { unreachable!() };
                        set!(v, d, Val::N(c.wrapping_sub(g(&v, l)?.n() as i32) as u32))?;
                    } else if k <= 10 {
                        let x = g(&v, l)?.n() as i32;
                        let y = match rhs {
                            Rhs::Reg(r) => g(&v, r)?.n() as i32,
                            Rhs::Lit(c) => c,
                        };
                        let r = match k {
                            0 => x.wrapping_add(y),
                            1 => x.wrapping_sub(y),
                            2 => x.wrapping_mul(y),
                            3 | 4 if y == 0 => return Err("ArithmeticException: / by zero".into()),
                            3 => x.wrapping_div(y),
                            4 => x.wrapping_rem(y),
                            5 => x & y,
                            6 => x | y,
                            7 => x ^ y,
                            8 => x.wrapping_shl(y as u32 & 31),
                            9 => x.wrapping_shr(y as u32 & 31),
                            _ => ((x as u32) >> (y as u32 & 31)) as i32,
                        };
                        set!(v, d, Val::N(r as u32))?;
                    } else if k <= 21 {
                        let Rhs::Reg(rr) = rhs else { unreachable!() };
                        let x = getw(&v, l)? as i64;
                        let r = if k >= 19 {
                            let s = g(&v, rr)?.n() & 63;
                            match k {
                                19 => x.wrapping_shl(s),
                                20 => x.wrapping_shr(s),
                                _ => ((x as u64) >> s) as i64,
                            }
                        } else {
                            let y = getw(&v, rr)? as i64;
                            match k {
                                11 => x.wrapping_add(y),
                                12 => x.wrapping_sub(y),
                                13 => x.wrapping_mul(y),
                                14 | 15 if y == 0 => return Err("ArithmeticException: / by zero".into()),
                                14 => x.wrapping_div(y),
                                15 => x.wrapping_rem(y),
                                16 => x & y,
                                17 => x | y,
                                _ => x ^ y,
                            }
                        };
                        setw!(v, d, r as u64)?;
                    } else {
                        let Rhs::Reg(rr) = rhs else { unreachable!() };
                        if k <= 26 {
                            let (x, y) = (fbits(g(&v, l)?.n()), fbits(g(&v, rr)?.n()));
                            let r = [x + y, x - y, x * y, x / y, x % y][k - 22];
                            set!(v, d, Val::N(r.to_bits()))?;
                        } else {
                            let (x, y) = (dbits(getw(&v, l)?), dbits(getw(&v, rr)?));
                            let r = [x + y, x - y, x * y, x / y, x % y][k - 27];
                            setw!(v, d, r.to_bits())?;
                        }
                    }
                    next += len
                }
                _ => return Err(format!("unsupported opcode {op:#04x} at {pc:#06x}")),
            }
            pc = next;
        }
    }

    /// Calls a static method with every argument null/zero (by its proto).
    pub fn call_with_defaults(&mut self, idx: u32) -> R<(Val, Val)> {
        let dex = self.dexes[0].clone();
        let p = dex.proto_of(idx).ok_or("bad method")?;
        let slots: usize = p.params.iter().map(|t| if is_wide(t) { 2 } else { 1 }).sum();
        let args: Vec<Val> = p.params.iter().flat_map(|t| if is_ref(t) { vec![Val::Null] } else if is_wide(t) { vec![Val::N(0); 2] } else { vec![Val::N(0)] }).collect();
        debug_assert_eq!(args.len(), slots);
        self.call(0, idx, &args)
    }
}

/// Every static method of the DEX's own classes returning a `String` and taking only reference parameters,
/// run with null arguments: `(signature, result)`.
pub fn run_string_methods(dex: &Rc<Dex>, native: Option<&dyn Fn(&str, &[u64]) -> R<u64>>, files: Option<&dyn Fn(&str) -> Option<Vec<u8>>>) -> Vec<(String, Result<String, String>)> {
    let mut out = Vec::new();
    for c in &dex.classes {
        for m in &c.methods {
            let Some(p) = dex.proto_of(m.idx) else { continue };
            if m.access & ACC_STATIC == 0 || m.code.is_none() || p.ret != "Ljava/lang/String;" || !p.params.iter().all(|t| is_ref(t)) {
                continue;
            }
            let mut vm = Vm::new(dex.clone());
            vm.limit = 20_000_000;
            vm.native = native;
            vm.files = files;
            let r = vm.call_with_defaults(m.idx).and_then(|(r, _)| match r {
                Val::Null => Ok("null".to_string()),
                x => vm.string(x).map(str::to_string).ok_or_else(|| "non-string result".to_string()),
            });
            out.push((dex.method_sig(m.idx), r));
        }
    }
    out
}
