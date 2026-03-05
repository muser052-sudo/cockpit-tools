import os
import json
import urllib.request
import urllib.error
import uuid

def get_account(email):
    data_dir = os.path.join(os.path.expanduser("~"), ".antigravity_cockpit", "accounts")
    for f in os.listdir(data_dir):
        if not f.endswith(".json"): continue
        acc = json.load(open(os.path.join(data_dir, f), encoding="utf-8"))
        if acc.get("email") == email:
            return acc
    return None

def test_google_fetch_models(access_token, project_id):
    print("\n========================================")
    print("1. 测试向 Google 上游获取模型列表 (fetchAvailableModels) ")
    print("========================================")
    
    base_urls = [
        "https://cloudcode-pa.googleapis.com",
        "https://daily-cloudcode-pa.sandbox.googleapis.com",
        "https://daily-cloudcode-pa.googleapis.com"
    ]
    
    # gcli2api 发送空的 json {}
    body = json.dumps({}).encode('utf-8')
    
    headers = {
        'User-Agent': 'antigravity/2.15.8 (Windows; AMD64)',
        'Authorization': f'Bearer {access_token}',
        'Content-Type': 'application/json'
    }
    
    final_models = []
    
    for base_url in base_urls:
        url = f"{base_url}/v1internal:fetchAvailableModels"
        print(f"\nTrying URL: {url}")
        req = urllib.request.Request(url, data=body, headers=headers, method="POST")
        
        try:
            ph = urllib.request.ProxyHandler()
            opener = urllib.request.build_opener(ph)
            resp = opener.open(req, timeout=15)
            print(f"Status: {resp.getcode()}")
            rd = json.loads(resp.read().decode('utf-8'))
            if 'models' in rd:
                models = list(rd['models'].keys())
                print(f"成功获取到 {len(models)} 个模型")
                print("模型列表:", models[:5], "...")
                final_models = models
                break
            else:
                print("响应中没有 models:", rd)
        except urllib.error.HTTPError as e:
            print(f"HTTPError: {e.code}")
            print(e.read().decode('utf-8')[:300])
        except Exception as e:
            print(f"Failed: {e}")
            
    return final_models

def test_google_chat(access_token, project_id, model):
    print("\n========================================")
    print(f"2. 测试向 Google 上游发对话请求 (streamGenerateContent) - {model}")
    print("========================================")
    
    url = "https://cloudcode-pa.googleapis.com/v1internal:streamGenerateContent?alt=sse"
    
    payload = {
        "model": model,
        "project": project_id,
        "request": {
            "contents": [
                {
                    "parts": [{"text": "hello，请回答一个字：在。"}],
                    "role": "user"
                }
            ],
            "generationConfig": {"maxOutputTokens": 100}
        }
    }
    
    body = json.dumps(payload).encode('utf-8')
    
    headers = {
        'User-Agent': 'antigravity/2.15.8 (Windows; AMD64)',
        'Authorization': f'Bearer {access_token}',
        'Content-Type': 'application/json',
        'requestId': f"req-{uuid.uuid4()}",
        'requestType': "agent"
    }
        
    req = urllib.request.Request(url, data=body, headers=headers, method="POST")
    try:
        ph = urllib.request.ProxyHandler()
        opener = urllib.request.build_opener(ph)
        resp = opener.open(req, timeout=30)
        print(f"Status: {resp.getcode()}")
        
        for line in resp:
            line_str = line.decode('utf-8').strip()
            if line_str:
                print("RAW SSE:", line_str)
    except urllib.error.HTTPError as e:
        print(f"HTTPError: {e.code}")
        print(e.read().decode('utf-8')[:500])
    except Exception as e:
        print(f"Failed: {e}")

if __name__ == "__main__":
    email = "chenyiding01@gmail.com"
    acc = get_account(email)
    if not acc:
        print(f"账号 {email} 不存在")
        exit(1)
        
    token = acc.get("token", {})
    access_token = token.get("access_token", "")
    project_id = token.get("project_id", "")
    
    print(f"Test with Account: {email}")
    print(f"Project ID: {project_id}")
    
    models = test_google_fetch_models(access_token, project_id)
    
    # 即使 fetch 失败，我们也强行拿一个常见模型测试对话，证明接口能通
    test_model = models[0] if models else "gemini-2.5-flash"
    
    test_google_chat(access_token, project_id, test_model)
