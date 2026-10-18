//! Video streams sniffed from their bytes (never the extension): MP4/MOV (`moov/trak` with a `vide`
//! handler), Matroska/WebM (TrackType 1) and AVI (`strh` `vids` + `strf` BITMAPINFOHEADER). Geometry
//! follows ffprobe: `width`x`height` is the decoded picture (H.264 SPS with its cropping window when
//! present, else the sample entry), with the container's claims (MP4 `tkhd` presentation size and
//! matrix, Matroska display size / pixel crop, AVI `avih`) kept apart. The primary stream is the
//! first video stream. Codec names follow ffprobe's `codec_name`.

use super::bytes::{u16be, u16le, u32be, u32le, u64be};

#[derive(Debug, Clone, Default)]
pub struct Video {
    /// ffprobe-style container name (`mp4`, `mov`, `matroska`, `webm`, `avi`).
    pub container: &'static str,
    pub codec: String,
    /// Decoded picture size (what ffprobe reports as `width`/`height`).
    pub width: u32,
    pub height: u32,
    /// Size stored in the sample entry / BITMAPINFOHEADER / PixelWidth (before SPS cropping).
    pub entry: Option<(u32, u32)>,
    /// SPS size before and after the cropping window.
    pub sps_coded: Option<(u32, u32)>,
    pub sps_crop: Option<(u32, u32, u32, u32)>,
    /// Container display claims: MP4 `tkhd` width/height, Matroska DisplayWidth/Height, AVI `avih`.
    pub display: Option<(u32, u32)>,
    /// Matroska PixelCrop (top, bottom, left, right).
    pub pixel_crop: Option<(u32, u32, u32, u32)>,
    /// MP4 `tkhd` matrix (a, b, c, d, tx, ty in 16.16; u, v, w in 2.30) and its rotation in degrees.
    pub matrix: Option<[i32; 9]>,
    pub rotation: Option<i32>,
    /// Sample aspect ratio (`pasp`, SPS VUI).
    pub sar: Option<(u32, u32)>,
    /// Frames per second as a ratio (sample count / media duration, or AVI rate/scale).
    pub frame_rate: Option<(u64, u64)>,
    pub duration_seconds: Option<f64>,
    pub frames: Option<u64>,
    /// Text tags (MP4 `udta` `ilst`/`mdta` keys, handler names, compressor name; Matroska Name).
    pub tags: Vec<(String, String)>,
}

pub fn sniff(d: &[u8]) -> Option<Video> {
    if matches!(d.get(4..8), Some(b"ftyp") | Some(b"moov") | Some(b"mdat") | Some(b"free") | Some(b"wide") | Some(b"skip")) {
        return mp4(d);
    }
    if d.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return matroska(d);
    }
    if d.starts_with(b"RIFF") && d.get(8..12) == Some(b"AVI ") {
        return avi(d);
    }
    None
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

impl Video {
    pub fn dimensions(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }

    /// Display aspect ratio, reduced (`16:9`), from the picture size and SAR.
    pub fn dar(&self) -> Option<(u64, u64)> {
        let (sw, sh) = self.sar.unwrap_or((1, 1));
        let (w, h) = (self.width as u64 * sw as u64, self.height as u64 * sh as u64);
        let g = gcd(w, h);
        (g > 0).then(|| (w / g, h / g))
    }
}

// ---------------------------------------------------------------- H.264 SPS

struct Bits<'a> {
    b: &'a [u8],
    p: usize,
}

impl Bits<'_> {
    fn bit(&mut self) -> Option<u32> {
        let v = (*self.b.get(self.p / 8)? >> (7 - self.p % 8)) & 1;
        self.p += 1;
        Some(v as u32)
    }
    fn bits(&mut self, n: u32) -> Option<u32> {
        (0..n).try_fold(0u32, |a, _| Some((a << 1) | self.bit()?))
    }
    fn ue(&mut self) -> Option<u32> {
        let mut z = 0;
        while self.bit()? == 0 {
            z += 1;
            if z > 31 {
                return None;
            }
        }
        Some((1u32 << z) - 1 + self.bits(z)?)
    }
    fn se(&mut self) -> Option<i32> {
        let k = self.ue()?;
        Some(if k & 1 == 1 { k.div_ceil(2) as i32 } else { -((k / 2) as i32) })
    }
}

