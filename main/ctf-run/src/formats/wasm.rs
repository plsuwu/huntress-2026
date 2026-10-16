//! WebAssembly binary modules (`\0asm`, version 1): section walk, enough to identify the module and
//! list its imports/exports, custom sections and memories (memory64 decides `wasm32` vs `wasm64`).

#[derive(Clone, Debug, Default)]
pub struct Wasm {
    pub version: u32,
    /// (id, payload offset, payload length) in file order.
    pub sections: Vec<(u8, usize, usize)>,
    pub custom_names: Vec<String>,
    /// (module, field, kind) with kind 0 func, 1 table, 2 memory, 3 global, 4 tag.
    pub imports: Vec<(String, String, u8)>,
    pub exports: Vec<(String, u8)>,
    /// One flag per memory (imported or defined): bit 0x04 marks memory64.
    pub memory_flags: Vec<u32>,
}

impl Wasm {
    pub fn is_memory64(&self) -> bool {
        self.memory_flags.iter().any(|f| f & 0x04 != 0)
    }

    /// Conventional architecture label: `wasm64` for memory64 modules, otherwise `wasm32`.
    pub fn arch(&self) -> &'static str {
        if self.is_memory64() { "wasm64" } else { "wasm32" }
    }
}

pub fn is_wasm(d: &[u8]) -> bool {
    d.starts_with(b"\0asm")
}

fn uleb(d: &[u8], p: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let b = *d.get(*p)?;
        *p += 1;
        v |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

fn name(d: &[u8], p: &mut usize) -> Option<String> {
    let n = uleb(d, p)? as usize;
    let s = d.get(*p..*p + n)?;
    *p += n;
    Some(String::from_utf8_lossy(s).into_owned())
}

/// Memory limits: flags then min (and max when flag bit 0 is set). Returns the flags.
fn limits(d: &[u8], p: &mut usize) -> Option<u32> {
    let f = uleb(d, p)? as u32;
    uleb(d, p)?;
    if f & 1 != 0 {
        uleb(d, p)?;
    }
    Some(f)
}

pub fn parse(d: &[u8]) -> Option<Wasm> {
    if !is_wasm(d) {
        return None;
    }
    let mut w = Wasm { version: u32::from_le_bytes(d.get(4..8)?.try_into().ok()?), ..Wasm::default() };
    let mut p = 8;
    while p < d.len() {
        let id = d[p];
        p += 1;
        let len = uleb(d, &mut p)? as usize;
        let (start, end) = (p, p.checked_add(len).filter(|&e| e <= d.len())?);
        w.sections.push((id, start, len));
        let body = &d[..end];
        let mut q = start;
        // Payload details are best-effort: a malformed body still leaves the section list intact.
        let _ = (|| -> Option<()> {
            match id {
                0 => w.custom_names.push(name(body, &mut q)?),
                2 => {
                    for _ in 0..uleb(body, &mut q)? {
                        let (m, f) = (name(body, &mut q)?, name(body, &mut q)?);
                        let kind = *body.get(q)?;
                        q += 1;
                        match kind {
                            0 => {
                                uleb(body, &mut q)?;
                            }
                            1 => {
                                q += 1;
                                limits(body, &mut q)?;
                            }
                            2 => w.memory_flags.push(limits(body, &mut q)?),
                            3 => q += 2,
                            4 => {
                                q += 1;
                                uleb(body, &mut q)?;
                            }
                            _ => return None,
                        }
                        w.imports.push((m, f, kind));
                    }
                }
                5 => {
                    for _ in 0..uleb(body, &mut q)? {
                        w.memory_flags.push(limits(body, &mut q)?);
                    }
                }
                7 => {
                    for _ in 0..uleb(body, &mut q)? {
                        let n = name(body, &mut q)?;
                        let kind = *body.get(q)?;
                        q += 1;
                        uleb(body, &mut q)?;
                        w.exports.push((n, kind));
                    }
                }
                _ => {}
            }
            Some(())
        })();
        p = end;
    }
    Some(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_and_exports() {
        // type () -> (), memory 1 page, export "memory" (mem 0) and "f" (func 0).
        let m = b"\0asm\x01\0\0\0\x05\x03\x01\x00\x01\x07\x0e\x02\x06memory\x02\x00\x01f\x00\x00\x00\x05\x04name";
        let w = parse(m).unwrap();
        assert_eq!(w.version, 1);
        assert_eq!(w.arch(), "wasm32");
        assert_eq!(w.exports, vec![("memory".into(), 2), ("f".into(), 0)]);
        assert_eq!(w.custom_names, vec!["name".to_string()]);
    }
}
