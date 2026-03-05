import os, json, urllib.request, urllib.error

f = os.path.join(os.path.expanduser("~"), ".antigravity_cockpit", "accounts", "78fa7aa5-7162-4d17-b484-8f303a262533.json")
d = json.load(open(f, encoding="utf-8"))
t = d["token"]
at = t["access_token"]
pid = t.get("project_id", "")
print("Testing with project_id:", pid)

ph = urllib.request.ProxyHandler()
opener = urllib.request.build_opener(ph)

# Step 1: loadCodeAssist
print("\n--- Step 1: loadCodeAssist ---")
body = json.dumps({"metadata": {"ideType": "ANTIGRAVITY"}}).encode("utf-8")
req = urllib.request.Request(
    "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist",
    data=body,
    headers={
        "Authorization": f"Bearer {at}",
        "Content-Type": "application/json",
        "User-Agent": "grpc-java-okhttp/1.68.2"
    },
    method="POST"
)
try:
    resp = opener.open(req, timeout=15)
    print(f"loadCodeAssist HTTP: {resp.getcode()}")
    rd = json.loads(resp.read().decode("utf-8"))
    print("loadCodeAssist resp keys:", list(rd.keys()))
    cp = rd.get("cloudaicompanionProject", "")
    print("cloudaicompanionProject:", cp)
    # Use the fresh project_id for next step
    if cp:
        pid = cp
except urllib.error.HTTPError as e:
    print(f"loadCodeAssist HTTPError: {e.code}")
    print(e.read().decode("utf-8")[:500])
except Exception as e:
    print(f"loadCodeAssist error: {e}")

# Step 2: fetchAvailableModels with project
print(f"\n--- Step 2: fetchAvailableModels (project={pid}) ---")
body2 = json.dumps({"project": pid}).encode("utf-8")
req2 = urllib.request.Request(
    "https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels",
    data=body2,
    headers={
        "Authorization": f"Bearer {at}",
        "Content-Type": "application/json",
        "User-Agent": "grpc-java-okhttp/1.68.2"
    },
    method="POST"
)
try:
    resp2 = opener.open(req2, timeout=15)
    print(f"fetchAvailableModels HTTP: {resp2.getcode()}")
    rd2 = json.loads(resp2.read().decode("utf-8"))
    if "models" in rd2:
        models = list(rd2["models"].keys())
        print(f"成功! 获取到 {len(models)} 个模型")
        for m in models[:10]:
            print(f"  - {m}")
    else:
        print("resp:", rd2)
except urllib.error.HTTPError as e:
    print(f"fetchAvailableModels HTTPError: {e.code}")
    print(e.read().decode("utf-8")[:500])
except Exception as e:
    print(f"fetchAvailableModels error: {e}")

# Step 3: fetchAvailableModels without project
print("\n--- Step 3: fetchAvailableModels (no project) ---")
body3 = json.dumps({}).encode("utf-8")
req3 = urllib.request.Request(
    "https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels",
    data=body3,
    headers={
        "Authorization": f"Bearer {at}",
        "Content-Type": "application/json",
        "User-Agent": "grpc-java-okhttp/1.68.2"
    },
    method="POST"
)
try:
    resp3 = opener.open(req3, timeout=15)
    print(f"fetchAvailableModels HTTP: {resp3.getcode()}")
    rd3 = json.loads(resp3.read().decode("utf-8"))
    if "models" in rd3:
        models = list(rd3["models"].keys())
        print(f"成功! 获取到 {len(models)} 个模型")
        for m in models[:10]:
            print(f"  - {m}")
    else:
        print("resp:", rd3)
except urllib.error.HTTPError as e:
    print(f"fetchAvailableModels HTTPError: {e.code}")
    print(e.read().decode("utf-8")[:500])
except Exception as e:
    print(f"fetchAvailableModels error: {e}")