/// Parsed H.264 SPS: (coded w, h), (crop left, right, top, bottom) in pixels, cropped (w, h), SAR.
pub struct Sps {
    pub coded: (u32, u32),
    pub crop: (u32, u32, u32, u32),
    pub size: (u32, u32),
    pub sar: Option<(u32, u32)>,
}

/// An SPS NAL unit (with its 1-byte header) -> geometry (ITU-T H.264 7.3.2.1.1, E.1.1).
pub fn h264_sps(nal: &[u8]) -> Option<Sps> {
    let mut rbsp = Vec::with_capacity(nal.len());
    let mut zeros = 0;
    for &x in nal.get(1..)? {
        if zeros >= 2 && x == 3 {
            zeros = 0;
            continue;
        }
        zeros = if x == 0 { zeros + 1 } else { 0 };
        rbsp.push(x);
    }
    let mut r = Bits { b: &rbsp, p: 0 };
    let profile = r.bits(8)?;
    r.bits(16)?; // constraint flags + level
    r.ue()?;
    let mut chroma = 1;
    if matches!(profile, 100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135) {
        chroma = r.ue()?;
        if chroma == 3 {
            r.bit()?;
        }
        r.ue()?;
        r.ue()?;
        r.bit()?;
        if r.bit()? == 1 {
            for i in 0..if chroma == 3 { 12 } else { 8 } {
                if r.bit()? == 1 {
                    let n = if i < 6 { 16 } else { 64 };
                    let (mut last, mut next) = (8i32, 8i32);
                    for _ in 0..n {
                        if next != 0 {
                            next = (last + r.se()? + 256) % 256;
                        }
                        last = if next == 0 { last } else { next };
                    }
                }
            }
        }
    }
    r.ue()?;
    match r.ue()? {
        0 => {
            r.ue()?;
        }
        1 => {
            r.bit()?;
            r.se()?;
            r.se()?;
            for _ in 0..r.ue()? {
                r.se()?;
            }
        }
        _ => {}
    }
    r.ue()?;
    r.bit()?;
    let wmb = r.ue()? + 1;
    let hmu = r.ue()? + 1;
    let frame_mbs_only = r.bit()?;
    if frame_mbs_only == 0 {
        r.bit()?;
    }
    r.bit()?;
    let (sub_w, sub_h) = match chroma {
        1 => (2, 2),
        2 => (2, 1),
        _ => (1, 1),
    };
    let (cux, cuy) = if chroma == 0 { (1, 2 - frame_mbs_only) } else { (sub_w, sub_h * (2 - frame_mbs_only)) };
    let mut crop = (0, 0, 0, 0);
    if r.bit()? == 1 {
        crop = (r.ue()? * cux, r.ue()? * cux, r.ue()? * cuy, r.ue()? * cuy);
    }
    let coded = (wmb * 16, (2 - frame_mbs_only) * hmu * 16);
    let size = (coded.0.checked_sub(crop.0 + crop.1)?, coded.1.checked_sub(crop.2 + crop.3)?);
    let mut sar = None;
    if r.bit() == Some(1) && r.bit() == Some(1) {
        const T: [(u32, u32); 17] = [(0, 0), (1, 1), (12, 11), (10, 11), (16, 11), (40, 33), (24, 11), (20, 11), (32, 11), (80, 33), (18, 11), (15, 11), (64, 33), (160, 99), (4, 3), (3, 2), (2, 1)];
        sar = match r.bits(8) {
            Some(255) => Some((r.bits(16)?, r.bits(16)?)),
            Some(i) => T.get(i as usize).copied().filter(|s| s.0 > 0),
            None => None,
        };
    }
    Some(Sps { coded, crop, size, sar })
}

/// The first SPS of an `avcC` record.
fn avcc_sps(c: &[u8]) -> Option<Sps> {
    if *c.first()? != 1 || c.get(5)? & 0x1F == 0 {
        return None;
    }
    let n = u16be(c, 6)? as usize;
    h264_sps(c.get(8..8 + n)?)
}

