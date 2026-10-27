//! Packet captures: classic libpcap (`a1b2c3d4` usec / `a1b23c4d` nsec, either byte order) and pcapng
//! (SHB/IDB/EPB/SPB/OPB blocks, per-section byte order). Packets are walked record by record; a record
//! whose captured length runs past the end of the file (or past the snaplen / a sane bound) stops the
//! walk and marks the capture malformed. Counts are always of complete records.

use super::bytes::{u32le, Endian};

#[derive(Clone, Debug)]
pub struct Packet {
    pub offset: usize,
    /// Offset of the packet bytes.
    pub data: usize,
    /// Timestamp in nanoseconds since the Unix epoch.
    pub ts_ns: u128,
    pub captured: u32,
    pub original: u32,
    pub interface: u32,
}

#[derive(Debug)]
pub struct Capture {
    pub len: usize,
    pub ng: bool,
    pub big_endian: bool,
    pub nanosecond: bool,
    pub version: (u16, u16),
    pub snaplen: u32,
    pub link_types: Vec<u16>,
    pub packets: Vec<Packet>,
    /// Why the walk stopped early (`None` = the file ended exactly on a record boundary).
    pub malformed: Option<String>,
    /// Bytes after the last complete record.
    pub trailing: usize,
}

const MAX_RECORD: u32 = 0x0400_0000;

impl Capture {
    pub fn parse(b: &[u8]) -> Option<Capture> {
        match u32le(b, 0)? {
            0x0A0D_0D0A => Self::parse_ng(b),
            0xA1B2_C3D4 => Self::parse_classic(b, Endian::Little, false),
            0xA1B2_3C4D => Self::parse_classic(b, Endian::Little, true),
            0xD4C3_B2A1 => Self::parse_classic(b, Endian::Big, false),
            0x4D3C_B2A1 => Self::parse_classic(b, Endian::Big, true),
            _ => None,
        }
    }

    fn parse_classic(b: &[u8], e: Endian, nano: bool) -> Option<Capture> {
        let mut c = Capture {
            len: b.len(),
            ng: false,
            big_endian: e == Endian::Big,
            nanosecond: nano,
            version: (e.u16(b, 4)?, e.u16(b, 6)?),
            snaplen: e.u32(b, 16)?,
            link_types: vec![(e.u32(b, 20)? & 0xFFFF) as u16],
            packets: Vec::new(),
            malformed: None,
            trailing: 0,
        };
        let mut p = 24usize;
        while p < b.len() {
            let (Some(sec), Some(frac), Some(incl), Some(orig)) = (e.u32(b, p), e.u32(b, p + 4), e.u32(b, p + 8), e.u32(b, p + 12)) else {
                c.malformed = Some(format!("partial record header at {p:#x}"));
                break;
            };
            if incl > MAX_RECORD {
                c.malformed = Some(format!("record at {p:#x}: incl_len {incl} is implausible"));
                break;
            }
            let end = p + 16 + incl as usize;
            if end > b.len() {
                c.malformed = Some(format!("record at {p:#x}: incl_len {incl} runs past EOF ({} B left)", b.len() - p - 16));
                break;
            }
            if c.snaplen != 0 && incl > c.snaplen {
                c.malformed = Some(format!("record at {p:#x}: incl_len {incl} > snaplen {}", c.snaplen));
                break;
            }
            if incl > orig {
                c.malformed = Some(format!("record at {p:#x}: incl_len {incl} > orig_len {orig}"));
                break;
            }
            if !nano && frac >= 1_000_000 || nano && frac >= 1_000_000_000 {
                c.malformed = Some(format!("record at {p:#x}: fractional timestamp {frac} out of range"));
                break;
            }
            let ts_ns = sec as u128 * 1_000_000_000 + if nano { frac as u128 } else { frac as u128 * 1000 };
            c.packets.push(Packet { offset: p, data: p + 16, ts_ns, captured: incl, original: orig, interface: 0 });
            p = end;
        }
        c.trailing = b.len() - c.packets.last().map(|k| k.offset + 16 + k.captured as usize).unwrap_or(24.min(b.len()));
        Some(c)
    }

