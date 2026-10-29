# Day 29 metadata - Trust Issues (`metadata-forensics-day-29-trust-issues`)

Prompt: "Certificates appear in binaries, packages, mail, captures, and disk images. Fingerprint each one, collapse
duplicates, and identify the trust claim unsupported by evidence." As usual only one new question kind came out of
it: single-solve asks it once, TT is the cumulative bundle (34 q), and mastery has **13 sets** (442 q).

## New kind: `d29-x509-001-x509-sha256-fingerprint`
- Graded: SHA-256 of the certificate's **DER** encoding (PEM body base64-decoded), lowercase hex, no colons.
  answer_format `pattern` (64 hex), normalization `case_insensitive`.
- Every variant seen (single, TT, 13 mastery sets) was a standalone `certificate-001.pem` (one PEM block, ~709 B).
  Single's cert: self-signed Ed25519, v3, `CN=arena-<hex>.example.test`, `O=Huntress CTF`, SAN = the CN.
- New `ctf-run/src/formats/x509.rs`: minimal DER walker; `find_all` finds PEM `CERTIFICATE` blocks and raw DER
  certificates anywhere in a file (prepared for "certificates in binaries, packages, mail, captures, disk images").
  `lanes/metadata.rs::x509_answer` dispatches on `-x509-<field>` after disk; identical copies collapse, several
  distinct certificates in one file are an error (fails with an x509 facts dump). Only `sha256-fingerprint` is
  graded; sha1/md5 fingerprints, serial (leading `00` stripped), subject/issuer CN and DN, validity (ISO `Z`),
  version, signature/key algorithm, SAN list, self-signed and the certificate counts are guesses. `facts()` now
  lists certificates too.
- Predicted follow-ups: a certificate embedded in an earlier family (PE Authenticode `WIN_CERTIFICATE` PKCS#7,
  APK `META-INF/*.RSA`, email attachment / S/MIME, pcap TLS payload, disk partition) - `find_all`'s raw DER scan
  already finds certs inside PKCS#7 blobs but picks every cert in the chain (error on >1 distinct); a "unique
  fingerprint count" / "collapse duplicates" question; or "trust claim unsupported" (issuer not present among the
  evidence certs, self-signed claimed as CA-issued, expired at a given time).

## Runs (all untimed, all CORRECT on the first submit after adding the kind)
- `ctf-run/runs-d29/.../single_solve-aea51114-1793278917`, `time_trial-9ace0ce7-1793278928`,
  `mastery-fe0871d6-1793278942` -> `fixtures/metadata/d29-{single_solve,time_trial,mastery}.json` (replay 79/79 clean
  before the fixtures were added).
