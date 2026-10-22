#!/usr/bin/env python3
"""Shared config loader for the orchestration tools (see config.example.toml).

  import orchcfg; C = orchcfg.load()          # in Python tools (same directory, or sys.path)
  python3 tools/orchcfg.py get run_host.host  # in shell scripts: prints one value (lists space-separated)
  python3 tools/orchcfg.py lanes [field]      # prints lane names (or one field per lane), one per line
  python3 tools/orchcfg.py fmt KEY k=v ...    # prints a config string with placeholders filled (day=8 -> {day:02d}=08)
  python3 tools/orchcfg.py check              # validates the config and prints a summary

Config path: $ORCH_CONFIG, else <repo>/config.toml, where <repo> is the parent of this tools/ directory.
"""
import json
import os
import sys
import tomllib
import urllib.parse
import urllib.request

TOOLS = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(TOOLS)


class Config(dict):
    """The parsed TOML plus helpers. Paths returned by helpers are absolute."""

    def __getattr__(self, k):
        try:
            return self[k]
        except KeyError as e:
            raise AttributeError(k) from e

    # ------------------------------------------------------------ paths
    @property
    def repo(self):
        return REPO

    @property
    def solver_dir(self):
        return os.path.join(REPO, self["solver"]["dir"])

    def solver_bin(self, solver_dir=None):
        return os.path.join(solver_dir or self.solver_dir, self["solver"]["bin"])

    def route_cache(self):
        return os.path.expanduser(self["solver"]["route_cache"])

    def runs_dir(self, day, solver_dir=None):
        return os.path.join(solver_dir or self.solver_dir, self["solver"]["runs_dir"].format(day=day))

    def fixtures_dir(self, lane=None, solver_dir=None):
        d = os.path.join(solver_dir or self.solver_dir, self["solver"]["fixtures_dir"])
        return os.path.join(d, self.lane(lane)["fixtures"]) if lane else d

    def worktree(self, lane):
        return os.path.normpath(os.path.join(REPO, self["agents"]["worktree_root"], lane))

    def fmt(self, s, **kw):
        """Format a config string with the standard placeholders."""
        kw.setdefault("repo", REPO)
        kw.setdefault("solver", self["solver"]["dir"])
        return s.format(**kw)

    # ------------------------------------------------------------ lanes
    @property
    def lanes(self):
        return self["lanes"]

    def lane(self, name):
        for l in self["lanes"]:
            if l["name"] == name:
                return l
        raise KeyError(f"no lane {name!r} in the config")

    def prefixes(self):
        return tuple(l["prefix"] for l in self["lanes"])

    def lane_of(self, challenge_id):
        """The lane owning a challenge id, or None (then: do not touch it)."""
        for l in self["lanes"]:
            if challenge_id.startswith(l["prefix"]):
                return l
        return None

    def in_scope(self, challenge_id):
        return self.lane_of(challenge_id) is not None

    def day_prefix(self, lane, day):
        return f"{self.lane(lane)['prefix']}{day:02d}"

    def owns_day_route(self, lane, day, rid):
        p = self.day_prefix(lane, day)
        return rid == p or rid.startswith((p + "-", p + "/"))

    # ------------------------------------------------------------ modes
    @property
    def single_mode(self):
        return self["modes"]["single"]

    @property
    def timed_modes(self):
        return [m["name"] for m in self["modes"]["timed"]]

    def mode_flag(self, mode):
        for m in self["modes"]["timed"]:
            if m["name"] == mode:
                return m["flag"]
        raise KeyError(mode)

    # ------------------------------------------------------------ API
    def token(self):
        name = self["event"]["token_env"]
        tok = os.environ.get(name)
        if tok:
            return tok
        env = os.path.join(REPO, ".env")
        if os.path.exists(env):
            for line in open(env):
                if line.startswith(name + "="):
                    return line.split("=", 1)[1].strip()
        home = os.path.expanduser("~/.gm_token")
        if os.path.exists(home):
            return open(home).read().strip()
        sys.exit(f"no {name} (env, {env} or {home})")

    def url(self, key, **kw):
        q = {k: urllib.parse.quote(str(v), safe="/") for k, v in kw.items()}
        return self["event"]["api"].rstrip("/") + self["api"][key].format(**q)

    def api_get(self, key, timeout=60, **kw):
        req = urllib.request.Request(self.url(key, **kw), headers={"Authorization": f"Bearer {self.token()}"})
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return json.loads(r.read())

    def solver_env(self, ranked=False):
        """Environment for solver subprocesses: token, API URL and (unless ranked) the no-ranked guard."""
        e = dict(os.environ)
        e[self["event"]["token_env"]] = self.token()
        e[self["event"]["url_env"]] = self["event"]["api"]
        if not ranked:
            e[self["solver"]["no_ranked_env"]] = self["solver"]["no_ranked_value"]
        return e


def path():
    return os.environ.get("ORCH_CONFIG") or os.path.join(REPO, "config.toml")


def load(p=None):
    p = p or path()
    if not os.path.exists(p):
        sys.exit(f"no config at {p} (copy config.example.toml, or set ORCH_CONFIG)")
    with open(p, "rb") as f:
        c = Config(tomllib.load(f))
    names = [l["name"] for l in c.get("lanes", [])]
    if not names:
        sys.exit(f"{p}: no [[lanes]]")
    if len(set(names)) != len(names):
        sys.exit(f"{p}: duplicate lane names")
    return c


def _dig(c, dotted):
    v = c
    for k in dotted.split("."):
        v = v[k]
    return v


def main():
    a = sys.argv[1:]
    if not a:
        sys.exit(__doc__)
    c = load()
    if a[0] == "get":
        v = _dig(c, a[1])
        print(" ".join(map(str, v)) if isinstance(v, list) else v)
    elif a[0] == "lanes":
        for l in c.lanes:
            print(l[a[1]] if len(a) > 1 else l["name"])
    elif a[0] == "fmt":
        kw = dict(x.split("=", 1) for x in a[2:])
        kw = {k: int(v) if v.isdigit() else v for k, v in kw.items()}
        print(c.fmt(_dig(c, a[1]), **kw))
    elif a[0] == "check":
        print(f"config {path()}: event {c['event']['name']} at {c['event']['api']}")
        print(f"solver {c.solver_bin()} ({'present' if os.path.exists(c.solver_bin()) else 'not built'})")
        for l in c.lanes:
            print(f"lane {l['name']:12} prefix {l['prefix']:28} fixtures {c.fixtures_dir(l['name'])}")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
