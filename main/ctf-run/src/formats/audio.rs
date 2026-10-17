//! Audio containers sniffed from their bytes (never the extension): RIFF/RIFX/RF64 WAVE, AIFF/AIFC,
//! FLAC, Ogg (Vorbis/Opus/FLAC/Speex), MPEG audio (ID3 + frame sync), ADTS AAC, AMR, Sun `.snd`,
//! MP4/M4A (`stsd`) and Matroska/WebM (`CodecID`). Codec names follow ffprobe's `codec_name`
//! (`pcm_s16le`, graded on d17). The primary stream is the first audio stream.

use super::bytes::{find, u16be, u16le, u32be, u32le};

#[derive(Debug, Clone, Default)]
pub struct Audio {
    /// ffprobe-style container name (`wav`, `aiff`, `flac`, `ogg`, `mp3`, `mp4`, `matroska`, ...).
    pub container: &'static str,
    pub codec: String,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub bits: Option<u16>,
    /// Payload bytes (WAV `data`, AIFF `SSND`), for duration and equivalence hashing.
    pub payload: Option<(usize, usize)>,
    /// Text tags (RIFF INFO, ID3 text frames, Vorbis comments), in file order.
    pub tags: Vec<(String, String)>,
}

impl Audio {
    /// PCM payload length / byte rate.
    pub fn duration_seconds(&self) -> Option<f64> {
        let (_, len) = self.payload?;
        let bps = self.sample_rate? as f64 * self.channels? as f64 * self.bits? as f64 / 8.0;
        (bps > 0.0 && self.codec.starts_with("pcm_")).then(|| len as f64 / bps)
    }
}

pub fn sniff(d: &[u8]) -> Option<Audio> {
    let m = |s: &[u8]| d.starts_with(s);
    if (m(b"RIFF") || m(b"RIFX") || m(b"RF64") || m(b"BW64")) && d.get(8..12) == Some(b"WAVE") {
        return wav(d);
    }
    if m(b"FORM") && matches!(d.get(8..12), Some(b"AIFF") | Some(b"AIFC")) {
        return aiff(d);
    }
    if m(b"fLaC") {
        return flac(d, 4).map(|mut a| {
            a.container = "flac";
            a
        });
    }
    if m(b"OggS") {
        return ogg(d);
    }
    if m(b"#!AMR-WB\n") {
        return Some(Audio { container: "amr", codec: "amr_wb".into(), sample_rate: Some(16000), channels: Some(1), ..Default::default() });
    }
    if m(b"#!AMR\n") {
        return Some(Audio { container: "amr", codec: "amr_nb".into(), sample_rate: Some(8000), channels: Some(1), ..Default::default() });
    }
    if m(b".snd") {
        return snd(d);
    }
    if d.get(4..8) == Some(b"ftyp") {
        return mp4(d);
    }
    if m(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return matroska(d);
    }
    mpeg(d)
}

/// WAVE format tag (+ bits, endianness) -> ffprobe codec name.
pub fn wav_codec(tag: u16, bits: u16, be: bool) -> String {
    let e = if be { "be" } else { "le" };
    match (tag, bits) {
        (1, 8) => "pcm_u8".into(),
        (1, b @ (16 | 24 | 32 | 64)) => format!("pcm_s{b}{e}"),
        (3, b @ (32 | 64)) => format!("pcm_f{b}{e}"),
        (2, _) => "adpcm_ms".into(),
        (6, _) => "pcm_alaw".into(),
        (7, _) => "pcm_mulaw".into(),
        (0x11, _) => "adpcm_ima_wav".into(),
        (0x14, _) | (0x40, _) | (0x45, _) => "adpcm_g726".into(),
        (0x22, _) => "truespeech".into(),
        (0x31, _) => "gsm_ms".into(),
        (0x50, _) => "mp2".into(),
        (0x55, _) => "mp3".into(),
        (0xFF, _) | (0x1610, _) | (0x4143, _) => "aac".into(),
        (0x160, _) => "wmav1".into(),
        (0x161, _) => "wmav2".into(),
        (0x162, _) => "wmapro".into(),
        (0x2000, _) => "ac3".into(),
        (0x2001, _) => "dts".into(),
        (0xF1AC, _) => "flac".into(),
        (0x674F..=0x6752, _) => "vorbis".into(),
        (0x704F, _) | (0x7050, _) => "opus".into(),
        (t, b) => format!("unknown_wav_tag_0x{t:04x}_{b}"),
    }
}

