//! A tiny x86-64 emulator for straight-line leaf functions (JNI `nativeShare(J)J` mixers in `libarena.so`):
//! integer mov/add/sub/and/or/xor/imul/not/neg/bswap/lea, shifts and rotates, push/pop and `[reg+disp]`
//! stack memory (for -O0 spills). Any other instruction - calls, branches, floating point - is an error.

use std::collections::HashMap;

pub struct Cpu {
    pub r: [u64; 16],
    mem: HashMap<u64, u8>,
    pub steps: u64,
    /// `(ZF, SF, CF, OF)` from the last flag-setting instruction; `None` after one whose flags are not modelled.
    flags: Option<(bool, bool, bool, bool)>,
}

#[derive(Clone, Copy)]
enum FlagOp {
    Add,
    Sub,
    Logic,
}

fn flags_of(op: FlagOp, x: u64, y: u64, res: u64, size: u32) -> (bool, bool, bool, bool) {
    let bits = 8 * size;
    let m = if size == 8 { u64::MAX } else { (1u64 << bits) - 1 };
    let (x, y, res) = (x & m, y & m, res & m);
    let sign = |v: u64| v >> (bits - 1) & 1 == 1;
    let (cf, of) = match op {
        FlagOp::Add => (res < x, sign(x) == sign(y) && sign(res) != sign(x)),
        FlagOp::Sub => (x < y, sign(x) != sign(y) && sign(res) != sign(x)),
        FlagOp::Logic => (false, false),
    };
    (res == 0, sign(res), cf, of)
}

const STACK_TOP: u64 = 0x7fff_0000_0000;
const RET_MAGIC: u64 = 0xdead_beef_0000;

enum Rm {
    Reg(usize),
    Mem(u64),
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Cpu {
        let mut c = Cpu { r: [0; 16], mem: HashMap::new(), steps: 0, flags: None };
        c.r[4] = STACK_TOP;
        c.push(RET_MAGIC);
        c
    }

    fn load(&self, a: u64, size: u32) -> u64 {
        (0..size as u64).fold(0, |acc, k| acc | (*self.mem.get(&(a + k)).unwrap_or(&0) as u64) << (8 * k))
    }

    fn store(&mut self, a: u64, size: u32, v: u64) {
        for k in 0..size as u64 {
            self.mem.insert(a + k, (v >> (8 * k)) as u8);
        }
    }

    fn push(&mut self, v: u64) {
        self.r[4] = self.r[4].wrapping_sub(8);
        self.store(self.r[4], 8, v);
    }

    fn pop(&mut self) -> u64 {
        let v = self.load(self.r[4], 8);
        self.r[4] = self.r[4].wrapping_add(8);
        v
    }

    /// Condition code `cc` (low nibble of `Jcc`/`SETcc`/`CMOVcc`).
    fn cond(&self, cc: u8) -> Result<bool, String> {
        let (zf, sf, cf, of) = self.flags.ok_or("x86: branch on unmodelled flags")?;
        let t = match cc >> 1 {
            0 => of,
            1 => cf,
            2 => zf,
            3 => cf || zf,
            4 => sf,
            5 => return Err("x86: parity condition".into()),
            6 => sf != of,
            _ => zf || sf != of,
        };
        Ok(if cc & 1 == 1 { !t } else { t })
    }

    fn get(&self, rm: &Rm, size: u32) -> u64 {
        let v = match rm {
            Rm::Reg(i) if size == 1 && *i >= 16 => self.r[*i - 16] >> 8,
            Rm::Reg(i) => self.r[*i],
            Rm::Mem(a) => self.load(*a, size),
        };
        if size == 8 {
            v
        } else {
            v & ((1u64 << (8 * size)) - 1)
        }
    }

    /// 32-bit register writes zero-extend; 64-bit write whole.
    fn put(&mut self, rm: &Rm, size: u32, v: u64) {
        match rm {
            Rm::Reg(i) => self.r[*i] = if size == 8 { v } else { v & 0xffff_ffff },
            Rm::Mem(a) => self.store(*a, size, v),
        }
    }

