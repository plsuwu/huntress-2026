# Day 24 binary RE - Patch Diffing ("Two builds, one fix")

d24 = **two Scala JARs** (CD order): `d24--sample-vulnerable.jar` (4 KB) and `d24--sample-corrected.jar` (4.4 KB),
each `Main-Class` + a `Day24<Role>000$` module class ("Huntress deterministic Scala factory"; Scala 2.13 stdlib not
bundled). The corrected `score` adds an `Option.filter` lambda (`$anonfun$score$1..3`) around the
`HUNTRESS_HOST_MODEL` env lookup; the vulnerable one indexes the string unchecked. None of the diff is scored.
The HCFG record is a base64 classfile constant (`encodedConfig`) in both jars, identical config.

New d24 questions (12): `artifact_sha256_{1,2}`, `analysis_{vulnerable,corrected}_format` (enum -> `jar`, routed by
the `-vulnerable`/`-corrected` name tag), untagged `config_{campaign,domain,mutex,url}` ("decode both role members and
require their config objects to be byte-for-byte equal") and tagged `corrected_config_*`. Untagged config fields are
answered from the primary (first) member = vulnerable.jar; `jar_hcfg` already finds the base64 constant.
If a future variant ships unequal configs the untagged answer would need an explicit both-equal check (not seen).

**No code change.** The automated first pass got all three modes CORRECT on the first submit (untimed): single (288
answers), TT (288), mastery (8 sets, 2304 answers). Old days unchanged (d01-d24 at 12 each = 288). Every mastery set
keeps the same CD order (vulnerable, corrected).
Fixtures: `ctf-run/fixtures/binary_re/d24-{single_solve,time_trial,mastery}.json` (replay 63/63 clean).
