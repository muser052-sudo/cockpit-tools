import os
import json
import urllib.request
import urllib.error
import glob

def test_proxy_fetch_models():
    print("--- 测试 gcli2api 的 fetch /v1/models ---")

    req = urllib.request.Request(
        "http://127.0.0.1:19530/antigravity/v1/models",
        headers={
            "Authorization": "Bearer chat-test",
            "Content-Type": "application/json",
            "x-api-key": "chat-test",     
        },
        method="GET"
    )
    
    try:
        # 本地请求不使用代理
        proxy_handler = urllib.request.ProxyHandler({})
        opener = urllib.request.build_opener(proxy_handler)
        response = opener.open(req, timeout=15)
        
        print(f"v1/models HTTP Status: {response.getcode()}")
        resp_data = json.loads(response.read().decode('utf-8'))
        
        models = [m["id"] for m in resp_data.get("data", [])]
        if models:
            print(f"\n============================")
            print(f"成功通过代理获取到 {len(models)} 个模型!")
            print(f"前 5 个模型: {', '.join(models[:5])}")
            print(f"============================\n")
            return models
        else:
            print("响应中没有有效数据:", resp_data)
            
    except urllib.error.HTTPError as e:
        print(f"HTTPError: {e.code}")
        print(e.read().decode('utf-8'))
    except Exception as e:
        print(f"请求失败: {e}")
        
    return None

if __name__ == "__main__":
    test_proxy_fetch_models()