fn fourcc_codec(f: &[u8]) -> String {
    match f {
        b"avc1" | b"avc3" | b"H264" | b"h264" | b"X264" | b"x264" | b"AVC1" => "h264".into(),
        b"hvc1" | b"hev1" | b"HEVC" => "hevc".into(),
        b"vp08" | b"VP80" => "vp8".into(),
        b"vp09" | b"VP90" => "vp9".into(),
        b"av01" | b"AV01" => "av1".into(),
        b"mp4v" | b"FMP4" | b"DIVX" | b"DX50" | b"XVID" | b"xvid" => "mpeg4".into(),
        b"jpeg" | b"MJPG" | b"mjpa" | b"mjpb" => "mjpeg".into(),
        b"s263" | b"H263" => "h263".into(),
        b"apch" | b"apcn" | b"apcs" | b"apco" | b"ap4h" | b"ap4x" => "prores".into(),
        b"png " | b"MPNG" => "png".into(),
        b"rle " => "qtrle".into(),
        b"raw " | b"\0\0\0\0" => "rawvideo".into(),
        b"mp2v" | b"MPG2" => "mpeg2video".into(),
        b"WMV3" => "wmv3".into(),
        b"WVC1" => "vc1".into(),
        b"cvid" => "cinepak".into(),
        b"MSVC" | b"CRAM" | b"WHAM" => "msvideo1".into(),
        b"FFV1" => "ffv1".into(),
        b"tscc" => "tscc".into(),
        other => format!("unknown_{}", String::from_utf8_lossy(other).trim_end()),
    }
}

// ---------------------------------------------------------------- MP4 / MOV