fn wav(d: &[u8]) -> Option<Audio> {
    let be = d.starts_with(b"RIFX");
    let u16f = |at: usize| if be { u16be(d, at) } else { u16le(d, at) };
    let u32f = |at: usize| if be { u32be(d, at) } else { u32le(d, at) };
    let mut a = Audio { container: "wav", ..Default::default() };
    let mut p = 12;
    let mut ds64_data: Option<u64> = None;
    let mut have_fmt = false;
    while p + 8 <= d.len() {
        let id = &d[p..p + 4];
        let size = u32f(p + 4)? as usize;
        let body = p + 8;
        match id {
            b"ds64" => ds64_data = super::bytes::u64le(d, body + 8),
            b"fmt " => {
                let mut tag = u16f(body)?;
                a.channels = u16f(body + 2);
                a.sample_rate = u32f(body + 4);
                let bits = u16f(body + 14).unwrap_or(0);
                a.bits = Some(bits);
                if tag == 0xFFFE && size >= 40 {
                    // WAVEFORMATEXTENSIBLE: the sub-format GUID starts with the real tag.
                    tag = u16f(body + 24)?;
                }
                a.codec = wav_codec(tag, bits, be);
                have_fmt = true;
            }
            b"data" => {
                let len = if size == 0xFFFF_FFFF { ds64_data.map(|n| n as usize).unwrap_or(d.len() - body) } else { size };
                a.payload = Some((body, len.min(d.len().saturating_sub(body))));
            }
            b"LIST" if d.get(body..body + 4) == Some(b"INFO") => {
                let end = (body + size).min(d.len());
                let mut q = body + 4;
                while q + 8 <= end {
                    let n = u32f(q + 4)? as usize;
                    let v = d.get(q + 8..(q + 8 + n).min(end))?;
                    let v = String::from_utf8_lossy(v).trim_end_matches('\0').to_string();
                    a.tags.push((String::from_utf8_lossy(&d[q..q + 4]).into_owned(), v));
                    q += 8 + n + (n & 1);
                }
            }
            _ => {}
        }
        if size == 0xFFFF_FFFF {
            break;
        }
        p = body + size + (size & 1);
    }
    have_fmt.then_some(a)
}

/// 80-bit IEEE extended (AIFF sample rate).
fn ext80(b: &[u8]) -> Option<u32> {
    let e = (u16be(b, 0)? & 0x7FFF) as i32 - 16383;
    let m = super::bytes::u64be(b, 2)?;
    if !(0..=63).contains(&e) {
        return None;
    }
    Some((m >> (63 - e)) as u32)
}

