# Day 27 binary RE - Compiler Attribution ("Compiler Says What")

d27 = **two x86-64 ELF builds of one C specimen**, one per toolchain (CD order): `d27--sample-gcc.elf` (108 KB)
and `d27--sample-tcc.elf` (462 KB). The HCFG record is raw in each (found by `hcfg::raw`, CRC-validated). The
toolchain fingerprints themselves (runtime/linker differences) are not scored.

New d27 questions (12, same shape as the d24/d25 pairs): `artifact_sha256_{1,2}` (CD order), tagged
`analysis_{gcc,tcc}_format` (enum -> `elf`) and `analysis_{gcc,tcc}_machine` (integer e_machine -> `62`),
untagged `config_{campaign,domain,mutex,url}` ("require their config objects to be byte-for-byte equal") answered
from the first member (gcc.elf), and tagged `gcc_config_{campaign,domain}`.

**No code change.** The automated first pass got all three modes CORRECT on the first submit (untimed): single (324
answers), TT (324), mastery (**11 sets**, 3564). Old days unchanged (d01-d27 at 12 each = 324).
The untagged configs still assume both members carry equal configs (no explicit equality check); held in every set.
Fixtures: `ctf-run/fixtures/binary_re/d27-{single_solve,time_trial,mastery}.json` (replay 72/72 clean).
