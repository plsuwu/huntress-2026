# Day 30 warmups - Percent Percent

Encoding: **double percent-encoding** ("That address bar problem? It happened twice."). The single-solve/TT blob is
the sentence percent-encoded byte by byte (`%4e%69...`), then percent-encoded again (`%25%34%65%25%36%39...`).
Mastery row 30 is the same per fragment (`30\t%25%36%63%25%32%64%25%33%36%25%33%36` -> `%6c%2d%36%36` -> `l-66`).
148 mastery sets (5N-2).

Code: **no change**. `codec::percent_decode_all` already decodes to a fixpoint, so the blob peel (`T::Pct`) and the
row path (`structured()` -> `has_percent` -> `T::Pct`; `fixed_codec(30)` is unset) both reach the sentence.

The automated first pass (2026-10-30 ~13:00Z) was CORRECT on every mode: single-solve (`calm-prairie-ridge-16`),
regular time trial and regular mastery. TT/mastery are dual-pool; Hall of Fame is the coordinator's.
Fixtures: `ctf-run/fixtures/warmups/d30-{single_solve,time_trial,mastery}.json`; replay 87/87 clean.
Run dirs: `ctf-run/runs-d30/warmups-day-30/` (`single_solve-a3f612cb-1793365204`, `time_trial-a7f73e4b-1793365210`,
`mastery-52321a50-1793365217`).

Day 31 guess: another "familiar layer, applied twice/twisted" (double base64 and double hex are already peeled by the
blob loop; rows would need the layer repeated, which `structured()` does only for percent), or the long-predicted
whitespace language / quoted-printable / `0x4E` tokens, none of which are decoded yet.