fn aiff(d: &[u8]) -> Option<Audio> {
    let aifc = d.get(8..12) == Some(b"AIFC");
    let mut a = Audio { container: "aiff", ..Default::default() };
    let mut p = 12;
    let mut have = false;
    while p + 8 <= d.len() {
        let id = &d[p..p + 4];
        let size = u32be(d, p + 4)? as usize;
        let body = p + 8;
        match id {
            b"COMM" => {
                a.channels = u16be(d, body);
                let bits = u16be(d, body + 6)?;
                a.bits = Some(bits);
                a.sample_rate = d.get(body + 8..body + 18).and_then(ext80);
                let comp = if aifc { d.get(body + 18..body + 22) } else { None };
                a.codec = match comp {
                    None | Some(b"NONE") | Some(b"twos") => match bits {
                        1..=8 => "pcm_s8".into(),
                        b => format!("pcm_s{}be", b.div_ceil(8) * 8),
                    },
                    Some(b"sowt") => match bits {
                        1..=8 => "pcm_s8".into(),
                        b => format!("pcm_s{}le", b.div_ceil(8) * 8),
                    },
                    Some(b"raw ") => "pcm_u8".into(),
                    Some(b"in24") => "pcm_s24be".into(),
                    Some(b"in32") => "pcm_s32be".into(),
                    Some(b"fl32") | Some(b"FL32") => "pcm_f32be".into(),
                    Some(b"fl64") | Some(b"FL64") => "pcm_f64be".into(),
                    Some(b"alaw") | Some(b"ALAW") => "pcm_alaw".into(),
                    Some(b"ulaw") | Some(b"ULAW") => "pcm_mulaw".into(),
                    Some(b"ima4") => "adpcm_ima_qt".into(),
                    Some(b"MAC3") => "mace3".into(),
                    Some(b"MAC6") => "mace6".into(),
                    Some(b"GSM ") => "gsm".into(),
                    Some(b"QDM2") => "qdm2".into(),
                    Some(c) => format!("unknown_aifc_{}", String::from_utf8_lossy(c)),
                };
                have = true;
            }
            b"SSND" => {
                let off = u32be(d, body)? as usize;
                let start = body + 8 + off;
                a.payload = Some((start, size.saturating_sub(8 + off).min(d.len().saturating_sub(start))));
            }
            b"NAME" | b"AUTH" | b"ANNO" | b"(c) " => {
                let v = d.get(body..(body + size).min(d.len()))?;
                a.tags.push((String::from_utf8_lossy(id).into_owned(), String::from_utf8_lossy(v).trim_end_matches('\0').to_string()));
            }
            _ => {}
        }
        p = body + size + (size & 1);
    }
    have.then_some(a)
}

/// FLAC metadata blocks from `p` (after `fLaC`): STREAMINFO + VORBIS_COMMENT.
fn flac(d: &[u8], mut p: usize) -> Option<Audio> {
    let mut a = Audio { container: "flac", codec: "flac".into(), ..Default::default() };
    loop {
        let h = *d.get(p)?;
        let len = (u32be(d, p)? & 0xFF_FFFF) as usize;
        let body = p + 4;
        match h & 0x7F {
            0 => {
                let x = super::bytes::u64be(d, body + 10)?;
                a.sample_rate = Some((x >> 44) as u32);
                a.channels = Some(((x >> 41) & 7) as u16 + 1);
                a.bits = Some(((x >> 36) & 31) as u16 + 1);
            }
            4 => a.tags.extend(vorbis_comments(d.get(body..body + len)?)),
            _ => {}
        }
        if h & 0x80 != 0 {
            break;
        }
        p = body + len;
    }
    Some(a)
}

fn vorbis_comments(b: &[u8]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Some(vl) = u32le(b, 0) else { return out };
    let mut p = 4 + vl as usize;
    if let Some(v) = b.get(4..p) {
        out.push(("vendor".into(), String::from_utf8_lossy(v).into_owned()));
    }
    let Some(n) = u32le(b, p) else { return out };
    p += 4;
    for _ in 0..n.min(4096) {
        let Some(l) = u32le(b, p) else { break };
        let Some(s) = b.get(p + 4..p + 4 + l as usize) else { break };
        let s = String::from_utf8_lossy(s);
        let (k, v) = s.split_once('=').unwrap_or((&s, ""));
        out.push((k.to_string(), v.to_string()));
        p += 4 + l as usize;
    }
    out
}

