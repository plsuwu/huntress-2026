# Background

Read the layout of in `/home/please/Documents/huntress-ctfs/2026` - this setup is named the "main solution pipeline". 

The main set up in such a way that it is able to function autonomously through well-crafted scripts for quickly
contextualizing orchestrators and workers on their tasks and goals. Human overview of in-progress work is intuitive via 
Tmux sessions, which makes it easy to view Claude workers and the interactive instance means the human can guide workers
directly whenever necessary.

The current directory, `/home/please/Documents/huntress-ctfs/2026-web`, referred to as the "web solution pipeline", 
looks to adapt the efficient workflow from the "main solution pipeline" to streamline the way `web-exploitation` challenges 
are solved.

# Requirements

Where workers in the main solution pipeline exclusively solve for the "main 5" challenge lanes (`binary`, `metadata`,
`mobile`, `script`, `warmups`), the web solution pipeline exclusively solves for `web-exploitation` lane challenges.

While `web-exploitation` is only one one lane, multiple workers are required as the lane always contains more than one
challenge each day (where the other lanes have contained one challenge per lane). The exact number of web-exploitation 
challenges has varied day-to-day, so we will likely need to pre-warm/contextualize 3 workers and provide them with their
challenge IDs when the challenges become available.

The web solution pipeline should name its tooling uniquely to avoid clobbering or merging into the main solution pipeline, 
e.g. Tmux server names in the web pipeline should not have names that conflict with the main pipeline, and worker 
directories should be unique to the web lane.

Worker prompts may also have to be adapted slightly; rather than being specifically instructed on their lane, a worker 
should be given its challenge ID and instructed to solve the `single_solve`, `time_trial`, and `mastery` modes for that
challenge as quickly as possible.

The `ctf-run` binary is not necessary for the web pipeline - workers should solve using `uv`/Python.

Workers should take notes about their solutions, and the orchestrator can merge these into a document that can be used 
to provides assistance to future worker agents.
