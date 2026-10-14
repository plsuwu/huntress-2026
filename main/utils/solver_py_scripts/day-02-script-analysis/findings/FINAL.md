## Solver

The reusable Python solver is [host_with_the_most.py](../host_with_the_most.py). It starts or resumes the attempt, downloads and hash-checks the assigned archive, statically decodes the PowerShell and JScript data, builds answers using the returned IDs, and submits them. It does not execute the challenge scripts. The answer JSON files above preserve each completed solution.

## Blockers and fixes

- The archives use AES ZIP encryption that the installed `unzip` could not extract. I used `pyzipper` in the workspace virtual environment with the archive password `infected`; the downloaded bytes matched the Game Master hashes.
- The JScript variants changed their encodings. I extended the static decoder to handle byte shifts, reverse operations, cyclic XOR, differential XOR, and separate Mastery deliveries.
- The first Single Solve submission was incorrect. **Correction to my earlier progress update:** the challenge had 87 fields; 80 were accepted initially and seven needed correction. The Game Master’s per-answer feedback identified those fields, and the corrected mappings completed the mode.
- The timed modes had five-second deadlines. I let early attempts expire while adapting the decoder, then used the Game Master’s retry operation and solved both modes. Their reports record the final assigned material and deadlines.
- I opened Day 1 Power Play while investigating the archive issue, but it was not needed once `infected` unlocked the files, so I did not submit that challenge.
