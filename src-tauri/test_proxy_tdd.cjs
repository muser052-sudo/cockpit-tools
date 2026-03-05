/**
 * TDD 测试脚本: 验证模型获取 + 对话功能
 * 参考: test_google_upstream.py (能正常工作的 Python 版本)
 * 
 * 关键差异修复:
 * 1. 使用 undici 显式绕过系统代理（Python 用 ProxyHandler() 实现）
 * 2. 端点顺序: prod → sandbox → daily（与 Python 一致）
 * 3. 自动检测代理端口
 * 
 * 运行方式: node test_proxy_tdd.cjs
 */
const fs = require('fs');
const path = require('path');
const os = require('os');
const http = require('http');
const https = require('https');

// 自动检测代理端口
function detectProxyPort() {
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

// 不走系统代理的 HTTPS 请求（对标 Python 的 ProxyHandler()）
function httpsRequest(url, options, body) {
    return new Promise((resolve, reject) => {
        const urlObj = new URL(url);
        const reqOptions = {
            hostname: urlObj.hostname,
            port: 443,
            path: urlObj.pathname + urlObj.search,
            method: options.method || 'GET',
            headers: options.headers || {},
            timeout: options.timeout || 15000,
        };

        const req = https.request(reqOptions, (res) => {
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

// 不走系统代理的 HTTP 请求（用于本地代理）
function httpRequest(url, options, body) {
    return new Promise((resolve, reject) => {
        const urlObj = new URL(url);
        const reqOptions = {
            hostname: urlObj.hostname,
            port: urlObj.port,
            path: urlObj.pathname + urlObj.search,
            method: options.method || 'GET',
            headers: options.headers || {},
            timeout: options.timeout || 30000,
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

// === 测试 1: Antigravity 获取模型（和 Python 测试完全一致）===
async function testAntigravityFetchModels() {
    console.log('\n=== 测试 1: Antigravity 获取模型 ===');
    const dataDir = path.join(os.homedir(), '.antigravity_cockpit', 'accounts');

    if (!fs.existsSync(dataDir)) {
        console.log('❌ 未找到 antigravity 账号目录');
        return null;
    }

    const files = fs.readdirSync(dataDir).filter(f => f.endsWith('.json'));
    if (files.length === 0) {
        console.log('❌ 未找到 antigravity 账号文件');
        return null;
    }

    // 找一个可用账号
    let account = null;
    for (const file of files) {
        const acc = JSON.parse(fs.readFileSync(path.join(dataDir, file), 'utf8'));
        if (!acc.disabled && acc.token?.access_token) {
            account = acc;
            break;
        }
    }
    if (!account) {
        console.log('❌ 没有可用的 antigravity 账号');
        return null;
    }

    console.log(`使用账号: ${account.email}`);
    const accessToken = account.token.access_token;
    const projectId = account.token.project_id || '';

    // 端点顺序和 Python 测试一致: prod → sandbox → daily
    const endpoints = [
        'https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels',
        'https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal:fetchAvailableModels',
        'https://daily-cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels',
    ];

    const headers = {
        'User-Agent': 'antigravity/2.15.8 (Windows; AMD64)',
        'Authorization': `Bearer ${accessToken}`,
        'Content-Type': 'application/json',
    };

    const body = JSON.stringify({});

    for (const url of endpoints) {
        const host = new URL(url).hostname;
        console.log(`\nTrying URL: ${url}`);
        try {
            const resp = await httpsRequest(url, {
                method: 'POST',
                headers,
                timeout: 15000,
            }, body);

            console.log(`Status: ${resp.status}`);
            if (resp.status === 200) {
                const data = JSON.parse(resp.body);
                if (data.models) {
                    // 过滤非模型名称
                    const validPrefixes = ['gemini-', 'claude-', 'gpt-', 'o1', 'o3', 'o4', 'tab_'];
                    const allKeys = Object.keys(data.models);
                    const models = allKeys.filter(k => validPrefixes.some(p => k.startsWith(p)));
                    const filtered = allKeys.length - models.length;

                    console.log(`✅ 成功获取到 ${models.length} 个模型 (过滤了 ${filtered} 个非模型 key)`);
                    for (const m of models) {
                        const qi = data.models[m]?.quotaInfo || {};
                        const rem = qi.remainingFraction !== undefined
                            ? `${(qi.remainingFraction * 100).toFixed(0)}%` : '无';
                        console.log(`  ${m}: 余额=${rem}`);
                    }
                    return { account, models };
                }
            } else {
                console.log(`  HTTP ${resp.status}: ${resp.body.substring(0, 200)}`);
            }
        } catch (e) {
            console.log(`  Failed: ${e.message}`);
        }
    }

    console.log('❌ 所有端点均失败');
    return null;
}

// === 测试 2: Antigravity 代理对话 ===
async function testAntigravityChat(models) {
    console.log('\n=== 测试 2: Antigravity 代理对话 ===');
    const testModel = models.includes('gemini-2.5-flash') ? 'gemini-2.5-flash' : models[0];
    console.log(`使用模型: ${testModel}, 代理: http://127.0.0.1:${PROXY_PORT}`);

    try {
        const resp = await httpRequest(
            `http://127.0.0.1:${PROXY_PORT}/antigravity/v1/messages`,
            {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-api-key': API_KEY,
                    'anthropic-version': '2023-06-01',
                },
                timeout: 30000,
            },
            JSON.stringify({
                model: testModel,
                max_tokens: 50,
                stream: true,
                messages: [{ role: 'user', content: 'Say pong' }],
            })
        );

        console.log(`HTTP Status: ${resp.status}`);
        if (resp.status >= 200 && resp.status < 300) {
            console.log('✅ 对话成功，收到响应 (' + resp.body.length + ' 字节)');
            console.log('  预览:', resp.body.substring(0, 200));
        } else {
            console.log('❌ 对话失败:', resp.body.substring(0, 300));
        }
    } catch (e) {
        console.log('❌ 代理请求失败:', e.message);
        console.log('  请确保代理已启动在端口', PROXY_PORT);
    }
}

// === 测试 3: Codex 对话 ===
async function testCodexChat() {
    console.log('\n=== 测试 3: Codex 代理对话 ===');
    const testModel = 'gpt-4o';
    console.log(`使用模型: ${testModel}, 代理: http://127.0.0.1:${PROXY_PORT}`);

    try {
        const resp = await httpRequest(
            `http://127.0.0.1:${PROXY_PORT}/codex/v1/chat/completions`,
            {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'Authorization': `Bearer ${API_KEY}`,
                },
                timeout: 30000,
            },
            JSON.stringify({
                model: testModel,
                max_tokens: 50,
                stream: true,
                messages: [{ role: 'user', content: 'Say pong' }],
            })
        );

        console.log(`HTTP Status: ${resp.status}`);
        if (resp.status >= 200 && resp.status < 300) {
            console.log('✅ 对话成功，收到响应 (' + resp.body.length + ' 字节)');
            console.log('  预览:', resp.body.substring(0, 200));
        } else if (resp.status === 429) {
            console.log('⚠️ Codex 频率限制 (429)');
        } else {
            console.log('❌ 对话失败:', resp.body.substring(0, 300));
        }
    } catch (e) {
        console.log('❌ 代理请求失败:', e.message);
        console.log('  请确保代理已启动在端口', PROXY_PORT);
    }
}

async function main() {
    console.log('========================================');
    console.log('  TDD 测试: 模型获取 + 对话');
    console.log(`  代理端口: ${PROXY_PORT}`);
    console.log('========================================');

    const result = await testAntigravityFetchModels();

    if (result && result.models.length > 0) {
        await testAntigravityChat(result.models);
    }

    await testCodexChat();

    console.log('\n========================================');
    console.log('  测试完成');
    console.log('========================================');
}

main();
