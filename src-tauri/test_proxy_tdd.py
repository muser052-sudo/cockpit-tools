import os
import json
import urllib.request
import urllib.error

def get_account(email):
    data_dir = os.path.join(os.path.expanduser("~"), ".antigravity_cockpit", "accounts")
    for f in os.listdir(data_dir):
        if not f.endswith(".json"): continue
        acc = json.load(open(os.path.join(data_dir, f), encoding="utf-8"))
        if acc.get("email") == email:
            return acc
    return None

VALID_PREFIXES = ["gemini-", "claude-", "gpt-", "o1", "o3", "o4", "tab_"]

# 测试使用的账号
TEST_CODEX_EMAIL = "aagejegeoeeenenjekwhelwkwkgwke@gmail.com"


def test_google_fetch_models(access_token, project_id):
    print("\n========================================")
    print("1. 测试向 Google 上游获取模型列表 (fetchAvailableModels) ")
    print("========================================")
    
    base_urls = [
        "https://cloudcode-pa.googleapis.com",
        "https://daily-cloudcode-pa.sandbox.googleapis.com",
        "https://daily-cloudcode-pa.googleapis.com"
    ]
    
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
                all_keys = list(rd['models'].keys())
                models = [k for k in all_keys if any(k.startswith(p) for p in VALID_PREFIXES)]
                filtered = len(all_keys) - len(models)
                print(f"成功获取到 {len(models)} 个模型 (过滤了 {filtered} 个非模型key)")
                for m in models:
                    qi = rd['models'][m].get('quotaInfo', {})
                    rem = qi.get('remainingFraction')
                    rem_str = f"{rem*100:.0f}%" if rem is not None else "无"
                    print(f"  {m}: 余额={rem_str}")
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
                    "parts": [{"text": "Say pong"}],
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
        'requestId': f"req-test",
        'requestType': "agent"
    }

    req = urllib.request.Request(url, data=body, headers=headers, method="POST")

    try:
        ph = urllib.request.ProxyHandler()
        opener = urllib.request.build_opener(ph)
        resp = opener.open(req, timeout=30)
        print(f"Status: {resp.getcode()}")
        body = resp.read(300).decode('utf-8')
        print(f"✅ 对话成功，收到数据")
        print(f"  预览: {body}")
    except urllib.error.HTTPError as e:
        print(f"❌ HTTPError: {e.code}")
        print(e.read().decode('utf-8')[:300])
    except Exception as e:
        print(f"❌ Failed: {e}")

def test_codex_chat(access_token, model="gpt-5.1-codex"):
    print("\n========================================")
    print(f"4. 测试向 Codex 上游发对话请求 (chat/completions -> responses) - {model}")
    print("========================================")
    
    url = "https://chatgpt.com/backend-api/codex/responses"
    
    payload = json.dumps({
        "model": model,
        "input": [{
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "Say pong"
            }]
        }],
        "stream": True,
        "store": False,
        "instructions": "You are a helpful assistant."
    }).encode('utf-8')

    headers = {
        'Content-Type': 'application/json',
        'Authorization': f'Bearer {access_token}',
        'User-Agent': 'codex_cli_rs/0.104.0',
        'originator': 'codex_cli_rs',
        'Accept': 'text/event-stream'
    }

    req = urllib.request.Request(url, data=payload, headers=headers, method="POST")

    try:
        ph = urllib.request.ProxyHandler()
        opener = urllib.request.build_opener(ph)
        resp = opener.open(req, timeout=30)
        print(f"Status: {resp.getcode()}")
        
        has_content = False
        empty_chunks = 0
        
        for line in resp:
            line_str = line.decode('utf-8').strip()
            if not line_str or line_str == "data: [DONE]":
                continue
            if line_str.startswith("data: "):
                try:
                    data = json.loads(line_str[6:])
                    if 'choices' in data and len(data['choices']) > 0:
                        delta = data['choices'][0].get('delta', {})
                        content = delta.get('content', '')
                        if content:
                            print(f"  Chunk: {repr(content)}")
                            has_content = True
                        else:
                            empty_chunks += 1
                except Exception as ex:
                    print(f"  Parse Error: {ex} - {line_str}")
        
        print(f"\nResult: {'✅ 成功' if has_content else '❌ 失败 (无文本内容)'}, 收到 {empty_chunks} 个空片段")
        if not has_content:
            raise Exception("No content received from Codex Proxy Stream")

    except urllib.error.HTTPError as e:
        print(f"HTTPError: {e.code}")
        print(e.read().decode('utf-8')[:300])
    except Exception as e:
        print(f"Failed: {e}")

