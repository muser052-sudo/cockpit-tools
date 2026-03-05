import fs from 'fs';
import path from 'path';
import os from 'os';
import { fetch as undiciFetch } from 'undici';

// 如果有本地代理，请取消注释并按需修改
// import { ProxyAgent, setGlobalDispatcher } from 'undici';
// const proxyAgent = new ProxyAgent('http://127.0.0.1:7890');
// setGlobalDispatcher(proxyAgent);

async function testFetchModels() {
    console.log('--- 测试 fetchAvailableModels ---');
    const dataDir = path.join(os.homedir(), '.antigravity_cockpit', 'accounts');
    const files = fs.readdirSync(dataDir).filter(f => f.endsWith('.json'));

    if (files.length === 0) {
        console.log('没有找到账号文件');
        return;
    }

    // 寻找 chenyiding01@gmail.com 账号
    let account = null;
    for (const file of files) {
        const acc = JSON.parse(fs.readFileSync(path.join(dataDir, file), 'utf8'));
        if (acc.email === 'chenyiding01@gmail.com') {
            account = acc;
            break;
        }
    }

    if (!account) {
        console.log('找不到指定的账号 chenyiding01@gmail.com');
        return;
    }

    console.log(`使用账号: ${account.email}`);

    const accessToken = account.token.access_token;
    const projectId = account.token.project_id || '';

    console.log(`access_token length: ${accessToken.length}, project_id: ${projectId}`);

    try {
        const body = projectId ? { project: projectId } : {};
        // 增加 fetch 错误捕捉
        console.log('开始请求 Google endpoint...');
        const response = await undiciFetch('https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels', {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${accessToken}`,
                'Content-Type': 'application/json',
                'User-Agent': 'grpc-java-okhttp/1.68.2'
            },
            body: JSON.stringify(body),
        });

        console.log(`fetchAvailableModels HTTP Status: ${response.status}`);
        const data = await response.json();

        if (response.status !== 200) {
            console.log('Error data:', JSON.stringify(data, null, 2));
        }

        if (data.models) {
            const models = Object.keys(data.models);
            console.log(`\n============================`);
            console.log(`成功获取到 ${models.length} 个模型!`);
            console.log(`前 5 个模型: ${models.slice(0, 5).join(', ')}`);
            console.log(`============================\n`);
            return { account, models };
        } else {
            console.log('响应中没有 models 字段:', data);
        }
    } catch (e) {
        console.error('请求失败:', e);
    }
    return null;
}

async function main() {
    const result = await testFetchModels();

    if (result && result.models.length > 0) {
        const testModel = result.models[0];
        console.log(`\n--- 测试 Chat 代理请求 (model: ${testModel}) ---`);
        try {
            // 请在此处修改你的代理端口（默认可能是19530）
            const response = await undiciFetch('http://127.0.0.1:19530/antigravity/v1/messages', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-api-key': 'chat-test',
                    'anthropic-version': '2023-06-01'
                },
                body: JSON.stringify({
                    model: testModel,
                    max_tokens: 100,
                    stream: true,
                    messages: [{ role: 'user', content: '测试消息，只需回复 hello' }]
                })
            });

            console.log(`Chat 代理 HTTP Status: ${response.status}`);
            if (!response.ok) {
                console.log('Chat 代理返回错误:', await response.text());
                return;
            }

            const reader = response.body.getReader();
            const decoder = new TextDecoder();
            while (true) {
                const { done, value } = await reader.read();
                if (done) break;
                const chunk = decoder.decode(value);
                console.log('STREAM CHUNK:', chunk);
            }
        } catch (e) {
            console.error('代理请求失败:', e);
        }
    }
}

main();
