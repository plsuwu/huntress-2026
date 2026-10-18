# Day 18 metadata - Frame of Reference

New kind: `media-primary-video-dimensions` (`d18-video-NNN-media-primary-video-dimensions`) on `video-001.mp4`
(ISO MP4 `isom`, H.264 High 320x180 10 fps 5 frames + AAC LC stereo 44.1 kHz, 0.5 s; `udta/meta` mdta `title` =
"Huntress MP4 evidence <12 hex>"). Single-solve: **`320x180` CORRECT** - graded as `WxH` like the jpeg/png kinds.
On this file every source agrees: `tkhd` 320x180 (identity matrix), `avc1` entry 320x180, SPS 20x12 macroblocks
(320x192 coded) with a 12-row bottom crop -> 320x180 (= ffprobe `width`/`height`).

Implementation: new `ctf-run/src/formats/video.rs` sniffs from the bytes: MP4/MOV (`moov/trak` with a `vide`
handler: `tkhd` size + matrix -> rotation, `mdhd`/`stsz` -> fps/frames, `stsd` entry size, `avcC` SPS, `pasp`, udta
tags), Matroska/WebM (TrackType 1: PixelWidth/Height, DisplayWidth/Height, PixelCrop, CodecPrivate avcC) and AVI
(`avih`, `strh` vids, `strf` BITMAPINFOHEADER). It includes an H.264 SPS parser (scaling lists, POC types, cropping
window, VUI SAR). `dimensions` = ffprobe's decoded size (SPS cropped size when parseable, else the sample entry);
container claims stay in separate fields. In `lanes/metadata.rs`, `media_answer` routes `-media-[primary-]video-<field>`
to `video_answer`; an `.eml` stands for its single video attachment. Only `dimensions` is graded; `width`, `height`,
`codec-name`, `coded-dimensions`, `display-dimensions` (tkhd/DisplayWidth/avih), `rotation` (degrees from the tkhd
matrix, 0..359), `sample-aspect-ratio`, `display-aspect-ratio`, `frame-rate`, `frame-count` and `container` are
unverified guesses. FACTS print all of them.

The prompt ("clips, previews, and screen recordings ... originals, crops, and the recording that could not have come
from the claimed device") points to later days with crops (SPS crop or Matroska PixelCrop vs the coded size),
rotated phone clips (tkhd matrix, display vs stream size) and a device claim in tags (make/model in `udta`) that the
geometry contradicts. Nothing like that on d18.

TT (23 questions) and mastery (115 questions, 5 sets) = the cumulative bundle + this kind. Every set used the same
`video-001.mp4` -> `320x180`. All CORRECT first try (untimed regular attempts). Route: dual-pool, ranked 32 /
regular 32; TT 50 s. Fixtures `ctf-run/fixtures/metadata/d18-*.json`.
