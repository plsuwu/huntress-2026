# Day 16 metadata - Return to Sender

New kind: `email-message-id` (`d16-email-NNN-email-message-id`) on a `.eml` (CRLF, multipart/mixed with a text
part + base64 `evidence-note.txt` attachment). Single-solve: `message-001.eml` ->
**`<artifact-assembly-line@example.test>` CORRECT - graded with the angle brackets**, as written in the header.

Implementation: `ctf-run/src/formats/email.rs` (header unfolding, RFC 2047 encoded words, RFC 2231 params,
multipart leaves with base64/QP decoding, `msg_ids` tokeniser) + `email_answer` in `ctf-run/src/lanes/metadata.rs`.
`message-id` / `in-reply-to` / `references` answer the `<...>` tokens space-joined (whitespace inside a folded id
removed). Any other `email-<header-name>` answers that header (unfolded, encoded words decoded); attachment
count/filename(s)/hash/size fields exist as unverified guesses. Keys containing `-email-` are routed to `email_answer` before the generic `-filename` / hash handlers (which would otherwise answer for the whole `.eml`); unknown email fields error with FACTS. FACTS dumps now list headers and MIME parts.

The prompt ("replies, forwards, duplicates, reconstructed messages ... rebuild which mail belongs together")
hints at threading kinds (In-Reply-To/References roots, duplicate Message-IDs) on later days; nothing like that
was asked on d16.

TT (21 questions) and mastery (105 questions, 5 sets) = the cumulative bundle + this kind; all CORRECT first try
(untimed regular attempts). Every set's email was identical to the single-solve message. Route: dual-pool,
ranked 32 / regular 32; TT 50 s, mastery 170 s. Fixtures `ctf-run/fixtures/metadata/d16-*.json`.
