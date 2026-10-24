//! Dalvik executable (`classes.dex`) parser: string/type/proto/field/method tables, class definitions with
//! their methods' code items and static initial values. `formats::dalvik` interprets the bytecode.

use super::bytes::{u16le, u32le};

#[derive(Clone, Debug)]
pub struct Proto {
    pub shorty: String,
    pub ret: String,
    pub params: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct MethodRef {
    pub class: String,
    pub name: String,
    pub proto: u32,
}

#[derive(Clone, Debug)]
pub struct FieldRef {
    pub class: String,
    pub ty: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct Code {
    pub registers: u16,
    pub ins: u16,
    pub insns: Vec<u16>,
}

#[derive(Clone, Debug)]
pub struct Method {
    pub idx: u32,
    pub access: u32,
    pub code: Option<Code>,
}

/// An `encoded_value` (static initial values).
#[derive(Clone, Debug, PartialEq)]
pub enum Encoded {
    Prim(u64),
    Str(u32),
    Null,
    Other,
}

#[derive(Clone, Debug)]
pub struct Class {
    pub name: String,
    pub superclass: Option<String>,
    /// `(field index, initial value)`; fields past the `static_values` array are zero/null.
    pub static_fields: Vec<(u32, Encoded)>,
    pub methods: Vec<Method>,
}

pub struct Dex {
    pub strings: Vec<String>,
    pub types: Vec<String>,
    pub protos: Vec<Proto>,
    pub fields: Vec<FieldRef>,
    pub methods: Vec<MethodRef>,
    pub classes: Vec<Class>,
}

pub const ACC_STATIC: u32 = 0x8;

pub fn is_dex(b: &[u8]) -> bool {
    b.len() >= 0x70 && b.starts_with(b"dex\n") && b[7] == 0
}

fn uleb(b: &[u8], at: &mut usize) -> Option<u32> {
    let mut v: u32 = 0;
    for i in 0..5 {
        let c = *b.get(*at)?;
        *at += 1;
        v |= ((c & 0x7f) as u32) << (7 * i);
        if c & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

/// MUTF-8 (`C0 80` for NUL, surrogates as 3-byte sequences) decoded loosely.
fn mutf8(b: &[u8]) -> String {
    let mut units: Vec<u16> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as u32;
        let (u, n) = if c < 0x80 {
            (c, 1)
        } else if c & 0xe0 == 0xc0 && i + 1 < b.len() {
            (((c & 0x1f) << 6) | (b[i + 1] as u32 & 0x3f), 2)
        } else if c & 0xf0 == 0xe0 && i + 2 < b.len() {
            (((c & 0x0f) << 12) | ((b[i + 1] as u32 & 0x3f) << 6) | (b[i + 2] as u32 & 0x3f), 3)
        } else {
            (0xfffd, 1)
        };
        units.push(u as u16);
        i += n;
    }
    String::from_utf16_lossy(&units)
}

fn encoded_value(b: &[u8], at: &mut usize) -> Option<Encoded> {
    let h = *b.get(*at)?;
    *at += 1;
    let (ty, arg) = (h & 0x1f, (h >> 5) as usize);
    let raw = |at: &mut usize, n: usize| -> Option<u64> {
        let s = b.get(*at..*at + n)?;
        *at += n;
        Some(s.iter().rev().fold(0u64, |a, &x| (a << 8) | x as u64))
    };
    let sext = |v: u64, n: usize| -> u64 {
        let sh = 64 - 8 * n as u32;
        (((v << sh) as i64) >> sh) as u64
    };
    Some(match ty {
        // byte, short, int, long: sign-extended.
        0x00 | 0x02 | 0x04 | 0x06 => Encoded::Prim(sext(raw(at, arg + 1)?, arg + 1)),
        // char: zero-extended.
        0x03 => Encoded::Prim(raw(at, arg + 1)?),
        // float, double: right-zero-extended.
        0x10 => Encoded::Prim(raw(at, arg + 1)? << (8 * (3 - arg as u32))),
        0x11 => Encoded::Prim(raw(at, arg + 1)? << (8 * (7 - arg as u32))),
        0x17 => Encoded::Str(raw(at, arg + 1)? as u32),
        0x18..=0x1b => {
            raw(at, arg + 1)?;
            Encoded::Other
        }
        0x1c => {
            let n = uleb(b, at)?;
            for _ in 0..n {
                encoded_value(b, at)?;
            }
            Encoded::Other
        }
        0x1d => {
            uleb(b, at)?;
            let n = uleb(b, at)?;
            for _ in 0..n {
                uleb(b, at)?;
                encoded_value(b, at)?;
            }
            Encoded::Other
        }
        0x1e => Encoded::Null,
        0x1f => Encoded::Prim(arg as u64),
        _ => return None,
    })
}

impl Dex {
    pub fn parse(b: &[u8]) -> Result<Dex, String> {
        if !is_dex(b) {
            return Err("not a DEX file".into());
        }
        let bad = || "truncated DEX".to_string();
        let table = |at: usize| -> Option<(usize, usize)> { Some((u32le(b, at)? as usize, u32le(b, at + 4)? as usize)) };
        let (ns, os) = table(0x38).ok_or_else(bad)?;
        let mut strings = Vec::with_capacity(ns);
        for i in 0..ns {
            let mut at = u32le(b, os + 4 * i).ok_or_else(bad)? as usize;
            uleb(b, &mut at).ok_or_else(bad)?;
            let end = at + b.get(at..).and_then(|s| s.iter().position(|&c| c == 0)).ok_or_else(bad)?;
            strings.push(mutf8(&b[at..end]));
        }
        let s = |i: u32| strings.get(i as usize).cloned().unwrap_or_default();
        let (nt, ot) = table(0x40).ok_or_else(bad)?;
        let types: Vec<String> = (0..nt).map(|i| u32le(b, ot + 4 * i).map(s)).collect::<Option<_>>().ok_or_else(bad)?;
        let t = |i: u32| types.get(i as usize).cloned().unwrap_or_default();
        let (np, op) = table(0x48).ok_or_else(bad)?;
        let mut protos = Vec::with_capacity(np);
        for i in 0..np {
            let at = op + 12 * i;
            let (sh, ret, poff) = (u32le(b, at).ok_or_else(bad)?, u32le(b, at + 4).ok_or_else(bad)?, u32le(b, at + 8).ok_or_else(bad)? as usize);
            let mut params = Vec::new();
            if poff != 0 {
                let n = u32le(b, poff).ok_or_else(bad)? as usize;
                for k in 0..n {
                    params.push(t(u16le(b, poff + 4 + 2 * k).ok_or_else(bad)? as u32));
                }
            }
            protos.push(Proto { shorty: s(sh), ret: t(ret), params });
        }
        let (nf, of) = table(0x50).ok_or_else(bad)?;
        let mut fields = Vec::with_capacity(nf);
        for i in 0..nf {
            let at = of + 8 * i;
            fields.push(FieldRef { class: t(u16le(b, at).ok_or_else(bad)? as u32), ty: t(u16le(b, at + 2).ok_or_else(bad)? as u32), name: s(u32le(b, at + 4).ok_or_else(bad)?) });
        }
        let (nm, om) = table(0x58).ok_or_else(bad)?;
        let mut methods = Vec::with_capacity(nm);
        for i in 0..nm {
            let at = om + 8 * i;
            methods.push(MethodRef { class: t(u16le(b, at).ok_or_else(bad)? as u32), proto: u16le(b, at + 2).ok_or_else(bad)? as u32, name: s(u32le(b, at + 4).ok_or_else(bad)?) });
        }
        let (nc, oc) = table(0x60).ok_or_else(bad)?;
        let mut classes = Vec::with_capacity(nc);
        for i in 0..nc {
            let at = oc + 32 * i;
            let word = |k: usize| u32le(b, at + 4 * k).ok_or_else(bad);
            let sup = word(2)?;
            let (data_off, sv_off) = (word(6)? as usize, word(7)? as usize);
            let mut static_ids = Vec::new();
            let mut cmethods = Vec::new();
            if data_off != 0 {
                let mut p = data_off;
                let mut r = || uleb(b, &mut p).ok_or_else(bad);
                let (nsf, nif, ndm, nvm) = (r()?, r()?, r()?, r()?);
                let mut idx = 0;
                for _ in 0..nsf {
                    idx += r()?;
                    r()?;
                    static_ids.push(idx);
                }
                for _ in 0..nif {
                    r()?;
                    r()?;
                }
                for n in [ndm, nvm] {
                    let mut idx = 0;
                    for _ in 0..n {
                        idx += r()?;
                        let access = r()?;
                        let code_off = r()? as usize;
                        let code = if code_off == 0 {
                            None
                        } else {
                            let regs = u16le(b, code_off).ok_or_else(bad)?;
                            let ins = u16le(b, code_off + 2).ok_or_else(bad)?;
                            let n = u32le(b, code_off + 12).ok_or_else(bad)? as usize;
                            let raw = b.get(code_off + 16..code_off + 16 + 2 * n).ok_or_else(bad)?;
                            Some(Code { registers: regs, ins, insns: raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect() })
                        };
                        cmethods.push(Method { idx, access, code });
                    }
                }
            }
            let mut values = Vec::new();
            if sv_off != 0 {
                let mut p = sv_off;
                let n = uleb(b, &mut p).ok_or_else(bad)?;
                for _ in 0..n {
                    values.push(encoded_value(b, &mut p).ok_or_else(bad)?);
                }
            }
            let static_fields = static_ids
                .into_iter()
                .enumerate()
                .map(|(k, f)| {
                    let zero = if matches!(fields.get(f as usize).map(|x| x.ty.as_bytes()[0]), Some(b'L' | b'[')) { Encoded::Null } else { Encoded::Prim(0) };
                    (f, values.get(k).cloned().unwrap_or(zero))
                })
                .collect();
            classes.push(Class { name: t(word(0)?), superclass: (sup != 0xffff_ffff).then(|| t(sup)), static_fields, methods: cmethods });
        }
        Ok(Dex { strings, types, protos, fields, methods, classes })
    }

    /// `Lpkg/Class;->name(params)ret`.
    pub fn method_sig(&self, idx: u32) -> String {
        let Some(m) = self.methods.get(idx as usize) else { return format!("method@{idx}") };
        let p = &self.protos[m.proto as usize];
        format!("{}->{}({}){}", m.class, m.name, p.params.concat(), p.ret)
    }

    pub fn proto_of(&self, method: u32) -> Option<&Proto> {
        self.protos.get(self.methods.get(method as usize)?.proto as usize)
    }
}
