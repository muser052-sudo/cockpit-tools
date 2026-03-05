const fs = require('fs');
const path = require('path');
const os = require('os');
const http = require('http');

async function testFetchModels() {
    console.log('--- 测试 fetchAvailableModels ---');
    const dataDir = path.join(os.homedir(), '.antigravity_cockpit', 'accounts');
    const files = fs.readdirSync(dataDir).filter(f => f.endsWith('.json'));

    if (files.length === 0) {
        console.log('没有找到账号文件');
        return;
    }

    // 读取第一个账号
    const account = JSON.parse(fs.readFileSync(path.join(dataDir, files[0]), 'utf8'));
    console.log(`使用账号: ${account.email}`);

    const accessToken = account.token.access_token;
    const projectId = account.token.project_id || '';

    console.log(`access_token length: ${accessToken.length}, project_id: ${projectId}`);

    try {
        const body = projectId ? { project: projectId } : {};
        const response = await fetch('https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels', {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${accessToken}`,
                'Content-Type': 'application/json',
                'User-Agent': 'grpc-java-okhttp/1.68.2'
            },
            body: JSON.stringify(body)
        });

        console.log(`HTTP Status: ${response.status}`);
        const data = await response.json();

        if (data.models) {
            const models = Object.keys(data.models);
            console.log(`成功获取到 ${models.length} 个模型:`);
            console.log(models.join(', '));
            return { account, models };
        } else {
            console.log('响应中没有 models 字段:', data);
        }
    } catch (e) {
        console.error('请求失败:', e);
    }
    return null;
}

async function testChatProxy(account, model) {
    console.log(`\n--- 测试 Chat 代理请求 (model: ${model}) ---`);
    try {
        const response = await fetch('http://127.0.0.1:8045/antigravity/v1/messages', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'x-api-key': 'chat-test',
                'anthropic-version': '2023-06-01'
            },
            body: JSON.stringify({
                model: model,
                max_tokens: 100,
                stream: true,
                messages: [{ role: 'user', content: 'hello' }]
            })
        });

        console.log(`HTTP Status: ${response.status}`);
        if (!response.ok) {
            console.log('Error text:', await response.text());
            return;
        }

        // 打印流式响应
        const reader = response.body.getReader();
        const decoder = new TextDecoder();
        while (true) {
            const { done, value } = await reader.read();
            if (done) break;
            const chunk = decoder.decode(value);
            console.log('CHUNK:', chunk);
        }
    } catch (e) {
        console.error('代理请求失败:', e);
    }
}

async function main() {
    const result = await testFetchModels();
    if (result && result.models.length > 0) {
        // 测试第一个可用的模型
        await testChatProxy(result.account, result.models[0]);
    }
}

main();