def test_codex_chat_via_proxy(model="gpt-5.1-codex", email=None):
    print("\n========================================")
    print(f"4.5 测试向本地代理发 Codex 对话请求 (/codex/v1/chat/completions) - {model}")
    print("========================================")
    
    url = "http://127.0.0.1:19531/codex/v1/chat/completions"
    
    payload = json.dumps({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": "Hi"
            },
            {
                "role": "assistant",
                "content": "Hello! How can I help you today?"
            },
            {
                "role": "user",
                "content": "Say pong in exact 2 words"
            }
        ],
        "stream": True,
        "store": False,
        "max_tokens": 100,
        "instructions": "You are a helpful assistant."
    }).encode('utf-8')

    headers = {
        'Content-Type': 'application/json',
        'Authorization': 'Bearer chat-test',
        'Accept': 'text/event-stream'
    }
    if email:
        headers["x-selected-account-email"] = email

    req = urllib.request.Request(url, data=payload, headers=headers, method="POST")

    try:
        ph = urllib.request.ProxyHandler({}) # Don't use system proxy for localhost
        opener = urllib.request.build_opener(ph)
        resp = opener.open(req, timeout=30)
        print(f"Status: {resp.getcode()}")
        
        has_content = False
        empty_chunks = 0
        
        for line in resp:
            line_str = line.decode('utf-8').strip()
            if not line_str or line_str == "data: [DONE]":
                continue
            if line_str.startswith("data: "):
                try:
                    data = json.loads(line_str[6:])
                    if 'choices' in data and len(data['choices']) > 0:
                        delta = data['choices'][0].get('delta', {})
                        content = delta.get('content', '')
                        if content:
                            print(f"  Chunk: {repr(content)}")
                            has_content = True
                        else:
                            empty_chunks += 1
                except Exception as ex:
                    print(f"  Parse Error: {ex} - {line_str}")
        
        print(f"\nResult: {'✅ 成功' if has_content else '❌ 失败 (无文本内容)'}, 收到 {empty_chunks} 个空片段")
        if not has_content:
            raise Exception("No content received from Codex Proxy Stream")
        
    except urllib.error.HTTPError as e:
        print(f"❌ HTTPError: {e.code}")
        print(e.read().decode('utf-8')[:300])
        raise
    except Exception as e:
        print(f"❌ Failed: {e}")
        raise

def test_kiro_chat_via_proxy(model="claude-sonnet-4.5", email=None):
    print("\n========================================")
    print(f"4.6 测试向本地代理发 Kiro 对话请求 (/kiro/v1/messages) - {model}")
    print("========================================")
    
    url = "http://127.0.0.1:19531/kiro/v1/messages"
    
    payload = json.dumps({
        "model": model,
        "messages": [{
            "role": "user",
            "content": "Say exactly: 'Hello Amazon'"
        }],
        "stream": True,
        "max_tokens": 100
    }).encode('utf-8')

    headers = {
        'Content-Type': 'application/json',
        'x-api-key': 'chat-test',
        'anthropic-version': '2023-06-01',
        'Accept': 'text/event-stream'
    }
    if email:
        headers["x-selected-account-email"] = email

    req = urllib.request.Request(url, data=payload, headers=headers, method="POST")

    try:
        ph = urllib.request.ProxyHandler({}) # Don't use system proxy for localhost
        opener = urllib.request.build_opener(ph)
        resp = opener.open(req, timeout=30)
        print(f"Status: {resp.getcode()}")
        
        full_text = ""
        has_truncation_warning = False
        has_stop = False
        
        for line in resp:
            line_str = line.decode('utf-8').strip()
            if not line_str:
                continue
                
            if line_str.startswith("event: message_stop"):
                has_stop = True
                print("  Received message_stop")
                
            if line_str.startswith("data: "):
                try:
                    data = json.loads(line_str[6:])
                    if data.get('type') == 'content_block_delta':
                        delta = data.get('delta', {})
                        if delta.get('type') == 'text_delta':
                            text = delta.get('text', '')
                            print(f"  Text: {repr(text)}")
                            full_text += text
                            if "似被截断" in text or "若需接续可回复" in text:
                                has_truncation_warning = True
                except:
                    pass
        
        print("\n=== Kiro 结果 ===")
        print(f"完整回复: {repr(full_text)}")
        
        if has_truncation_warning:
            print("❌ 收到异常截断报警 (回答似被截断...)")
            raise Exception("Kiro stream injected artificial truncation warning")
        elif not has_stop:
            print("❌ 未收到 message_stop")
            raise Exception("Kiro stream missing message_stop event")
        elif not full_text.strip():
            print("❌ 收到空文本")
            raise Exception("Kiro stream returned empty text")
        else:
            print("✅ 成功")
            
    except urllib.error.HTTPError as e:
        print(f"❌ HTTPError: {e.code}")
        print(e.read().decode('utf-8')[:300])
        raise
    except Exception as e:
        print(f"❌ Failed: {e}")
        raise


# 参考 Codex2API DefaultModels (gpt-5.x 系列)
CODEX_DEFAULT_MODELS = [
    "gpt-5.3-codex",
    "gpt-5.4",
    "gpt-5.2-codex",
    "gpt-5.1-codex-max",
    "gpt-5.2",
    "gpt-5.1-codex-mini",
]

# 默认测试模型
CODEX_DEFAULT_TEST_MODEL = "gpt-5.3-codex"

