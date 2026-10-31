# Day 31 binary RE - Reversing Gauntlet ("Final Boss")

d31 = **one x86-64 ELF** (`d31--sample.elf`, ~500 KB). It has no HCFG. Instead it carries an **`HGAU`** record near the
end of the file. The record uses the HCFG layout (`magic | ver=01 | xor key | len LE16 | crc32 LE32 | payload^key`)
and its JSON is a recap of the whole event:
`{schema, day, variant, languages[31], capabilities[10], prior_anchors[{day,sha256}], config:{campaign, domain, ipv4,
url, mutex, windows_path, linux_path, service, host_model}, trace:{branch, event_count, effects, network}}`.

New d31 questions (14, so 434 = 420 + 14). Old days are unchanged:
- `artifact_sha256_1`, `analysis_format` (enum `elf`), `analysis_machine` (`x86-64`)
- `analysis_config_{campaign,domain,mutex,url}`: "decoded HGAU JSON property config.X" (no member named)
- `analysis_gauntlet_config_*`: the path is `gauntlet.config.X`. `gauntlet` is the root's name, not a JSON key
- `analysis_gauntlet_crc32_hex` (header bytes 8..11, `0x%08x`) and `analysis_xor_key_hex` (header byte 5, `0x%02x`).
  Both come from the HGAU header, not from HCFG
- `analysis_gauntlet_trace_branch`: "the exact gauntlet.trace_branch token" means JSON `trace.branch`, so the `_`
  joins nested keys. It is an enum (success/decoy/failure) and was `success` in all 15 mastery sets

What broke on the first pass: the gauntlet fields, plus `xor_key_hex` (no HCFG). The untagged `config_*` answered by
luck through the `xor_json` fallback.

Code:
- `formats/hcfg.rs`: `last_hg_records(d)` keeps the last CRC-valid record for every `HG??` magic, scanning backward
  (memmem on `HG`, cheap even on the 67 MB d08 ELF).
- `lanes/binary_re.rs`: `Spec::graph` became `Spec::records` (magic -> record). `record_answer` replaces the d30
  HGRF-only reader. It finds the `HG??` magic in the question text, then picks the member: the one named ("at X
  decoded ..." or `named_member`), else the unique member holding that magic. It answers either a JSON path
  ("JSON property P" or "exact P token") or a header field (`*crc32_hex`, `*xor_key_hex`). A leading path segment is
  dropped when the JSON has no such key (`graph`, `gauntlet`). Each segment is looked up `_`-aware
  (`trace_branch` -> `trace.branch`). The d30 "graphs match when this_role is omitted" check is kept, now per magic.

All three modes were CORRECT untimed on the first submit after the fix: single (434), TT (434), mastery (**15 sets**,
6510). No ranked attempts were spent. Fixtures: `ctf-run/fixtures/binary_re/d31-{single_solve,time_trial,mastery}.json`
(replay 84/84 clean).

Day 32 prediction (if there is one): the "gauntlet" schema suggests a finale. If the branch varies (`decoy` or
`failure`), it is read verbatim and needs nothing new. A new `HG??` magic or JSON path needs nothing either. A
question on a list element (`languages`, `prior_anchors[i].sha256`) would need index support in `lookup`.
