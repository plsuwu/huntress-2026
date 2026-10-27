# Day 27 metadata - Packet Loss Prevention (`metadata-forensics-day-27-packet-loss-prevention`)

Prompt: "Packet captures, exports, and host artifacts describe the same incident from different angles. Count
traffic, reject malformed captures, and find the host narrative missing network evidence." As usual only one new
question kind came out of it: single-solve asks it once, TT is the cumulative bundle (32 q), and mastery has
11 sets (352 q).

## New kind: `d27-pcap-001-pcap-packet-count`
- Graded: the number of walked records in a classic libpcap file (`capture-001.pcap`). answer_format `integer`.
- Variants seen: single = LE usec (`d4c3b2a1`, linktype 147, 3 records, last one sliced 10/64); mastery shipped all
  four classic magics: LE usec, **BE usec** (`a1b2c3d4`), **LE nsec** (`4d3cb2a1`), **BE nsec** (`a1b23c4d`).
  No pcapng yet.
- **Sliced packets** (incl_len < orig_len) are common (0-7 per file) and count as packets. Every graded file
  walked exactly to EOF, so the "malformed captures" rule (a record past EOF, incl > snaplen) is still **ungraded**:
  the parser stops there, counts complete records only and sets `malformed`.
- New `ctf-run/src/formats/pcap.rs`: classic (4 magics) + pcapng (SHB per-section byte order, IDB if_tsresol,
  EPB/SPB/OPB). `lanes/metadata.rs::pcap_answer` dispatches on `-pcap-<field>` after prefetch. Only `packet-count`
  is graded; the other fields (malformed/valid, link-type, snaplen, version, byte-order, timestamp precision,
  first/last timestamp ISO, duration, captured/original byte totals, truncated-packet-count) are guesses. An
  unknown field fails with a `pcap_facts` dump (per-packet offset/time/lengths/payload prefix).
- Predicted follow-ups: "reject malformed captures" (a valid/malformed flag, or a count on a capture with a
  record running past EOF - decide whether the partial record counts), pcapng, and a cross-artifact question
  ("host narrative missing network evidence": a host-log entry with no matching packet, likely by hostname/IP in
  payloads; payloads look like `huntress-<hex>` / `PCAP-PROOF` strings).

## Runs (all untimed, all CORRECT on the first submit after adding the kind)
- `ctf-run/runs-d27/.../single_solve-8920607e-1793106246`, `time_trial-ef9c6148-1793106259`,
  `mastery-282afdc2-1793106274` -> `fixtures/metadata/d27-{single_solve,time_trial,mastery}.json` (replay 73/73 clean).
