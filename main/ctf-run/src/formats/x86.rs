//! A deliberately tiny x86-64 decoder for short leaf functions (constant returns, prologues),
//! printing objdump's Intel syntax (`mov eax,0x1fa4`). Anything it does not know is an error, so a
//! new instruction shape fails loudly instead of being mis-decoded.

const R32: [&str; 8] = ["eax", "ecx", "edx", "ebx", "esp", "ebp", "esi", "edi"];
const R64: [&str; 8] = ["rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi"];

fn imm(v: u64) -> String {
    format!("0x{v:x}")
}

/// Decodes `code` up to and including the first `ret`.
pub fn disassemble(code: &[u8]) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < code.len() {
        let b = &code[i..];
        let (text, len) = match b {
            [0xC3, ..] => ("ret".to_string(), 1),
            [0x90, ..] => ("nop".to_string(), 1),
            [0xC9, ..] => ("leave".to_string(), 1),
            [0xF3, 0x0F, 0x1E, 0xFA, ..] => ("endbr64".to_string(), 4),
            [r @ 0x50..=0x57, ..] => (format!("push {}", R64[(*r - 0x50) as usize]), 1),
            [r @ 0x58..=0x5F, ..] => (format!("pop {}", R64[(*r - 0x58) as usize]), 1),
            [0x48, 0x89, 0xE5, ..] => ("mov rbp,rsp".to_string(), 3),
            [r @ 0xB8..=0xBF, a, b2, c, d, ..] => (format!("mov {},{}", R32[(*r - 0xB8) as usize], imm(u32::from_le_bytes([*a, *b2, *c, *d]) as u64)), 5),
            [0x31, m @ 0xC0..=0xFF, ..] | [0x33, m @ 0xC0..=0xFF, ..] => {
                let (rm, reg) = ((*m & 7) as usize, ((*m >> 3) & 7) as usize);
                let (dst, src) = if b[0] == 0x31 { (rm, reg) } else { (reg, rm) };
                (format!("xor {},{}", R32[dst], R32[src]), 2)
            }
            _ => return Err(format!("unsupported instruction bytes {:02x?} at +{i}", &b[..b.len().min(8)])),
        };
        out.push(text);
        i += len;
        if out.last().map(String::as_str) == Some("ret") {
            break;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_return() {
        assert_eq!(disassemble(&[0xB8, 0xA4, 0x1F, 0, 0, 0xC3]).unwrap(), ["mov eax,0x1fa4", "ret"]);
        assert_eq!(disassemble(&[0xF3, 0x0F, 0x1E, 0xFA, 0x31, 0xC0, 0xC3]).unwrap(), ["endbr64", "xor eax,eax", "ret"]);
        assert!(disassemble(&[0x0F, 0x05]).is_err());
    }
}
