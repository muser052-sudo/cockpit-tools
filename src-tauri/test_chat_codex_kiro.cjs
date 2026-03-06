/**
 * 独立 Chat 测试: 向 Codex 和 Kiro 各发送 "hi"，校验返回内容
 * 运行前请先在应用内启动 API 代理（Chat 测试页 -> 启动）
 * 运行: node test_chat_codex_kiro.cjs
 */
const fs = require('fs');
const path = require('path');
const os = require('os');
const http = require('http');

function detectProxyPort() {
    if (process.env.PROXY_PORT) {
        const p = parseInt(process.env.PROXY_PORT, 10);
        if (!isNaN(p)) return p;
    }
    const configPath = path.join(os.homedir(), '.antigravity_cockpit', 'api_proxy_config.json');
    try {
        const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
        return config.port || 19531;
    } catch {
        return 19531;
    }
}

const PROXY_PORT = detectProxyPort();
const API_KEY = 'chat-test';

function httpRequest(url, options, body) {
    return new Promise((resolve, reject) => {
        const urlObj = new URL(url);
        const reqOptions = {
            hostname: urlObj.hostname,
            port: urlObj.port,
            path: urlObj.pathname + urlObj.search,
            method: options.method || 'GET',
            headers: options.headers || {},
            timeout: options.timeout || 45000,
        };
        const req = http.request(reqOptions, (res) => {
            let data = '';
            res.on('data', chunk => data += chunk);
            res.on('end', () => resolve({ status: res.statusCode, body: data }));
        });
        req.on('timeout', () => { req.destroy(); reject(new Error('timeout')); });
        req.on('error', reject);
        if (body) req.write(body);
        req.end();
    });
}

/** 从 Codex SSE 里提取累积文本 (data: {"choices":[{"delta":{"content":"..."}}]}) */
function parseCodexSseContent(body) {
    let text = '';
    const lines = body.split('\n');
    for (const line of lines) {
        if (!line.startsWith('data: ')) continue;
        const data = line.slice(6).trim();
        if (data === '[DONE]') continue;
        try {
            const obj = JSON.parse(data);
            const delta = obj.choices?.[0]?.delta?.content;
            if (typeof delta === 'string') text += delta;
        } catch (_) {}
    }
    return text;
}

/** 从 Kiro SSE 里提取累积文本 (event: content_block_delta, data: {"delta":{"type":"text_delta","text":"..."}}) */
function parseKiroSseContent(body) {
    let text = '';
    const lines = body.split('\n');
    let i = 0;
    while (i < lines.length) {
        const line = lines[i];
        if (line.startsWith('data: ')) {
            const data = line.slice(6).trim();
            try {
                const obj = JSON.parse(data);
                const delta = obj.delta;
                if (delta?.type === 'text_delta' && typeof delta.text === 'string') {
                    text += delta.text;
                }
                if (delta?.type === 'thinking_delta' && typeof delta.thinking === 'string') {
                    text += delta.thinking;
                }
            } catch (_) {}
        }
        i++;
    }
    return text;
}

async function testCodexChatHi() {
    console.log('\n--- Codex Chat 测试: 发送 "hi" ---');
    const url = `http://127.0.0.1:${PROXY_PORT}/codex/v1/chat/completions`;
    const body = JSON.stringify({
        model: 'gpt-5.1-codex',
        stream: true,
        max_tokens: 100,
        messages: [{ role: 'user', content: 'hi' }],
    });
    try {
        const resp = await httpRequest(url, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${API_KEY}`,
            },
            timeout: 120000,
        }, body);

        if (resp.status !== 200) {
            console.log(`❌ Codex 失败: HTTP ${resp.status}`);
            console.log('--- Codex 原始返回 ---\n' + resp.body.slice(0, 800));
            return false;
        }
        const content = parseCodexSseContent(resp.body);
        console.log('--- Codex 原始返回 (前 2000 字符) ---');
        console.log(resp.body.slice(0, 2000));
        if (resp.body.length > 2000) console.log('... [共 ' + resp.body.length + ' 字符，已截断]');
        console.log('--- Codex 解析出的回复内容 ---');
        console.log(content || '(无)');
        console.log('---');
        if (!content || content.length < 1) {
            console.log('❌ Codex 失败: 未解析到返回文本');
            return false;
        }
        console.log('✅ Codex 通过: 收到回复长度', content.length, '字符');
        return true;
    } catch (e) {
        console.log('❌ Codex 请求异常:', e.message);
        console.log('   请确认代理已启动 (端口', PROXY_PORT + ')');
        return false;
    }
}

async function testKiroChatHi() {
    console.log('\n--- Kiro Chat 测试: 发送 "hi" ---');
    const url = `http://127.0.0.1:${PROXY_PORT}/kiro/v1/messages`;
    const body = JSON.stringify({
        model: 'auto',
        max_tokens: 100,
        stream: true,
        messages: [{ role: 'user', content: [{ type: 'text', text: 'hi' }] }],
    });
    try {
        const resp = await httpRequest(url, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'x-api-key': API_KEY,
                'anthropic-version': '2023-06-01',
            },
        }, body);

        if (resp.status !== 200) {
            console.log(`❌ Kiro 失败: HTTP ${resp.status}`);
            console.log('--- Kiro 原始返回 ---\n' + resp.body.slice(0, 800));
            return false;
        }
        const content = parseKiroSseContent(resp.body);
        console.log('--- Kiro 原始返回 (前 1200 字符) ---');
        console.log(resp.body.slice(0, 1200));
        if (resp.body.length > 1200) console.log('... [共 ' + resp.body.length + ' 字符，已截断]');
        console.log('--- Kiro 解析出的回复内容 ---');
        console.log(content || '(无)');
        console.log('---');
        if (!content || content.length < 1) {
            console.log('❌ Kiro 失败: 未解析到返回文本');
            return false;
        }
        console.log('✅ Kiro 通过: 收到回复长度', content.length, '字符');
        return true;
    } catch (e) {
        console.log('❌ Kiro 请求异常:', e.message);
        console.log('   请确认代理已启动 (端口', PROXY_PORT + ')');
        return false;
    }
}

async function main() {
    console.log('========================================');
    console.log('  Chat 独立测试 (Codex + Kiro)');
    console.log('  代理端口:', PROXY_PORT);
    console.log('========================================');

    const codexOk = await testCodexChatHi();
    const kiroOk = await testKiroChatHi();

    console.log('\n========================================');
    console.log('  Codex:', codexOk ? '通过' : '失败');
    console.log('  Kiro: ', kiroOk ? '通过' : '失败');
    console.log('========================================');

    process.exit(codexOk && kiroOk ? 0 : 1);
}

main();