/// Ogg pages -> per-serial packets (first two packets of each logical stream are enough).
fn ogg(d: &[u8]) -> Option<Audio> {
    use std::collections::BTreeMap;
    let mut streams: BTreeMap<u32, (usize, Vec<Vec<u8>>, Vec<u8>)> = BTreeMap::new();
    let mut p = 0;
    let mut order = 0;
    while d.get(p..p + 4) == Some(b"OggS") {
        let serial = u32le(d, p + 14)?;
        let nseg = *d.get(p + 26)? as usize;
        let segs = d.get(p + 27..p + 27 + nseg)?;
        let mut q = p + 27 + nseg;
        let e = streams.entry(serial).or_insert_with(|| {
            order += 1;
            (order, Vec::new(), Vec::new())
        });
        for &s in segs {
            let chunk = d.get(q..q + s as usize)?;
            q += s as usize;
            if e.1.len() < 3 {
                e.2.extend_from_slice(chunk);
                if s < 255 {
                    e.1.push(std::mem::take(&mut e.2));
                }
            }
        }
        p = q;
    }
    let mut ordered: Vec<_> = streams.into_values().collect();
    ordered.sort_by_key(|s| s.0);
    for (_, pk, _) in ordered {
        let first = pk.first()?;
        let mut a = Audio { container: "ogg", ..Default::default() };
        if first.starts_with(b"\x01vorbis") {
            a.codec = "vorbis".into();
            a.channels = first.get(11).map(|&c| c as u16);
            a.sample_rate = u32le(first, 12);
            if let Some(c) = pk.get(1).filter(|c| c.starts_with(b"\x03vorbis")) {
                a.tags = vorbis_comments(&c[7..]);
            }
        } else if first.starts_with(b"OpusHead") {
            a.codec = "opus".into();
            a.channels = first.get(9).map(|&c| c as u16);
            a.sample_rate = Some(48000);
            if let Some(c) = pk.get(1).filter(|c| c.starts_with(b"OpusTags")) {
                a.tags = vorbis_comments(&c[8..]);
            }
        } else if first.starts_with(b"\x7fFLAC") {
            let mut f = flac(first, 13).unwrap_or_default();
            f.codec = "flac".into();
            f.container = "ogg";
            if let Some(c) = pk.get(1).filter(|c| c.first().map(|h| h & 0x7F) == Some(4)) {
                f.tags = vorbis_comments(&c[4..]);
            }
            return Some(f);
        } else if first.starts_with(b"Speex   ") {
            a.codec = "speex".into();
            a.sample_rate = u32le(first, 36);
            a.channels = u32le(first, 48).map(|c| c as u16);
        } else {
            continue; // video (Theora, ...) or skeleton
        }
        return Some(a);
    }
    None
}

fn snd(d: &[u8]) -> Option<Audio> {
    let off = u32be(d, 4)? as usize;
    let size = u32be(d, 8)? as usize;
    let enc = u32be(d, 12)?;
    let codec = match enc {
        1 => "pcm_mulaw",
        2 => "pcm_s8",
        3 => "pcm_s16be",
        4 => "pcm_s24be",
        5 => "pcm_s32be",
        6 => "pcm_f32be",
        7 => "pcm_f64be",
        23 => "adpcm_g726le",
        27 => "pcm_alaw",
        _ => return None,
    };
    let len = if size == 0xFFFF_FFFF { d.len().saturating_sub(off) } else { size.min(d.len().saturating_sub(off)) };
    let bits = match enc { 2 => 8, 3 => 16, 4 => 24, 5 | 6 => 32, 7 => 64, _ => 8 };
    Some(Audio { container: "au", codec: codec.into(), sample_rate: u32be(d, 16), channels: u32be(d, 20).map(|c| c as u16), bits: Some(bits), payload: Some((off, len)), tags: Vec::new() })
}

