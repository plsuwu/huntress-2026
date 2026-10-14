# Day 13 metadata-forensics - Product Placement
Lane questions are MSI (OLE/CFB) metadata. `msi_meta.py` is a stdlib CFB + MSI string-pool/Property reader
(7z only shows the cab; `file` shows the SummaryInformation).

## single_solve (SOLVED)
- 1 question: `d13-msi-001-msi-product-code-guid` for `installer-001.msi` (one MSI in `metadata-day-13-evidence.zip`).
- Answer = Property-table `ProductCode` **uppercase with braces**: `{DD1A80D5-0250-4F4D-B289-13096D633C61}`.
  Despite "canonical 8-4-4-4-12" in answer_format, bare-uppercase and bare-lowercase were `incorrect`; braces+upper CORRECT.
- Other GUIDs in the file: UpgradeCode `{5F63B105-...}`, SummaryInfo Revision (package code) `{99BE1D60-...}` - not the answer.
- Pool sizes: single 1; TT 32 ranked (deadline 50 s, 1 set); mastery 32 ranked (80 s, 2 sets; 5 regular sets).
- Prompt hints at TT/mastery: "revisions vs different products, find the collision" -> expect upgrade-code / version /
  product-code collision questions across several installers; solve.py dumps FACTS for unknown kinds.

## time_trial (SOLVED via regular untimed start, no run_for_time; flag 756fa79cb2238117a9b0ca8dd0c5dfbf)
- 18-question cumulative bundle (days 1-13 kinds) over `evidence.zip`: roster/evidence-note hashes+size+filename, jpeg SOF,
  png IHDR, pdf header, zip EOCD/CD names, PE machine/entry RVA/OriginalFilename, .NET MVID, MSI ProductCode, synthesis sha256 order.
- Format gotchas learned via the partial-answer oracle (wrong full submit = bare `incorrect`; single-field submits give
  `accepted`+`remaining`):
  - `pe.address_of_entry_point_rva_hex` -> `0x%08x` zero-padded 8 digits (`0x00065bd0`), NOT `0x65bd0`.
  - `pe.version_original_filename` -> VS_VERSION_INFO OriginalFilename (`pe-base-386.exe`), not the member name
    (careful: id suffix `-filename` also matches the roster-filename handler; fixed in solve.py).
  - `dotnet.mvid_guid` -> lowercase bare 8-4-4-4-12 (Module table Mvid via #GUID heap).
  - `msi.product_code_guid` -> braces + uppercase as stored.
  - synthesis order = files sorted ascending by full-file SHA-256, compact JSON array.
- `python3 -I solve.py time_trial --submit` (token from .env). Timed (ranked, 50 s) attempts NOT used; 32 remain.

## mastery (SOLVED via regular untimed start, 5 sets; flag 8ded306ca33bca0c6d7c19dae91d73a9)
- Same 18-question bundle x5 sets (`setN.` prefix, `evidence-0N.zip`); only evidence-note text and the question subset
  per set vary (e.g. set1 asks `evidence-note-copy-filename`, set3 `roster-filename`). Media files identical every set.
- Gotcha: `<stem>-filename` must match member stem exactly (`evidence-note-copy.txt` vs `evidence-note.txt`).
- Timed ranked attempts (TT 50 s, mastery 80 s) not used; 32 remain each. Run:
  `set -a; . ./.env; set +a; python3 -I solve.py {single_solve|time_trial|mastery} [--timed] [--retry] [--submit]`.
- Dir layout: `solve.py`, `msi_meta.py` (stdlib CFB/MSI reader), `artifacts/` (start JSONs + latest zips).
- The "revisions vs products / collision" hint from the prompt never surfaced as a question in regular variants;
  ranked variants might differ - solve.py prints UNKNOWN-KIND and blocks submit rather than guessing.