    /// Runs from `code[start]` until the outermost `ret`; returns `rax`.
    pub fn run(&mut self, code: &[u8], start: usize, limit: u64) -> Result<u64, String> {
        let mut pc = start;
        loop {
            self.steps += 1;
            if self.steps > limit {
                return Err("x86 step limit exceeded".into());
            }
            let at = pc;
            let byte = |i: usize| code.get(i).copied().ok_or_else(|| format!("x86: code ends at {i:#x}"));
            let mut rex = 0u8;
            let mut b = byte(pc)?;
            if b == 0xf3 && code.get(pc..pc + 4) == Some(&[0xf3, 0x0f, 0x1e, 0xfa]) {
                pc += 4;
                continue;
            }
            if b == 0x66 && code.get(pc + 1) == Some(&0x90) {
                pc += 2;
                continue;
            }
            if (0x40..=0x4f).contains(&b) {
                rex = b;
                pc += 1;
                b = byte(pc)?;
            }
            let w = rex & 8 != 0;
            let size = if w { 8 } else { 4 };
            let (rr, rx, rb) = (((rex >> 2) & 1) as usize * 8, ((rex >> 1) & 1) as usize * 8, (rex & 1) as usize * 8);
            pc += 1;
            // ModRM decode at pc: (reg field, operand, new pc).
            let modrm = |cpu: &Cpu, p: usize| -> Result<(usize, Rm, usize), String> {
                let m = byte(p)?;
                let (md, reg, rm) = (m >> 6, ((m >> 3) & 7) as usize + rr, (m & 7) as usize);
                let mut p = p + 1;
                if md == 3 {
                    return Ok((reg, Rm::Reg(rm + rb), p));
                }
                let mut addr: u64;
                if rm == 4 {
                    let sib = byte(p)?;
                    p += 1;
                    let (sc, idx, base) = (sib >> 6, ((sib >> 3) & 7) as usize + rx, (sib & 7) as usize);
                    addr = if base == 5 && md == 0 {
                        let d = i32::from_le_bytes(code.get(p..p + 4).ok_or("x86: truncated")?.try_into().unwrap());
                        p += 4;
                        d as i64 as u64
                    } else {
                        cpu.r[base + rb]
                    };
                    if idx != 4 {
                        addr = addr.wrapping_add(cpu.r[idx] << sc);
                    }
                } else if rm == 5 && md == 0 {
                    return Err(format!("x86: rip-relative operand at {at:#x}"));
                } else {
                    addr = cpu.r[rm + rb];
                }
                match md {
                    1 => {
                        addr = addr.wrapping_add(byte(p)? as i8 as i64 as u64);
                        p += 1;
                    }
                    2 => {
                        addr = addr.wrapping_add(i32::from_le_bytes(code.get(p..p + 4).ok_or("x86: truncated")?.try_into().unwrap()) as i64 as u64);
                        p += 4;
                    }
                    _ => {}
                }
                Ok((reg, Rm::Mem(addr), p))
            };
            let imm32 = |p: usize| -> Result<u64, String> { Ok(i32::from_le_bytes(code.get(p..p + 4).ok_or("x86: truncated")?.try_into().unwrap()) as i64 as u64) };
            let mask = |v: u64| if w { v } else { v & 0xffff_ffff };
            let alu = |op: usize, x: u64, y: u64| -> Option<u64> {
                Some(match op {
                    0 => x.wrapping_add(y),
                    1 => x | y,
                    4 => x & y,
                    5 => x.wrapping_sub(y),
                    6 => x ^ y,
                    7 => x, // cmp: flags only
                    _ => return None,
                })
            };
            match b {
                0x90 => {}
                0xc3 => {
                    let ret = self.pop();
                    if ret == RET_MAGIC {
                        return Ok(self.r[0]);
                    }
                    return Err("x86: ret to unknown address".into());
                }
                0xc9 => {
                    self.r[4] = self.r[5];
                    self.r[5] = self.pop();
                }
                0x50..=0x57 => {
                    let v = self.r[(b - 0x50) as usize + rb];
                    self.push(v)
                }
                0x58..=0x5f => {
                    let v = self.pop();
                    self.r[(b - 0x58) as usize + rb] = v;
                }
                0xb8..=0xbf => {
                    let r = (b - 0xb8) as usize + rb;
                    if w {
                        self.r[r] = u64::from_le_bytes(code.get(pc..pc + 8).ok_or("x86: truncated")?.try_into().unwrap());
                        pc += 8;
                    } else {
                        self.r[r] = imm32(pc)? & 0xffff_ffff;
                        pc += 4;
                    }
                }
                0x89 | 0x8b | 0x01 | 0x03 | 0x09 | 0x0b | 0x21 | 0x23 | 0x29 | 0x2b | 0x31 | 0x33 | 0x39 | 0x3b | 0x85 => {
                    let (reg, rm, p) = modrm(self, pc)?;
                    pc = p;
                    let to_reg = b & 2 != 0 && b != 0x85;
                    let (x, y) = if to_reg { (self.r[reg], self.get(&rm, size)) } else { (self.get(&rm, size), self.r[reg]) };
                    let res = match b {
                        0x89 | 0x8b => Some(y),
                        0x85 => {
                            self.flags = Some(flags_of(FlagOp::Logic, x, y, x & y, size));
                            None
                        }
                        0x39 | 0x3b => {
                            self.flags = Some(flags_of(FlagOp::Sub, x, y, x.wrapping_sub(y), size));
                            None
                        }
                        _ => {
                            let op = ((b >> 3) & 7) as usize;
                            let v = alu(op, x, y);
                            if let Some(v) = v {
                                self.flags = Some(flags_of(alu_kind(op), x, y, v, size));
                            }
                            v
                        }
                    };
                    if let Some(v) = res {
                        if to_reg {
                            self.put(&Rm::Reg(reg), size, mask(v));
                        } else {
                            self.put(&rm, size, mask(v));
                        }
                    }
                }
                0x05 | 0x0d | 0x25 | 0x2d | 0x35 | 0x3d | 0xa9 => {
                    let (x, y) = (self.r[0], imm32(pc)?);
                    pc += 4;
                    let op = if b == 0xa9 { 4 } else { ((b >> 3) & 7) as usize };
                    let v = alu(op, x, y).unwrap();
                    self.flags = Some(flags_of(if b == 0x3d { FlagOp::Sub } else { alu_kind(op) }, x, y, if b == 0x3d { x.wrapping_sub(y) } else { v }, size));
                    if b != 0x3d && b != 0xa9 {
                        self.put(&Rm::Reg(0), size, mask(v));
                    }
                }
                0x81 | 0x83 => {
                    let (op, rm, p) = modrm(self, pc)?;
                    let (y, n) = if b == 0x81 { (imm32(p)?, 4) } else { (byte(p)? as i8 as i64 as u64, 1) };
                    pc = p + n;
                    let x = self.get(&rm, size);
                    let v = alu(op - rr, x, y).ok_or_else(|| format!("x86: unsupported group-1 op /{} at {at:#x}", op - rr))?;
                    if op - rr != 7 {
                        self.flags = Some(flags_of(alu_kind(op - rr), x, y, v, size));
                        self.put(&rm, size, mask(v));
                    } else {
                        self.flags = Some(flags_of(FlagOp::Sub, x, y, x.wrapping_sub(y), size));
                    }
                }
                0x70..=0x7f => {
                    let rel = byte(pc)? as i8 as i64;
                    pc += 1;
                    if self.cond(b & 0xf)? {
                        pc = (pc as i64 + rel) as usize;
                    }
                }
                0xeb => {
                    let rel = byte(pc)? as i8 as i64;
                    pc = (pc as i64 + 1 + rel) as usize;
                }
                0xe9 => {
                    let rel = imm32(pc)? as i64;
                    pc = (pc as i64 + 4 + rel) as usize;
                }
                0x63 => {
                    let (reg, rm, p) = modrm(self, pc)?;
                    pc = p;
                    let v = self.get(&rm, 4) as u32 as i32 as i64 as u64;
                    self.put(&Rm::Reg(reg), size, mask(v));
                }
                0xc7 => {
                    let (_, rm, p) = modrm(self, pc)?;
                    let y = imm32(p)?;
                    pc = p + 4;
                    self.put(&rm, size, mask(y));
                }
                0x8d => {
                    let (reg, rm, p) = modrm(self, pc)?;
                    pc = p;
                    let Rm::Mem(a) = rm else { return Err("x86: lea with register operand".into()) };
                    self.put(&Rm::Reg(reg), size, mask(a));
                }
                0xc1 | 0xd1 | 0xd3 => {
                    let (op, rm, p) = modrm(self, pc)?;
                    let (n, len) = match b {
                        0xc1 => (byte(p)? as u32, 1),
                        0xd1 => (1, 0),
                        _ => (self.r[1] as u32, 0),
                    };
                    pc = p + len;
                    let bits = 8 * size;
                    let n = n & if w { 63 } else { 31 };
                    let x = self.get(&rm, size);
                    let v = match op - rr {
                        0 => {
                            if w {
                                x.rotate_left(n)
                            } else {
                                (x as u32).rotate_left(n) as u64
                            }
                        }
                        1 => {
                            if w {
                                x.rotate_right(n)
                            } else {
                                (x as u32).rotate_right(n) as u64
                            }
                        }
                        4 | 6 => x << n,
                        5 => x >> n,
                        7 => {
                            if w {
                                ((x as i64) >> n) as u64
                            } else {
                                ((x as u32 as i32) >> n) as u32 as u64
                            }
                        }
                        o => return Err(format!("x86: unsupported shift /{o} at {at:#x}")),
                    };
                    let _ = bits;
                    self.flags = None;
                    self.put(&rm, size, mask(v));
                }
                0xf7 => {
                    let (op, rm, p) = modrm(self, pc)?;
                    pc = p;
                    let x = self.get(&rm, size);
                    let v = match op - rr {
                        2 => !x,
                        3 => x.wrapping_neg(),
                        o => return Err(format!("x86: unsupported f7 /{o} at {at:#x}")),
                    };
                    self.put(&rm, size, mask(v));
                }
                0x69 | 0x6b => {
                    let (reg, rm, p) = modrm(self, pc)?;
                    let (y, n) = if b == 0x69 { (imm32(p)?, 4) } else { (byte(p)? as i8 as i64 as u64, 1) };
                    pc = p + n;
                    let v = self.get(&rm, size).wrapping_mul(y);
                    self.flags = None;
                    self.put(&Rm::Reg(reg), size, mask(v));
                }
                0x0f => {
                    let b2 = byte(pc)?;
                    pc += 1;
                    match b2 {
                        0xaf => {
                            let (reg, rm, p) = modrm(self, pc)?;
                            pc = p;
                            let v = self.r[reg].wrapping_mul(self.get(&rm, size));
                            self.flags = None;
                            self.put(&Rm::Reg(reg), size, mask(v));
                        }
                        0xc8..=0xcf => {
                            let r = (b2 - 0xc8) as usize + rb;
                            self.r[r] = if w { self.r[r].swap_bytes() } else { (self.r[r] as u32).swap_bytes() as u64 };
                        }
                        0x1f => {
                            let (_, _, p) = modrm(self, pc)?;
                            pc = p;
                        }
                        0xb6 | 0xb7 | 0xbe | 0xbf => {
                            let (reg, rm, p) = modrm(self, pc)?;
                            pc = p;
                            let src = if b2 & 1 == 0 { 1 } else { 2 };
                            // Without REX, byte registers 4-7 are ah/ch/dh/bh.
                            let rm = match rm {
                                Rm::Reg(i) if src == 1 && rex == 0 && (4..8).contains(&i) => Rm::Reg(i - 4 + 16),
                                o => o,
                            };
                            let x = self.get(&rm, src);
                            let v = if b2 >= 0xbe { (((x << (64 - 8 * src)) as i64) >> (64 - 8 * src)) as u64 } else { x };
                            self.put(&Rm::Reg(reg), size, mask(v));
                        }
                        0x80..=0x8f => {
                            let rel = imm32(pc)?;
                            pc += 4;
                            if self.cond(b2 & 0xf)? {
                                pc = (pc as i64 + rel as i64) as usize;
                            }
                        }
                        0x40..=0x4f => {
                            let (reg, rm, p) = modrm(self, pc)?;
                            pc = p;
                            if self.cond(b2 & 0xf)? {
                                let v = self.get(&rm, size);
                                self.put(&Rm::Reg(reg), size, v);
                            } else if !w {
                                self.r[reg] &= 0xffff_ffff;
                            }
                        }
                        0x90..=0x9f => {
                            let (_, rm, p) = modrm(self, pc)?;
                            pc = p;
                            let c = self.cond(b2 & 0xf)? as u64;
                            match rm {
                                Rm::Reg(i) if rex == 0 && (4..8).contains(&i) => self.r[i - 4] = (self.r[i - 4] & !0xff00) | c << 8,
                                Rm::Reg(i) => self.r[i] = (self.r[i] & !0xff) | c,
                                m => self.put(&m, 1, c),
                            }
                        }
                        _ => return Err(format!("x86: unsupported opcode 0f {b2:02x} at {at:#x}")),
                    }
                }
                _ => return Err(format!("x86: unsupported opcode {b:02x} at {at:#x}")),
            }
        }
    }
}