def test_codex_fetch_models():
    """获取 Codex 模型列表 (参考 Codex2API DefaultModels)"""
    print("\n========================================")
    print("3. 测试获取 Codex 模型列表 (参考 Codex2API DefaultModels)")
    print("========================================")
    print(f"模型列表 ({len(CODEX_DEFAULT_MODELS)} 个):")
    for m in CODEX_DEFAULT_MODELS:
        marker = " <-- 默认测试模型" if m == CODEX_DEFAULT_TEST_MODEL else ""
        print(f"  {m}{marker}")
    return CODEX_DEFAULT_MODELS

def test_codex_check_quota(access_token, account_id=None):
    """调用 wham/usage 检查 Codex 账号配额
    
    返回: (has_quota: bool, plan_type: str, detail: str)
    """
    url = "https://chatgpt.com/backend-api/wham/usage"
    headers = {
        'Authorization': f'Bearer {access_token}',
        'Accept': 'application/json',
        'User-Agent': 'codex_cli_rs/0.104.0',
    }
    if account_id:
        headers['ChatGPT-Account-Id'] = account_id

    req = urllib.request.Request(url, headers=headers, method="GET")
    try:
        ph = urllib.request.ProxyHandler()
        opener = urllib.request.build_opener(ph)
        resp = opener.open(req, timeout=15)
        data = json.loads(resp.read().decode('utf-8'))
        
        plan_type = data.get("plan_type", "unknown")
        rate_limit = data.get("rate_limit", {})
        limit_reached = rate_limit.get("limit_reached", False)
        
        primary = rate_limit.get("primary_window", {})
        used_percent = primary.get("used_percent", 0)
        remaining = 100 - (used_percent or 0)
        
        reset_at = primary.get("reset_at")
        reset_str = ""
        if reset_at:
            import datetime
            try:
                dt = datetime.datetime.fromtimestamp(reset_at)
                reset_str = f", 重置时间: {dt.strftime('%m-%d %H:%M')}"
            except Exception:
                reset_str = f", reset_at: {reset_at}"
        
        if limit_reached:
            detail = f"plan={plan_type}, 余额=0% (已耗尽{reset_str})"
            return False, plan_type, detail
        else:
            detail = f"plan={plan_type}, 余额={remaining}%{reset_str}"
            return True, plan_type, detail
            
    except urllib.error.HTTPError as e:
        body = e.read().decode('utf-8')[:200]
        return False, "unknown", f"HTTPError {e.code}: {body}"
    except Exception as e:
        return False, "unknown", f"Error: {e}"

def check_codex_accounts():
    """扫描 Codex 账号，找到第一个有额度的账号即停止
    
    返回: (account_data, email) 或 None
    """
    data_dir = os.path.join(os.getenv("LOCALAPPDATA"), "com.antigravity.cockpit-tools", "codex_accounts")
    print(f"\nLooking for Codex accounts in: {data_dir}")
    if not os.path.exists(data_dir):
        print("Codex account directory does not exist.")
        return None
    
    files = sorted([f for f in os.listdir(data_dir) if f.endswith(".json")])
    print(f"Found {len(files)} Codex account file(s), scanning for available quota...")
    
    checked = 0
    for f in files:
        file_path = os.path.join(data_dir, f)
        try:
            acc = json.load(open(file_path, encoding="utf-8"))
            if "tokens" not in acc or "access_token" not in acc["tokens"]:
                continue
        except Exception:
            continue
        
        checked += 1
        email = acc.get('email', 'unknown')
        token = acc["tokens"]["access_token"]
        account_id = acc.get("account_id", None)
        
        has_quota, plan_type, detail = test_codex_check_quota(token, account_id)
        
        if has_quota:
            print(f"  [OK] {email} ({f}): {detail}  (checked {checked} accounts)")
            return acc, email
        else:
            print(f"  [NO QUOTA] {email} ({f}): {detail}")
    
    print(f"All {checked} accounts checked, none have available quota.")
    return None

def get_codex_account_by_email(email):
    data_dir = os.path.join(os.getenv("LOCALAPPDATA"), "com.antigravity.cockpit-tools", "codex_accounts")
    if not os.path.exists(data_dir): return None
    for f in os.listdir(data_dir):
        if not f.endswith(".json"): continue
        try:
            acc = json.load(open(os.path.join(data_dir, f), encoding="utf-8"))
            if acc.get('email') == email:
                return acc
        except: pass
    return None

def test_proxy_chain():
    print("\n\n========================================")
    print(" === 5. 指定账号 Kiro 代理测试 ===")
    print("========================================")
    try:
        test_kiro_chat_via_proxy()
    except Exception as e:
        print(f"Kiro 测试失败: {e}")

    print("\n\n========================================")
    print(" === 6. 指定账号 Codex 远程测试 ===")
    print("========================================")
    target_email = "aagejegeoeeenenjekwhelwkwkgwke@gmail.com"
    target_acc = get_codex_account_by_email(target_email)
    if target_acc and "tokens" in target_acc and "access_token" in target_acc["tokens"]:
        print(f"找到指定账号: {target_email}, 开始本地代理请求...")
        
        # Test with local codex request
        test_codex_chat_via_proxy(model=CODEX_DEFAULT_TEST_MODEL, email=target_email)
    else:
        print(f"未能找到账号 {target_email} 或缺少 access_token")

if __name__ == "__main__":
    test_proxy_chain()
