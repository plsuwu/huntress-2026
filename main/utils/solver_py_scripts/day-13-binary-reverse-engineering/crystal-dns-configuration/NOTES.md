# Crystal Clear (day 13, binary-reverse-engineering-day-13-crystal-dns-configuration)

- single_solve CORRECT (2026-10-14), flag `5d3eb99686da3828f62a5289ac351fd8`.
- Evidence = same d01..d10 bank as day 10 (byte-identical files) + d11, d12, d13 ELFs. 130 answers.
- `solve.py <evidence_dir>` = day-10 solver + one loop over d11/d12/d13.
  - d11 = D-language ELF, d13 = Crystal ELF (dynamic, not stripped): base64 `SENGR...` string constant -> `elf_b64_hcfg`;
    `hcfg_offset` = file offset of the base64 text start (`d.find(b'SENGR')`).
  - d12 = static stripped ELF with a plain XOR'd HCFG blob; `hcfg_offset` = offset of the validated `HCFG` magic.
- **Gotcha: `format` casing differs**: d11 `ELF` accepted, but d12/d13 only `elf` (lowercase); `ELF64`, `elf64-x86-64`,
  `ELF 64-bit LSB executable` all rejected. Found by submitting single fields in single_solve (partial submits return
  `accepted` + `accepted_ids` there too, so single_solve is a per-field oracle; incorrect subsets cost nothing).
- config values are the usual `BRnn-000` / `node-000.dayNN.example.invalid` / `Global\HuntressBRnn_000` / `https://.../api/BRnn-000`.

## Results (2026-10-14)
- single_solve CORRECT, flag `5d3eb99686da3828f62a5289ac351fd8` (first submit wrong only because of `format` casing, see above).
- time_trial CORRECT first timed attempt (`sh live_run.sh start time_trial`, 11.9 s wall of 80 s deadline, HoF eligible),
  flag `d58910380258665ba3ff4ce99e480d76`, 63 attempts left.
- mastery CORRECT first timed attempt (`sh mastery_run.sh start timed`, 28.5 s wall of 200 s deadline, HoF eligible;
  390 answers = 3 sets x 130), flag `3835c3d2e1ad9a2b16e07354955c1bdb`, 15 attempts left.

## Gotchas
- This route has `run_for_time_supported=true` (unlike day 10): plain `start` is an untimed regular attempt; send
  `run_for_time:true` (live_run.sh does; mastery_run.sh needs the `timed` arg) for the Hall-of-Fame attempt.
- `source ../../.env` with `set -a` before running the scripts, otherwise the GM returns `{"error":"unauthorized"}`
  (no attempt burned; mastery_run.sh must then be rerun - clean `artifacts/mastery/mastery_*` first).
- Mastery sets had no surprises: same d01-d13 bank per set, `elf` casing for d12/d13 held in all three.
