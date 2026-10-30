# Day 30 binary RE - Multi-Binary Campaign ("Campaign Trail")

d30 = **three NekoVM bytecode modules** (`NEKO` magic, Haxe `.n`), CD order: `d30--sample-loader.n`,
`d30--sample-library.n`, `d30--sample-config.n` (~21 KB each). No HCFG: each carries an **`HGRF`** record at the
end of the file, same 12-byte layout as HCFG (`magic | ver=01 | xor key | len LE16 | crc32 LE32 | payload^key`),
whose JSON is a campaign graph:
`{schema, campaign_id, nodes, edges, load_order, config:{campaign, domain, ipv4, url, mutex, ...}, this_role, ...}`.
The three graphs are equal except `this_role` (`loader` / `library` / `config`).

New d30 questions (14): `artifact_sha256_{1,2,3}`, `analysis_{loader,library,config_capsule}_format` (enum
`neko_bytecode`), untagged `config_{campaign,domain,mutex,url}` ("Use loader.n's decoded HGRF graph.config object
after confirming all three canonical campaign graphs match when this_role is omitted") and
`analysis_config_capsule_graph_config_{campaign,domain,mutex,url}` (from config.n). Old days unchanged (420 = 406 + 14).

Code:
- `formats/hcfg.rs`: `record_at(d, i, magic)` / `last_record(d, magic)` (HCFG layout under another magic, last
  CRC-valid hit, "scanning backward from the end"); `at` delegates to `record_at`.
- `lanes/binary_re.rs`: `Container::Neko` (`neko_bytecode` for exact/enum formats), `Spec::graph` (the HGRF record,
  scanned in every specimen), and a question-driven reader: "at <member> decoded HGRF JSON property <path>" picks the
  member and walks the path (`graph` = the record's root); the "graphs match when this_role is omitted" check compares
  every member's graph minus `this_role` and refuses on a difference.
- Role tags not in the member name (`config_capsule` for `-config.n`): tag routing now also drops leading field words
  on "unknown field" (`answer_dropping`, shared with the d29 named-member path).

All three modes CORRECT untimed on the first submit after the fix: single (420), TT (420), mastery (**14 sets**, 5880).
No ranked attempts spent. Fixtures: `ctf-run/fixtures/binary_re/d30-{single_solve,time_trial,mastery}.json`
(replay 81/81 clean).

Day 31 prediction: another multi-member campaign; the HGRF path reader is generic over member and JSON path, so a new
graph property (`edges`, `load_order`, `ipv4`) needs nothing unless it asks for a non-string value format.