fn boxes(d: &[u8], mut p: usize, end: usize) -> Vec<(&[u8], usize, usize)> {
    let mut v = Vec::new();
    while p + 8 <= end {
        let Some(mut size) = u32be(d, p).map(|s| s as usize) else { break };
        let mut hdr = 8;
        if size == 1 {
            size = u64be(d, p + 8).unwrap_or(0) as usize;
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

fn child(d: &[u8], s: usize, e: usize, k: &[u8]) -> Option<(usize, usize)> {
    boxes(d, s, e).into_iter().find(|b| b.0 == k).map(|b| (b.1, b.2))
}

/// `udta/meta` `keys`+`ilst` (mdta) and plain `ilst` atoms (`©nam`, ...) -> (key, text).
fn mp4_tags(d: &[u8], s: usize, e: usize, out: &mut Vec<(String, String)>) {
    for (k, bs, be) in boxes(d, s, e) {
        match k {
            b"udta" => mp4_tags(d, bs, be, out),
            b"meta" => {
                // ISO `meta` is a full box (4-byte version/flags); QuickTime's is not.
                let start = if d.get(bs + 4..bs + 8) == Some(b"hdlr") { bs } else { bs + 4 };
                let kids = boxes(d, start, be);
                let mut keys = Vec::new();
                if let Some(&(_, ks, _)) = kids.iter().find(|b| b.0 == b"keys") {
                    let n = u32be(d, ks + 4).unwrap_or(0);
                    let mut p = ks + 8;
                    for _ in 0..n {
                        let Some(sz) = u32be(d, p).map(|x| x as usize) else { break };
                        if sz < 8 {
                            break;
                        }
                        keys.push(String::from_utf8_lossy(d.get(p + 8..p + sz).unwrap_or(&[])).into_owned());
                        p += sz;
                    }
                }
                if let Some(&(_, is, ie)) = kids.iter().find(|b| b.0 == b"ilst") {
                    for (ik, es, ee) in boxes(d, is, ie) {
                        let name = match u32be(ik, 0) {
                            Some(i) if i >= 1 && (i as usize) <= keys.len() => keys[i as usize - 1].clone(),
                            _ => String::from_utf8_lossy(ik).into_owned(),
                        };
                        if let Some((ds, de)) = child(d, es, ee, b"data") {
                            out.push((name, String::from_utf8_lossy(d.get(ds + 8..de).unwrap_or(&[])).into_owned()));
                        }
                    }
                }
            }
            k if k.first() == Some(&0xA9) && be >= bs + 4 => {
                // QuickTime `udta` text atom: u16 length, u16 language, text.
                let n = u16be(d, bs).unwrap_or(0) as usize;
                if let Some(t) = d.get(bs + 4..bs + 4 + n) {
                    out.push((String::from_utf8_lossy(k).into_owned(), String::from_utf8_lossy(t).into_owned()));
                }
            }
            _ => {}
        }
    }
}

fn mp4(d: &[u8]) -> Option<Video> {
    let top = boxes(d, 0, d.len());
    let (ms, me) = top.iter().find(|b| b.0 == b"moov").map(|b| (b.1, b.2))?;
    let qt = top.iter().find(|b| b.0 == b"ftyp").is_some_and(|b| d.get(b.1..b.1 + 4) == Some(b"qt  "));
    let mut tags = Vec::new();
    mp4_tags(d, ms, me, &mut tags);
    for (k, ts, te) in boxes(d, ms, me) {
        if k != b"trak" {
            continue;
        }
        let Some((ds, de)) = child(d, ts, te, b"mdia") else { continue };
        let Some((hs, he)) = child(d, ds, de, b"hdlr") else { continue };
        if d.get(hs + 8..hs + 12) != Some(b"vide") {
            continue;
        }
        let mut v = Video { container: if qt { "mov" } else { "mp4" }, tags: tags.clone(), ..Default::default() };
        let hname = String::from_utf8_lossy(d.get(hs + 24..he).unwrap_or(&[])).trim_matches(|c: char| c == '\0' || c.is_control()).to_string();
        if !hname.is_empty() {
            v.tags.push(("handler_name".into(), hname));
        }
        if let Some((th, _)) = child(d, ts, te, b"tkhd") {
            let o = th + 4 + if d[th] == 1 { 32 } else { 20 } + 16;
            let m: Option<Vec<i32>> = (0..9).map(|i| u32be(d, o + 4 * i).map(|x| x as i32)).collect();
            if let Some(m) = m {
                let m: [i32; 9] = m.try_into().ok()?;
                let deg = (m[1] as f64).atan2(m[0] as f64).to_degrees().round() as i32;
                v.rotation = Some(deg.rem_euclid(360));
                v.matrix = Some(m);
            }
            if let (Some(w), Some(h)) = (u32be(d, o + 36), u32be(d, o + 40)) {
                v.display = Some((w >> 16, h >> 16));
            }
        }
        if let Some((mh, _)) = child(d, ds, de, b"mdhd") {
            let (scale, dur) = if d[mh] == 1 { (u32be(d, mh + 20), u64be(d, mh + 24)) } else { (u32be(d, mh + 12), u32be(d, mh + 16).map(u64::from)) };
            if let (Some(sc), Some(du)) = (scale, dur) {
                if sc > 0 {
                    v.duration_seconds = Some(du as f64 / sc as f64);
                }
                v.frame_rate = Some((sc as u64, du));
            }
        }
        let (ns, ne) = child(d, ds, de, b"minf")?;
        let (ss, se) = child(d, ns, ne, b"stbl")?;
        if let Some((zs, _)) = child(d, ss, se, b"stsz") {
            v.frames = u32be(d, zs + 8).map(u64::from);
        }
        if let (Some(n), Some((sc, du))) = (v.frames, v.frame_rate) {
            let (a, b) = (n * sc, du);
            let g = gcd(a, b);
            v.frame_rate = (g > 0).then(|| (a / g, b / g));
        } else {
            v.frame_rate = None;
        }
        let (st, ste) = child(d, ss, se, b"stsd")?;
        let (fourcc, es, ee) = boxes(d, st + 8, ste).into_iter().next()?;
        v.codec = fourcc_codec(fourcc);
        let (w, h) = (u16be(d, es + 24)? as u32, u16be(d, es + 26)? as u32);
        v.entry = Some((w, h));
        (v.width, v.height) = (w, h);
        let comp = d.get(es + 42..es + 74).map(|c| c[1..1 + (c[0] as usize).min(31)].to_vec()).unwrap_or_default();
        if !comp.is_empty() {
            v.tags.push(("compressor_name".into(), String::from_utf8_lossy(&comp).into_owned()));
        }
        for (ck, cs, ce) in boxes(d, es + 78, ee) {
            match ck {
                b"avcC" => {
                    if let Some(s) = avcc_sps(&d[cs..ce]) {
                        v.sps_coded = Some(s.coded);
                        v.sps_crop = Some(s.crop);
                        (v.width, v.height) = s.size;
                        v.sar = v.sar.or(s.sar);
                    }
                }
                b"pasp" => {
                    if let (Some(a), Some(b)) = (u32be(d, cs), u32be(d, cs + 4)) {
                        if a > 0 && b > 0 {
                            v.sar = Some((a, b));
                        }
                    }
                }
                _ => {}
            }
        }
        return Some(v);
    }
    None
}

// ---------------------------------------------------------------- Matroska / WebM

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

fn ebml(d: &[u8], mut p: usize, end: usize) -> Vec<(u64, usize, usize)> {
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

fn matroska(d: &[u8]) -> Option<Video> {
    let uint = |s: usize, e: usize| d.get(s..e).map(|b| b.iter().fold(0u64, |a, &x| (a << 8) | x as u64));
    let segment = ebml(d, 0, d.len()).into_iter().find(|x| x.0 == 0x18538067)?;
    let tracks = ebml(d, segment.1, segment.2).into_iter().find(|x| x.0 == 0x1654AE6B)?;
    for (id, s, e) in ebml(d, tracks.1, tracks.2) {
        if id != 0xAE {
            continue;
        }
        let kids = ebml(d, s, e);
        let get = |k: u64| kids.iter().find(|x| x.0 == k);
        if get(0x83).and_then(|k| uint(k.1, k.2)) != Some(1) {
            continue;
        }
        let cid = get(0x86).map(|k| String::from_utf8_lossy(&d[k.1..k.2]).into_owned()).unwrap_or_default();
        let mut v = Video { container: if d.windows(4).take(64).any(|w| w == b"webm") { "webm" } else { "matroska" }, ..Default::default() };
        if let Some(n) = get(0x536E) {
            v.tags.push(("name".into(), String::from_utf8_lossy(&d[n.1..n.2]).into_owned()));
        }
        if let Some(du) = get(0x23E383).and_then(|k| uint(k.1, k.2)) {
            if du > 0 {
                let g = gcd(1_000_000_000, du);
                v.frame_rate = Some((1_000_000_000 / g, du / g));
            }
        }
        v.codec = match cid.as_str() {
            "V_MPEG4/ISO/AVC" => "h264".into(),
            "V_MPEGH/ISO/HEVC" => "hevc".into(),
            "V_VP8" => "vp8".into(),
            "V_VP9" => "vp9".into(),
            "V_AV1" => "av1".into(),
            "V_MJPEG" => "mjpeg".into(),
            "V_THEORA" => "theora".into(),
            "V_MPEG2" => "mpeg2video".into(),
            "V_MPEG4/ISO/ASP" | "V_MPEG4/ISO/SP" => "mpeg4".into(),
            "V_UNCOMPRESSED" => "rawvideo".into(),
            "V_FFV1" => "ffv1".into(),
            "V_PRORES" => "prores".into(),
            "V_MS/VFW/FOURCC" => get(0x63A2).and_then(|k| d.get(k.1 + 16..k.1 + 20)).map(fourcc_codec).unwrap_or_else(|| "unknown_vfw".into()),
            other => format!("unknown_{other}"),
        };
        let mut pw = None;
        let mut ph = None;
        let mut dw = None;
        let mut dh = None;
        let mut crop = (0, 0, 0, 0);
        if let Some(vi) = get(0xE0) {
            for (k, ks, ke) in ebml(d, vi.1, vi.2) {
                let x = uint(ks, ke).map(|x| x as u32);
                match k {
                    0xB0 => pw = x,
                    0xBA => ph = x,
                    0x54B0 => dw = x,
                    0x54BA => dh = x,
                    0x54BB => crop.0 = x.unwrap_or(0),
                    0x54AA => crop.1 = x.unwrap_or(0),
                    0x54CC => crop.2 = x.unwrap_or(0),
                    0x54DD => crop.3 = x.unwrap_or(0),
                    _ => {}
                }
            }
        }
        (v.width, v.height) = (pw?, ph?);
        v.entry = Some((v.width, v.height));
        if crop != (0, 0, 0, 0) {
            v.pixel_crop = Some(crop);
        }
        if let (Some(w), Some(h)) = (dw, dh) {
            v.display = Some((w, h));
        }
        if v.codec == "h264" {
            if let Some(sp) = get(0x63A2).and_then(|k| avcc_sps(&d[k.1..k.2])) {
                v.sps_coded = Some(sp.coded);
                v.sps_crop = Some(sp.crop);
                (v.width, v.height) = sp.size;
                v.sar = sp.sar;
            }
        }
        return Some(v);
    }
    None
}

// ---------------------------------------------------------------- AVI

fn avi(d: &[u8]) -> Option<Video> {
    fn chunks(d: &[u8], mut p: usize, end: usize) -> Vec<(&[u8], usize, usize)> {
        let mut v = Vec::new();
        while p + 8 <= end {
            let Some(n) = u32le(d, p).map(|n| n as usize) else { break };
            let e = (p + 8 + n).min(end);
            v.push((&d[p..p + 4], p + 8, e));
            p = p + 8 + n + (n & 1);
        }
        v
    }
    let mut v = Video { container: "avi", ..Default::default() };
    let hdrl = chunks(d, 12, d.len()).into_iter().find(|c| c.0 == b"LIST" && d.get(c.1..c.1 + 4) == Some(b"hdrl"))?;
    for (k, s, e) in chunks(d, hdrl.1 + 4, hdrl.2) {
        if k == b"avih" {
            v.display = Some((u32le(d, s + 32)?, u32le(d, s + 36)?));
            v.frames = u32le(d, s + 16).map(u64::from);
        }
        if k != b"LIST" || d.get(s..s + 4) != Some(b"strl") {
            continue;
        }
        let kids = chunks(d, s + 4, e);
        let Some(&(_, hs, _)) = kids.iter().find(|c| c.0 == b"strh") else { continue };
        if d.get(hs..hs + 4) != Some(b"vids") {
            continue;
        }
        let (scale, rate) = (u32le(d, hs + 20)? as u64, u32le(d, hs + 24)? as u64);
        if scale > 0 {
            let g = gcd(rate, scale);
            v.frame_rate = Some((rate / g, scale / g));
        }
        let &(_, fs, _) = kids.iter().find(|c| c.0 == b"strf")?;
        let w = u32le(d, fs + 4)?;
        let h = (u32le(d, fs + 8)? as i32).unsigned_abs();
        let comp = d.get(fs + 16..fs + 20)?;
        v.codec = fourcc_codec(if comp == [0, 0, 0, 0] { b"raw " } else { comp });
        if v.codec.starts_with("unknown_") {
            v.codec = fourcc_codec(d.get(hs + 4..hs + 8)?);
        }
        let _ = u16le(d, fs + 14);
        v.entry = Some((w, h));
        (v.width, v.height) = (w, h);
        return Some(v);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sps_320x180() {
        // d18 single-solve avcC SPS: 20x12 macroblocks, 12 rows cropped at the bottom.
        let sps = [0x67, 0x64, 0x00, 0x15, 0xAC, 0x72, 0x04, 0x41, 0x41, 0x9F, 0x9F, 0x01, 0x10, 0x00, 0x00, 0x03, 0x00, 0x10, 0x00, 0x00, 0x03, 0x01, 0x40, 0xF1, 0x62, 0xD8, 0x46];
        let s = h264_sps(&sps).unwrap();
        assert_eq!(s.size, (320, 180));
        assert_eq!(s.coded, (320, 192));
        assert_eq!(s.crop, (0, 0, 0, 12));
    }
}
