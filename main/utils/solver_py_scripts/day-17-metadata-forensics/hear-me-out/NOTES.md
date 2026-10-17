# Day 17 metadata - Hear Me Out

New kind: `media-primary-audio-codec-name` (`d17-audio-NNN-media-primary-audio-codec-name`) on `audio-001.wav`
(RIFF/WAVE, `fmt ` tag 1, 16-bit, mono, 16 kHz, `LIST/INFO` `INAM` = "Huntress audio evidence <12 hex>").
Single-solve: **`pcm_s16le` CORRECT** - graded with **ffprobe `codec_name` spelling**.

Implementation: `ctf-run/src/formats/audio.rs` sniffs the container from the bytes, not the extension (RIFF/RIFX/RF64
WAVE incl. WAVEFORMATEXTENSIBLE, AIFF/AIFC, FLAC, Ogg Vorbis/Opus/FLAC/Speex, ID3+MPEG (mp1/mp2/mp3), ADTS AAC, AMR,
Sun `.snd`, MP4 `stsd`, Matroska/WebM `CodecID`). It maps each to ffprobe names (`pcm_u8`, `pcm_s24le`, `pcm_f32le`, `pcm_mulaw`,
`adpcm_ms`, `mp3`, `aac`, `opus`, ...); "primary" means the first audio stream. In `lanes/metadata.rs`, `media_answer` routes
`-media-` keys before the email and suffix handlers. An `.eml` stands for its single audio attachment. An unrecognised
container is an error, not `none`. Only `codec-name` is graded; `sample-rate`, `channels`, `bits-per-sample` and
`container` are unverified guesses. FACTS now print audio container/codec/rate/channels/bits/duration, payload sha256
and text tags (RIFF INFO, ID3 T***, Vorbis comments).

The prompt ("transcoded, renamed, and embedded in mail ... connect equivalent recordings ... the stream that breaks
the sender's story") points to later days asking for a codec behind a lying extension, an audio attachment inside
an `.eml`, and equivalence across transcodes (shared INFO/ID3 token or payload hash) - nothing like that on d17.

TT (22 questions) and mastery (110 questions, 5 sets) = the cumulative bundle + this kind; every set was the same
`audio-001.wav` -> `pcm_s16le`. All CORRECT first try (untimed regular attempts). Route: dual-pool, ranked 32 /
regular 32; mastery 170 s. Fixtures `ctf-run/fixtures/metadata/d17-*.json` (single-solve fixture written by hand from
the first-pass run: its ctf-run resubmit said `already_solved` because the curl format check had already solved it).