    fn parse_ng(b: &[u8]) -> Option<Capture> {
        let mut c = Capture {
            len: b.len(),
            ng: true,
            big_endian: false,
            nanosecond: false,
            version: (0, 0),
            snaplen: 0,
            link_types: Vec::new(),
            packets: Vec::new(),
            malformed: None,
            trailing: 0,
        };
        // Per-interface timestamp units per second (if_tsresol, default 10^6).
        let mut units: Vec<u128> = Vec::new();
        let mut e = Endian::Little;
        let mut p = 0usize;
        let mut last_end = 0usize;
        while p < b.len() {
            let Some(kind) = e.u32(b, p).or_else(|| u32le(b, p)) else {
                c.malformed = Some(format!("partial block header at {p:#x}"));
                break;
            };
            if kind == 0x0A0D_0D0A {
                e = match u32le(b, p + 8) {
                    Some(0x1A2B_3C4D) => Endian::Little,
                    Some(0x4D3C_2B1A) => Endian::Big,
                    _ => {
                        c.malformed = Some(format!("bad SHB byte-order magic at {p:#x}"));
                        break;
                    }
                };
                if c.version == (0, 0) {
                    c.big_endian = e == Endian::Big;
                    c.version = (e.u16(b, p + 12)?, e.u16(b, p + 14)?);
                }
                units.clear();
            }
            let Some(blen) = e.u32(b, p + 4) else {
                c.malformed = Some(format!("partial block header at {p:#x}"));
                break;
            };
            let blen = blen as usize;
            if blen < 12 || blen % 4 != 0 || p + blen > b.len() || e.u32(b, p + blen - 4) != Some(blen as u32) {
                c.malformed = Some(format!("block {kind:#x} at {p:#x}: bad length {blen}"));
                break;
            }
            let body = &b[p + 8..p + blen - 4];
            match kind {
                1 => {
                    c.link_types.push(e.u16(body, 0)?);
                    let snap = e.u32(body, 4)?;
                    if c.snaplen == 0 {
                        c.snaplen = snap;
                    }
                    let mut res = 1_000_000u128;
                    let mut o = 8;
                    while o + 4 <= body.len() {
                        let (code, olen) = (e.u16(body, o)?, e.u16(body, o + 2)? as usize);
                        if code == 0 {
                            break;
                        }
                        if code == 9 && olen >= 1 {
                            let v = body[o + 4];
                            res = if v & 0x80 != 0 { 1u128.checked_shl((v & 0x7F) as u32).unwrap_or(1) } else { 10u128.checked_pow(v as u32).unwrap_or(1) };
                            if res == 1_000_000_000 {
                                c.nanosecond = true;
                            }
                        }
                        o += 4 + olen.div_ceil(4) * 4;
                    }
                    units.push(res);
                }
                6 | 2 => {
                    let (iface, ts, incl, orig, hdr) = if kind == 6 {
                        let ts = ((e.u32(body, 4)? as u64) << 32) | e.u32(body, 8)? as u64;
                        (e.u32(body, 0)?, ts, e.u32(body, 12)?, e.u32(body, 16)?, 20)
                    } else {
                        let ts = ((e.u32(body, 4)? as u64) << 32) | e.u32(body, 8)? as u64;
                        (e.u16(body, 0)? as u32, ts, e.u32(body, 12)?, e.u32(body, 16)?, 20)
                    };
                    if hdr + incl as usize > body.len() {
                        c.malformed = Some(format!("packet block at {p:#x}: captured {incl} exceeds block body"));
                        break;
                    }
                    let Some(&u) = units.get(iface as usize) else {
                        c.malformed = Some(format!("packet block at {p:#x}: unknown interface {iface}"));
                        break;
                    };
                    let ts_ns = ts as u128 * 1_000_000_000 / u;
                    c.packets.push(Packet { offset: p, data: p + 28, ts_ns, captured: incl, original: orig, interface: iface });
                }
                3 => {
                    let orig = e.u32(body, 0)?;
                    let incl = orig.min(c.snaplen.max(1)).min((body.len() - 4) as u32);
                    c.packets.push(Packet { offset: p, data: p + 12, ts_ns: 0, captured: incl, original: orig, interface: 0 });
                }
                _ => {}
            }
            p += blen;
            last_end = p;
        }
        c.trailing = b.len() - last_end;
        Some(c)
    }

    pub fn duration_ns(&self) -> u128 {
        let ts = self.packets.iter().map(|p| p.ts_ns);
        match (ts.clone().min(), ts.max()) {
            (Some(a), Some(z)) => z - a,
            _ => 0,
        }
    }

    pub fn data<'a>(&self, b: &'a [u8], p: &Packet) -> &'a [u8] {
        b.get(p.data..p.data + p.captured as usize).unwrap_or(&[])
    }
}

/// `YYYY-MM-DDTHH:MM:SS.ffffff[fff]Z` for a nanosecond Unix timestamp (micro unless `nano`).
pub fn ts_iso(ns: u128, nano: bool) -> String {
    let secs = (ns / 1_000_000_000) as i64;
    let s = crate::time::iso(secs * 1000);
    let frac = ns % 1_000_000_000;
    if nano { format!("{}.{frac:09}Z", &s[..19]) } else { format!("{}.{:06}Z", &s[..19], frac / 1000) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_little_endian() {
        let b = std::fs::read("testdata/d27-capture-001.pcap").unwrap();
        let c = Capture::parse(&b).unwrap();
        assert!(!c.big_endian && !c.ng && c.malformed.is_none());
        assert_eq!(c.packets.len(), 3);
        assert_eq!(c.link_types, vec![147]);
        assert_eq!(c.data(&b, &c.packets[2]), b"PCAP-PROOF");
        assert_eq!(c.packets[2].original, 64);
        let mut t = b.clone();
        t.truncate(b.len() - 1);
        let c = Capture::parse(&t).unwrap();
        assert_eq!(c.packets.len(), 2);
        assert!(c.malformed.is_some());
    }

    #[test]
    fn pcapng() {
        let mut b = Vec::new();
        let blk = |b: &mut Vec<u8>, kind: u32, body: &[u8]| {
            let n = (12 + body.len()) as u32;
            b.extend_from_slice(&kind.to_le_bytes());
            b.extend_from_slice(&n.to_le_bytes());
            b.extend_from_slice(body);
            b.extend_from_slice(&n.to_le_bytes());
        };
        blk(&mut b, 0x0A0D_0D0A, &[0x4d, 0x3c, 0x2b, 0x1a, 1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
        blk(&mut b, 1, &[1, 0, 0, 0, 0, 0, 4, 0, 9, 0, 1, 0, 9, 0, 0, 0, 0, 0, 0, 0]);
        let mut epb = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0xca, 0x9a, 0x3b, 4, 0, 0, 0, 4, 0, 0, 0];
        epb.extend_from_slice(b"ABCD");
        blk(&mut b, 6, &epb);
        let c = Capture::parse(&b).unwrap();
        assert!(c.ng && c.malformed.is_none() && c.nanosecond);
        assert_eq!(c.packets.len(), 1);
        assert_eq!(c.packets[0].ts_ns, 1_000_000_000);
        assert_eq!(c.data(&b, &c.packets[0]), b"ABCD");
    }
}
