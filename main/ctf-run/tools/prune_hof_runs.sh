#!/bin/bash
# Free jumpbox disk: delete deliveries/*.zip in binary-RE ranked run dirs under ~/ctf-run-v2/runs-hof, except the
# newest run per (route, mode). JSON (start/meta/answers/result) is kept. Binary evidence zips are up to ~32 MB each;
# on 2026-10-25 they filled the 38 GB disk mid-campaign. Dry run by default; `--do` deletes.
# Run it from here without copying it over (a full disk can't take the scp):
#   ssh -i huntress-id_ed25519 HuntressCTF@10.1.181.90 'bash -s -- --do' < ctf-run/tools/prune_hof_runs.sh
cd ~/ctf-run-v2/runs-hof || exit 1
total=0; n=0
for base in binary-reverse-engineering-day-*/; do
  for mode in time_trial mastery; do
    dirs=$(ls -d "$base$mode"-* 2>/dev/null | awk -F- '{print $NF" "$0}' | sort -n | awk '{print $2}')
    [ -z "$dirs" ] && continue
    newest=$(echo "$dirs" | tail -1)
    for d in $(echo "$dirs" | head -n -1); do
      for z in "$d"/deliveries/*.zip; do
        [ -f "$z" ] || continue
        s=$(stat -c %s "$z"); total=$((total+s)); n=$((n+1))
        [ "$1" = "--do" ] && rm -f -- "$z"
      done
    done
  done
done
echo "$([ "$1" = "--do" ] && echo deleted || echo would delete) $n zips, $((total/1024/1024)) MB"
df -h ~ | tail -1
