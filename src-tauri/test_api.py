import os
import json
import urllib.request
import urllib.error
import glob

def test_fetch_models():
    print("--- 测试 fetchAvailableModels ---")
    data_dir = os.path.join(os.path.expanduser("~"), ".antigravity_cockpit", "accounts")
    files = glob.glob(os.path.join(data_dir, "*.json"))
    
    if not files:
        print("没有找到账号文件")
        return None
        
    # 寻找 chenyiding01@gmail.com 账号
    account = None
    for f in files:
        try:
            with open(f, 'r', encoding='utf-8') as file:
                acc = json.load(file)
                if acc.get("email") == "chenyiding01@gmail.com":
                    account = acc
                    break
        except Exception:
            pass
            
    if not account:
        print("找不到指定的账号 chenyiding01@gmail.com")
        return None
        
    print(f"使用账号: {account['email']}")
    
    access_token = account.get("token", {}).get("access_token")
    project_id = account.get("token", {}).get("project_id", "")
    
    body = {}
    data = json.dumps(body).encode('utf-8')
    
    req = urllib.request.Request(
        "https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels",
        data=data,
        headers={
            "Authorization": f"Bearer {access_token}",
            "Content-Type": "application/json",
            "User-Agent": "grpc-java-okhttp/1.68.2"
        },
        method="POST"
    )
    
    try:
        # 使用系统默认代理
        proxy_handler = urllib.request.ProxyHandler()
        opener = urllib.request.build_opener(proxy_handler)
        response = opener.open(req, timeout=15)
        
        print(f"fetchAvailableModels HTTP Status: {response.getcode()}")
        resp_data = json.loads(response.read().decode('utf-8'))
        
        models = list(resp_data.get("models", {}).keys())
        if models:
            print(f"\n============================")
            print(f"成功获取到 {len(models)} 个模型!")
            print(f"前 5 个模型: {', '.join(models[:5])}")
            print(f"============================\n")
            return {"account": account, "models": models}
        else:
            print("响应中没有 models 字段:", resp_data)
            
    except urllib.error.HTTPError as e:
        print(f"HTTPError: {e.code}")
        print(e.read().decode('utf-8'))
    except Exception as e:
        print(f"请求失败: {e}")
        
    return None

def test_chat_proxy(model):
    print(f"\n--- 测试 Chat 代理请求 (model: {model}) ---")
    data = json.dumps({
        "model": model,
        "max_tokens": 100,
        "stream": True,
        "messages": [{"role": "user", "content": "测试消息，只需回复 hello"}]
    }).encode('utf-8')
    
    # 获取 IDE 设置中当前的代理端口，这里直接盲猜默认的 19530 或 8045，用户截图里有 19530
    req = urllib.request.Request(
        "http://127.0.0.1:19530/antigravity/v1/messages",
        data=data,
        headers={
            "Content-Type": "application/json",
            "x-api-key": "chat-test",
            "anthropic-version": "2023-06-01"
        },
        method="POST"
    )
    
    try:
        # 本地请求不使用代理
        proxy_handler = urllib.request.ProxyHandler({})
        opener = urllib.request.build_opener(proxy_handler)
        response = opener.open(req, timeout=15)
        
        print(f"Chat 代理 HTTP Status: {response.getcode()}")
        for line in response:
            line_str = line.decode('utf-8').strip()
            if line_str:
                print(f"STREAM CHUNK: {line_str}")
    except urllib.error.HTTPError as e:
        print(f"HTTPError: {e.code}")
        print(e.read().decode('utf-8'))
    except Exception as e:
        print(f"请求失败: {e}")

if __name__ == "__main__":
    result = test_fetch_models()
    if result and result["models"]:
        test_chat_proxy(result["models"][0])