fn alu_kind(op: usize) -> FlagOp {
    match op {
        0 => FlagOp::Add,
        5 | 7 => FlagOp::Sub,
        _ => FlagOp::Logic,
    }
}

/// Calls `code[start]` with System V integer arguments; returns `rax`.
pub fn call(code: &[u8], start: usize, args: &[u64]) -> Result<u64, String> {
    const ARGS: [usize; 6] = [7, 6, 2, 1, 8, 9];
    if args.len() > ARGS.len() {
        return Err("x86: too many arguments".into());
    }
    let mut cpu = Cpu::new();
    for (i, &a) in args.iter().enumerate() {
        cpu.r[ARGS[i]] = a;
    }
    cpu.run(code, start, 1_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn leaf_mixer() {
        // d15 libarena.so nativeShare(J)J: add/xor/rol/shr/imul with movabs constants.
        let code = hex("4889d048b90d57f4c15f865a604801c848b922cb672417099d1c4831c848c1c02548b901d6bff0d60d3bfd4801c84889c148c1e9154831c848b9cb5d413e694ed7844801c848c1c00448b965e4b1cc1e8d7281480fafc1c3");
        let x: u64 = 0x0123_4567_89ab_cdef;
        let mut e = x.wrapping_add(0x605a865fc1f4570d) ^ 0x1c9d09172467cb22;
        e = e.rotate_left(0x25).wrapping_add(0xfd3b0dd6f0bfd601);
        e ^= e >> 0x15;
        e = e.wrapping_add(0x84d74e693e415dcb).rotate_left(4).wrapping_mul(0x81728d1eccb1e465);
        assert_eq!(call(&code, 0, &[0, 0, x]).unwrap(), e);
    }

    #[test]
    fn guarded_branches() {
        // d16 nativeShare(JI)J: returns 0 unless the int is > 0xffff with low 16 bits 0x12a2.
        let code = hex("81f9ffff00007e480fb7f181fea2120000753d4889d048b9a5272aba78a4da904831c84889c148c1e92a4831c848c1c0184889c148c1e90a4831c84889c148c1e9254831c84889c148c1e91b4831c8c331c0c3");
        assert_eq!(call(&code, 0, &[0, 0, 5, 0x12a2]).unwrap(), 0);
        assert_eq!(call(&code, 0, &[0, 0, 5, 0x112a3]).unwrap(), 0);
        let mut e = 5u64 ^ 0x90daa478ba2a27a5;
        e ^= e >> 0x2a;
        e = e.rotate_left(0x18);
        for s in [0x0a, 0x25, 0x1b] {
            e ^= e >> s;
        }
        assert_eq!(call(&code, 0, &[0, 0, 5, 0x112a2]).unwrap(), e);
    }
}
