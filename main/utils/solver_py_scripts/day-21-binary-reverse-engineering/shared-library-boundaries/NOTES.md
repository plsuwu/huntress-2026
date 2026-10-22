# Day 21 binary RE - Shared Library Boundaries ("Shared Custody")

d21 = **two members**: an OCaml native loader (`d21--sample-loader.elf`, 780 KB x86-64 PIE, not stripped, links
`camlDynlink` / `caml_natdynlink_open`) and a natdynlink plugin (`d21--sample-plugin.cmxs`, 9 KB x86-64 ELF shared
object). The HCFG record lives **only in the plugin**, raw in `.data` (variant-000: file offset 4364); the loader has
no HCFG at all. No overlay, no packing, nothing OCaml-specific needed.

New d21 questions (12): `artifact_sha256_{1,2}` (CD order: loader, then plugin), `loader_machine` / `plugin_machine`
(integer e_machine, 62), `plugin_config_{campaign,domain,mutex,url}` (tag-routed by the existing `-<tag>` stem logic)
and **untagged `config_{campaign,domain,mutex,url}`** whose text says "Use the d21-- plugin.cmxs member; the loader has
no scored HCFG config".

What broke: the untagged `config_*` were answered from the day's primary specimen (first non-json member = the
loader) -> "config has no campaign" -> first pass refused. Fix (`binary_re.rs::named_member` / `member_matches`): an
untagged `analysis_*` field whose question names a member ("Use the [unique] dNN-- member whose basename ends X" or
"Use the dNN-- X member") is answered from that member when exactly one specimen's basename or stem ends with X;
otherwise the primary as before. Tag routing still runs first.

Graded: single CORRECT (after the fix; first pass refused, no submit spent), TT CORRECT (regular attempt, first
submit), mastery CORRECT (5 sets, 1260 answers, first submit). Old days unchanged (d01-d21 at 12 each = 252).
Route: single run_for_time false; TT dual-pool ranked 64 / regular 32 (80 s); mastery ranked 16 / regular 16 (325 s).
Fixtures: `ctf-run/fixtures/binary_re/d21-{single_solve,time_trial,mastery}.json` (replay 54/54 clean).

Prediction: multi-member days now come with member-selector phrasing in the question text; if a future day's
selector uses other wording (e.g. "starts", "the member named"), extend `named_member`.
