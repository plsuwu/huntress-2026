#!/usr/bin/env python3
"""Generate agent prompts for every challenge (one per lane) of a given CTF day.

Queries the Game Master list endpoint and renders PROMPT_TEMPLATE once per
(day, lane) challenge - the single_solve / time_trial / mastery variants share a
base id, so they collapse into one prompt (the template itself tells the agent to
continue through the modes).

    utils/gen_prompts.py 6                    # all lanes for day 6
    utils/gen_prompts.py 6 --lane warmups     # one lane (repeatable)
    utils/gen_prompts.py 6 --unsolved         # skip challenges with every mode solved
    utils/gen_prompts.py 6 --json             # machine-readable

The bearer token is read from $HUNTRESS_GM_TOKEN, falling back to a `.env` file in
the current directory or the repo root. It is never embedded in this script.
"""

import argparse
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

BASE_URL = os.environ.get("HUNTRESS_GM_URL", "https://2026.huntress.ctf.games/")

PROMPT_TEMPLATE = (
    'Read AGENTS.md, and then proceed to solve the day {day} challenge "{id}" '
    '("{lane}" lane). After each successful solution, tidy your workdir, organize '
    "notes, and then continue to the next mode. Solutions and notes exists for "
    "previous {lane} challenges; see `utils/solver_py_scripts/`; relevant "
    "directories carry the -{lane} suffix."
)

MODES = ("single_solve", "time_trial", "mastery")


def load_token() -> str:
    token = os.environ.get("HUNTRESS_GM_TOKEN")
    if token:
        return token.strip()
    here = Path(__file__).resolve().parent
    for env in (Path.cwd() / ".env", here.parent / ".env", here / ".env"):
        if not env.is_file():
            continue
        for line in env.read_text().splitlines():
            line = line.strip()
            if line.startswith("export "):
                line = line[len("export "):]
            key, sep, value = line.partition("=")
            if sep and key.strip() == "HUNTRESS_GM_TOKEN":
                return value.strip().strip("'\"")
    sys.exit("HUNTRESS_GM_TOKEN not set and not found in a .env file")


def list_challenges(day: int, token: str) -> list[dict]:
    query = urllib.parse.urlencode({"op": "list", "day": day, "all": "true"})
    req = urllib.request.Request(
        f"{BASE_URL}?{query}", headers={"Authorization": f"Bearer {token}"}
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            data = json.load(resp)
    except urllib.error.HTTPError as e:
        sys.exit(f"Game Master returned HTTP {e.code}: {e.read()[:300]!r}")
    except urllib.error.URLError as e:
        sys.exit(f"request failed: {e.reason}")
    # The `day` filter is applied server-side; re-check so a changed API can't
    # silently leak other days into the output.
    return [c for c in data.get("challenges", []) if c.get("day") == day]


def build_prompts(challenges: list[dict], lanes: set[str], unsolved: bool) -> list[dict]:
    groups: dict[str, list[dict]] = {}
    for c in challenges:  # dicts preserve API order => stable lane order
        base = c["id"].split("/", 1)[0]
        groups.setdefault(base, []).append(c)

    out = []
    for base, members in groups.items():
        # Prefer the single_solve entry as the canonical title/lane source.
        members.sort(key=lambda c: MODES.index(c["mode"]) if c["mode"] in MODES else 99)
        head = members[0]
        if lanes and head["lane"] not in lanes:
            continue
        solved = {c["mode"]: bool(c.get("solved")) for c in members}
        if unsolved and all(solved.values()):
            continue
        out.append({
            "day": head["day"],
            "lane": head["lane"],
            "title": head["title"],
            "id": base,
            "modes": solved,
            "prompt": PROMPT_TEMPLATE.format(
                day=f"{head['day']:02d}", id=base, lane=head["lane"]
            ),
        })
    return out


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("day", type=int, help="CTF day number (1-31)")
    ap.add_argument("--lane", action="append", default=[],
                    help="restrict to a lane (repeatable), e.g. script-analysis")
    ap.add_argument("--unsolved", action="store_true",
                    help="skip challenges where every mode is already solved")
    ap.add_argument("--json", action="store_true", help="emit JSON instead of text")
    args = ap.parse_args()

    challenges = list_challenges(args.day, load_token())
    prompts = build_prompts(challenges, set(args.lane), args.unsolved)
    if not prompts:
        sys.exit(f"no challenges matched for day {args.day}")

    if args.json:
        json.dump(prompts, sys.stdout, ensure_ascii=False, indent=2)
        print()
        return

    for i, p in enumerate(prompts):
        if i:
            print()
        status = ", ".join(f"{m}={'solved' if s else 'open'}" for m, s in p["modes"].items())
        print(f"# [{p['lane']}] {p['id']} ({status})")
        print(p["prompt"])


if __name__ == "__main__":
    main()