/// ID3v2 (skipped, text frames collected) then the first MPEG audio / ADTS frame header.
fn mpeg(d: &[u8]) -> Option<Audio> {
    let mut tags = Vec::new();
    let mut p = 0;
    let mut tagged = false;
    while d.get(p..p + 3) == Some(b"ID3") {
        tagged = true;
        let ver = *d.get(p + 3)?;
        let ss = |at: usize| -> Option<usize> { Some(d.get(at..at + 4)?.iter().fold(0usize, |a, &b| (a << 7) | (b & 0x7F) as usize)) };
        let size = ss(p + 6)?;
        let end = (p + 10 + size).min(d.len());
        let mut q = p + 10;
        while ver >= 3 && q + 10 <= end && d[q] != 0 {
            let id = String::from_utf8_lossy(&d[q..q + 4]).into_owned();
            let n = if ver == 4 { ss(q + 4)? } else { u32be(d, q + 4)? as usize };
            let body = d.get(q + 10..(q + 10 + n).min(end))?;
            if id.starts_with('T') && !body.is_empty() {
                tags.push((id, id3_text(body)));
            }
            q += 10 + n;
        }
        p = end + if d.get(p + 5).map(|f| f & 0x10 != 0) == Some(true) { 10 } else { 0 };
    }
    // Frame sync: allow leading padding only after an ID3 tag.
    let limit = if tagged { d.len().min(p + 4096) } else { 1 };
    for i in p..limit {
        let (Some(&a), Some(&b)) = (d.get(i), d.get(i + 1)) else { break };
        if a != 0xFF || b & 0xE0 != 0xE0 {
            continue;
        }
        let h = u32be(d, i)?;
        let layer = (h >> 17) & 3;
        let version = (h >> 19) & 3;
        if b & 0xF6 == 0xF0 && layer == 0 {
            let rates = [96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350];
            let sr = rates.get(((h >> 10) & 0xF) as usize).copied();
            let ch = ((u16be(d, i + 2)? >> 6) & 7) as u16;
            return Some(Audio { container: "aac", codec: "aac".into(), sample_rate: sr, channels: Some(ch), tags, ..Default::default() });
        }
        if layer == 0 || version == 1 || (h >> 12) & 0xF == 0xF || (h >> 10) & 3 == 3 {
            continue;
        }
        let base = [44100, 48000, 32000][((h >> 10) & 3) as usize];
        let sr = match version { 3 => base, 2 => base / 2, _ => base / 4 };
        let codec = match layer { 1 => "mp3", 2 => "mp2", _ => "mp1" };
        let ch = if (h >> 6) & 3 == 3 { 1 } else { 2 };
        return Some(Audio { container: codec, codec: codec.into(), sample_rate: Some(sr), channels: Some(ch), tags, ..Default::default() });
    }
    None
}

fn id3_text(b: &[u8]) -> String {
    let t = &b[1..];
    let s = match b[0] {
        1 | 2 => {
            let (t, be) = match t {
                [0xFE, 0xFF, r @ ..] => (r, true),
                [0xFF, 0xFE, r @ ..] => (r, false),
                r => (r, b[0] == 2),
            };
            let u: Vec<u16> = t.chunks_exact(2).map(|c| if be { u16::from_be_bytes([c[0], c[1]]) } else { u16::from_le_bytes([c[0], c[1]]) }).collect();
            String::from_utf16_lossy(&u)
        }
        3 => String::from_utf8_lossy(t).into_owned(),
        _ => super::bytes::cp1252(t),
    };
    s.trim_end_matches('\0').replace('\0', "/")
}

