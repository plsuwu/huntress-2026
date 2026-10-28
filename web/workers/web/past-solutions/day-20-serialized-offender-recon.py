#!/usr/bin/env python3
"""Recon helper for Serialized Offender (day-26). Scoped to one target origin.

Fetches root + a handful of likely discovery endpoints, prints status, headers
(esp. Set-Cookie), and body excerpts so we can classify the session/deser bug.
"""
import sys
import http.cookiejar
import urllib.request
import urllib.error

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
TIMEOUT = 30


def fetch(opener, base, path, method="GET", data=None, headers=None):
    url = base.rstrip("/") + path
    hdrs = {"User-Agent": UA}
    if data is not None:
        hdrs["Content-Type"] = "application/json"
    hdrs.update(headers or {})
    req = urllib.request.Request(url, data=data, headers=hdrs, method=method)
    try:
        with opener.open(req, timeout=TIMEOUT) as r:
            return r.status, dict(r.headers), r.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        return e.code, dict(e.headers), e.read().decode("utf-8", "replace")
    except Exception as e:  # noqa
        return None, {}, f"ERR {e}"


def main():
    base = sys.argv[1].rstrip("/")
    cj = http.cookiejar.CookieJar()
    opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cj))
    paths = ["/", "/api/clues", "/robots.txt", "/login", "/dashboard",
             "/profile", "/me", "/api/me", "/openapi.json"]
    for p in paths:
        st, hdrs, body = fetch(opener, base, p)
        print(f"\n===== {p} -> {st} =====")
        for k in ("Set-Cookie", "Content-Type", "Server", "Location", "X-Powered-By"):
            if k in hdrs:
                print(f"  {k}: {hdrs[k]}")
        print("  body[:1200]:")
        print("  " + body[:1200].replace("\n", "\n  "))
    print("\n===== cookie jar =====")
    for c in cj:
        print(f"  {c.name} = {c.value}")


if __name__ == "__main__":
    main()
