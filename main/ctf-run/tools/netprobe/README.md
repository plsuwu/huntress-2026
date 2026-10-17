# netprobe: when is the Game Master fastest?

The goal is to find out whether there's a quieter time of day for timed and ranked runs. A systemd user timer
(`install-timer.sh`, every 10 min) runs `collect.py` locally. It pipes `remote_probe.py` to the jumpbox's `python3` over
ssh, so nothing needs deploying. One JSON line is appended to `ctf-run/runs/netprobe/probes.jsonl`, which is gitignored.

- **Burst:** 5 rounds, 0.25 s apart, round-robin over these targets, each on a fresh connection. None of them
  changes game state:
  - `proxy_status`: proxy `?op=status&challenge=warmups-day-01`. Goes through to the Game Master backend.
  - `proxy_download`: that solved challenge's artifact, via the capability link in the status response. This is the
    download leg, the long pole of a timed run's window.
  - `proxy_submit`: a junk `submit` to the solved challenge. The GM answers `already_solved` ("Further submissions are
    not scored"). Only the `result` keyword is kept. Any other result turns the submit target off for the rest of that
    burst, and `report.py` flags it. Note that this measures the submit path, not grading: a solved attempt is never
    graded.
  - `proxy_edge`: proxy `?op=openapi`. CourseStack/Vercel answers this without any GM work, at about 70 ms.
  - `direct_status`: the external host with the bearer token from `~/.huntress_token`. Reaches the backend without the
    proxy.
  - `ref_cloudflare`: the jumpbox to `https://1.1.1.1/cdn-cgi/trace`. A control for the jumpbox's own network.
- **Local baseline (`local`):** before the ssh, this machine pings 1.1.1.1, 8.8.8.8 and the jumpbox over the VPN (loss,
  RTT, mdev jitter). It also makes 3 fresh-connection HTTPS GETs each to Cloudflare and Google (TCP / TLS / TTFB).
  `report.py --exclude-degraded` drops bursts where a control was bad: local ping loss, or local/jumpbox Cloudflare
  TTFB above 2x its own median. The GM numbers are measured from the jumpbox, so local conditions only affect them
  indirectly (via the VPN and ssh). The `ref_cloudflare` control is the one that matters for the GM columns.
- **Cold vs warm:** `i=0` is the first request after about 10 idle minutes. On 2026-10-18 it was 1.6-2.3 s cold vs
  0.25-0.45 s warm. `report.py` keeps cold and warm in separate columns, so cold starts don't swamp the averages.
- **Skips:** the burst is skipped (`{"skipped":"campaign"}`) whenever `hof_campaign`/`campaign.py` or any `ctf-run` binary
  is running on the jumpbox. That way probes never compete with a ranked run for the t3.small's CPU.
- **Errors:** ssh/VPN failures and timeouts are logged as `{"error": ...}` records. Non-2xx responses and request errors
  count towards `err%`.
- **Overhead check:** `ssh_wall - elapsed` is the local<->jumpbox overhead. `steal_pct` is the jumpbox CPU steal during
  the burst.

```sh
python3 ctf-run/tools/netprobe/report.py [--exclude-degraded]  # by AEST hour; best proxy_download hours last
python3 ctf-run/tools/netprobe/report.py --by weekday-hour --target proxy_status --since 2026-10-18
python3 ctf-run/tools/netprobe/collect.py                      # one manual burst
ctf-run/tools/netprobe/install-timer.sh [--remove]             # journalctl --user -fu huntress-netprobe
```

Caveat: none of these exercise grading, because solved attempts aren't graded. Treat them as stand-ins for backend load. Use the HoF `elapsed_ms` values (`tools/run_legs.py`) to confirm a "good hour" once
the probe data points to one.
