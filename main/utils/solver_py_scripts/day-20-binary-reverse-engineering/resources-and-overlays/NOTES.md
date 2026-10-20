# Day 20 binary RE - Resources and Overlays ("Resourceful")

d20 = x86-64 ELF (1.2 MB) with an **`RS20` resource overlay** appended at EOF (the `RS20` scheme the Day 19 question
foreshadowed). Layout, spelled out in the config questions:

```
+0 "RS20"  +4..+7 (ver 01, unused)  +8 resource_len u32le  +12 scratch_len u16le  +14..+15 pad
+16 sha256(scratch)[32]  +48 resource (resource_len)  scratch (scratch_len)  -> must end exactly at EOF
```

The resource is a **plain** HCFG record at resource offset 0 (no packing). variant-000: overlay at 1200888, resource
380 B, scratch 35 B, overlay size 463, HCFG key 0x51. The raw HCFG is also visible to a plain file scan (no decoy
this time), so the config answers were already right on the first pass.

Rust: `ctf-run/src/formats/overlay.rs` now tries both layouts on every `PK`/`RS` + two-digit magic (`packed_at` for
PK19, `resource_at` for RS20; SHA-256 decides, last validating magic wins). `resource_overlay.py` here is the Python
reference.

New d20 questions (12): `artifact_sha256_1`, `architecture`, `format`, `machine`, `config_{campaign,domain,mutex,url}`,
`config_crc32_hex`, `hcfg_version`, `overlay_offset` (offset of the final RS20 magic) and **`hcfg_resource_size`**
(the u32le resource length field, base 10). No `overlay_size`, `xor_key_hex`, `hcfg_offset` or `hcfg_storage` on d20.
Old days unchanged (d01-d19 at 12 each: 240 single/TT, 1200 mastery over 5 sets).

Graded: first pass refused (unknown `hcfg_resource_size`, no validated overlay); after the fix single CORRECT,
TT CORRECT (regular attempt), mastery CORRECT - all first submit. Route: dual-pool, TT ranked 64 / regular 32 (80 s),
mastery ranked 16 / regular 16 (325 s).
Fixtures: `ctf-run/fixtures/binary_re/d20-{single_solve,time_trial,mastery}.json` (replay 51/51 clean).