/// ISO BMFF: the first `trak` whose `hdlr` is `soun`; codec from its `stsd` sample entry.
fn mp4(d: &[u8]) -> Option<Audio> {
    fn boxes(d: &[u8], mut p: usize, end: usize) -> Vec<(&[u8], usize, usize)> {
        let mut v = Vec::new();
        while p + 8 <= end {
            let Some(mut size) = u32be(d, p).map(|s| s as usize) else { break };
            let mut hdr = 8;
            if size == 1 {
                size = super::bytes::u64be(d, p + 8).unwrap_or(0) as usize;
                hdr = 16;
            } else if size == 0 {
                size = end - p;
            }
            if size < hdr || p + size > end {
                break;
            }
            v.push((&d[p + 4..p + 8], p + hdr, p + size));
            p += size;
        }
        v
    }
    let child = |s: usize, e: usize, k: &[u8]| boxes(d, s, e).into_iter().find(|b| b.0 == k).map(|b| (b.1, b.2));
    let (ms, me) = child(0, d.len(), b"moov")?;
    for (k, ts, te) in boxes(d, ms, me) {
        if k != b"trak" {
            continue;
        }
        let Some((ds, de)) = child(ts, te, b"mdia") else { continue };
        let Some((hs, _)) = child(ds, de, b"hdlr") else { continue };
        if d.get(hs + 8..hs + 12) != Some(b"soun") {
            continue;
        }
        let mut a = Audio { container: "mp4", ..Default::default() };
        let (ns, ne) = child(ds, de, b"minf")?;
        let (ss, se) = child(ns, ne, b"stbl")?;
        let (st, ste) = child(ss, se, b"stsd")?;
        let (fourcc, es, ee) = boxes(d, st + 8, ste).into_iter().next()?;
        a.channels = u16be(d, es + 16);
        a.bits = u16be(d, es + 18);
        a.sample_rate = u16be(d, es + 24).map(|r| r as u32);
        a.codec = match fourcc {
            b"mp4a" => {
                // esds objectTypeIndication: 0x40/0x66-0x68 AAC, 0x69/0x6B MP3.
                let oti = find(&d[es..ee], b"esds").and_then(|i| {
                    let b = &d[es + i..ee];
                    let j = b.iter().position(|&x| x == 0x04)?;
                    let mut k = j + 1;
                    while b.get(k).map(|x| x & 0x80 != 0) == Some(true) {
                        k += 1;
                    }
                    b.get(k + 1).copied()
                });
                match oti {
                    Some(0x69) | Some(0x6B) => "mp3".into(),
                    Some(0xA5) => "ac3".into(),
                    Some(0xA6) => "eac3".into(),
                    _ => "aac".into(),
                }
            }
            b"alac" => "alac".into(),
            b"Opus" => "opus".into(),
            b"fLaC" => "flac".into(),
            b"ac-3" => "ac3".into(),
            b"ec-3" => "eac3".into(),
            b"samr" => "amr_nb".into(),
            b"sawb" => "amr_wb".into(),
            b"ulaw" => "pcm_mulaw".into(),
            b"alaw" => "pcm_alaw".into(),
            b"sowt" => "pcm_s16le".into(),
            b"twos" => "pcm_s16be".into(),
            b".mp3" => "mp3".into(),
            b"ima4" => "adpcm_ima_qt".into(),
            other => format!("unknown_mp4_{}", String::from_utf8_lossy(other)),
        };
        return Some(a);
    }
    None
}

