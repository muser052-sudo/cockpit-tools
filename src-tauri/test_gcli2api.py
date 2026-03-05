import urllib.request
import json

def test_gcli2api_models():
    print("========================================")
    print("测试 gcli2api 获取模型列表 (/antigravity/v1/models)")
    print("========================================")
    req = urllib.request.Request(
        "http://127.0.0.1:19530/antigravity/v1/models",
        headers={"Authorization": "Bearer chat-test", "x-api-key": "chat-test"}
    )
    try:
        ph = urllib.request.ProxyHandler({})
        opener = urllib.request.build_opener(ph)
        resp = opener.open(req, timeout=10)
        print(f"Status: {resp.getcode()}")
        data = json.loads(resp.read().decode('utf-8'))
        models = [m['id'] for m in data.get('data', [])]
        print(f"获取到 {len(models)} 个模型:")
        print(models[:10], "...等")
        return models
    except urllib.error.HTTPError as e:
        print(f"HTTPError: {e.code}")
        print(e.read().decode('utf-8'))
    except Exception as e:
        print(f"Failed models fetch: {e}")
        return []

def test_gcli2api_chat_openai(model):
    print("\n========================================")
    print(f"测试 gcli2api OpenAI 格式对话 (/antigravity/v1/chat/completions) - {model}")
    print("========================================")
    body = json.dumps({
        "model": model,
        "messages": [{"role": "user", "content": "hello，请回答一个字：在。"}],
        "stream": True
    }).encode('utf-8')
    req = urllib.request.Request(
        "http://127.0.0.1:19530/antigravity/v1/chat/completions",
        data=body,
        headers={"Authorization": "Bearer chat-test", "Content-Type": "application/json"}
    )
    try:
        ph = urllib.request.ProxyHandler({})
        opener = urllib.request.build_opener(ph)
        resp = opener.open(req, timeout=20)
        print(f"Status: {resp.getcode()}")
        for line in resp:
            line = line.decode('utf-8').strip()
            if line:
                print("STREAM:", line)
    except urllib.error.HTTPError as e:
        print(f"HTTPError: {e.code}")
        print(e.read().decode('utf-8'))
    except Exception as e:
        print(f"Failed OpenAI chat: {e}")

def test_gcli2api_chat_claude(model):
    print("\n========================================")
    print(f"测试 gcli2api Claude 格式对话 (/antigravity/v1/messages) - {model}")
    print("========================================")
    body = json.dumps({
        "model": model,
        "max_tokens": 100,
        "messages": [{"role": "user", "content": "hello，请回答一个字：在。"}],
        "stream": True
    }).encode('utf-8')
    req = urllib.request.Request(
        "http://127.0.0.1:19530/antigravity/v1/messages",
        data=body,
        headers={"x-api-key": "chat-test", "anthropic-version": "2023-06-01", "Content-Type": "application/json"}
    )
    try:
        ph = urllib.request.ProxyHandler({})
        opener = urllib.request.build_opener(ph)
        resp = opener.open(req, timeout=20)
        print(f"Status: {resp.getcode()}")
        for line in resp:
            line = line.decode('utf-8').strip()
            if line:
                print("STREAM:", line)
    except urllib.error.HTTPError as e:
        print(f"HTTPError: {e.code}")
        print(e.read().decode('utf-8'))
    except Exception as e:
        print(f"Failed Claude chat: {e}")

if __name__ == "__main__":
    models = test_gcli2api_models()
    test_model = models[0] if (models and len(models) > 0) else 'gemini-2.5-flash'
    test_gcli2api_chat_openai(test_model)
    test_gcli2api_chat_claude(test_model)