/// Matroska/WebM: EBML walk to the first TrackEntry of TrackType 2 (audio).
fn matroska(d: &[u8]) -> Option<Audio> {
    fn vint(d: &[u8], p: usize, strip: bool) -> Option<(u64, usize)> {
        let f = *d.get(p)?;
        let n = f.leading_zeros() as usize + 1;
        if n > 8 {
            return None;
        }
        let mut v = if strip { (f as u64) & (0xFF >> n) } else { f as u64 };
        for i in 1..n {
            v = (v << 8) | *d.get(p + i)? as u64;
        }
        Some((v, n))
    }
    fn walk(d: &[u8], mut p: usize, end: usize) -> Vec<(u64, usize, usize)> {
        let mut v = Vec::new();
        while p < end {
            let Some((id, il)) = vint(d, p, false) else { break };
            let Some((sz, sl)) = vint(d, p + il, true) else { break };
            let s = p + il + sl;
            let unknown = sz == (1u64 << (7 * sl)) - 1;
            let e = if unknown { end } else { (s as u64 + sz).min(end as u64) as usize };
            v.push((id, s, e));
            p = e;
        }
        v
    }
    let uint = |s: usize, e: usize| d.get(s..e).map(|b| b.iter().fold(0u64, |a, &x| (a << 8) | x as u64));
    let segment = walk(d, 0, d.len()).into_iter().find(|x| x.0 == 0x18538067)?;
    let tracks = walk(d, segment.1, segment.2).into_iter().find(|x| x.0 == 0x1654AE6B)?;
    for (id, s, e) in walk(d, tracks.1, tracks.2) {
        if id != 0xAE {
            continue;
        }
        let kids = walk(d, s, e);
        if kids.iter().find(|k| k.0 == 0x83).and_then(|k| uint(k.1, k.2)) != Some(2) {
            continue;
        }
        let cid = kids.iter().find(|k| k.0 == 0x86).map(|k| String::from_utf8_lossy(&d[k.1..k.2]).into_owned()).unwrap_or_default();
        let mut a = Audio { container: if d.windows(4).take(64).any(|w| w == b"webm") { "webm" } else { "matroska" }, ..Default::default() };
        if let Some(au) = kids.iter().find(|k| k.0 == 0xE1) {
            for (k, ks, ke) in walk(d, au.1, au.2) {
                match k {
                    0xB5 => {
                        let b = &d[ks..ke];
                        a.sample_rate = match b.len() {
                            4 => Some(f32::from_be_bytes(b.try_into().ok()?) as u32),
                            8 => Some(f64::from_be_bytes(b.try_into().ok()?) as u32),
                            _ => None,
                        }
                    }
                    0x9F => a.channels = uint(ks, ke).map(|c| c as u16),
                    0x6264 => a.bits = uint(ks, ke).map(|c| c as u16),
                    _ => {}
                }
            }
        }
        a.codec = match cid.as_str() {
            "A_OPUS" => "opus".into(),
            "A_VORBIS" => "vorbis".into(),
            "A_FLAC" => "flac".into(),
            "A_MPEG/L3" => "mp3".into(),
            "A_MPEG/L2" => "mp2".into(),
            "A_AC3" => "ac3".into(),
            "A_EAC3" => "eac3".into(),
            "A_ALAC" => "alac".into(),
            "A_DTS" => "dts".into(),
            c if c.starts_with("A_AAC") => "aac".into(),
            "A_PCM/INT/LIT" => format!("pcm_s{}le", a.bits.unwrap_or(16)),
            "A_PCM/INT/BIG" => format!("pcm_s{}be", a.bits.unwrap_or(16)),
            "A_PCM/FLOAT/IEEE" => format!("pcm_f{}le", a.bits.unwrap_or(32)),
            "A_MS/ACM" => kids.iter().find(|k| k.0 == 0x63A2).and_then(|k| Some(wav_codec(u16le(d, k.1)?, u16le(d, k.1 + 14).unwrap_or(0), false))).unwrap_or_else(|| "unknown_acm".into()),
            c => format!("unknown_mkv_{c}"),
        };
        return Some(a);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav16(rate: u32) -> Vec<u8> {
        let mut v = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0".to_vec();
        v.extend(rate.to_le_bytes());
        v.extend((rate * 2).to_le_bytes());
        v.extend(b"\x02\0\x10\0data\x04\0\0\0\x01\x02\x03\x04");
        v
    }

    #[test]
    fn wav_pcm() {
        let a = sniff(&wav16(16000)).unwrap();
        assert_eq!((a.container, a.codec.as_str(), a.sample_rate, a.channels, a.bits), ("wav", "pcm_s16le", Some(16000), Some(1), Some(16)));
        assert_eq!(a.payload, Some((44, 4)));
    }

    #[test]
    fn others() {
        assert_eq!(sniff(b"#!AMR\n\x3c").unwrap().codec, "amr_nb");
        // MPEG-1 layer III, 44.1 kHz, joint stereo.
        assert_eq!(sniff(&[0xFF, 0xFB, 0x90, 0x44, 0, 0]).unwrap().codec, "mp3");
        // ADTS AAC-LC 44.1 kHz stereo.
        assert_eq!(sniff(&[0xFF, 0xF1, 0x50, 0x80, 0x02, 0x1F, 0xFC]).unwrap().codec, "aac");
        let mut ogg = b"OggS\0\x02".to_vec();
        ogg.extend([0u8; 8]);
        ogg.extend(7u32.to_le_bytes());
        ogg.extend([0u8; 8]);
        ogg.push(1);
        ogg.push(19);
        ogg.extend(b"OpusHead\x01\x01\x38\x01\x80\xbb\0\0\0\0\0");
        assert_eq!(sniff(&ogg).unwrap().codec, "opus");
    }
}
