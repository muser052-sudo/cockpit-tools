/**
 * 直接向远程 Codex / Kiro 服务器发送 "hi"，打印各自返回（不经过本地代理）
 * 会从本机已保存的账号读取 token；若无账号则用占位 token 仅看服务器响应
 * 运行: node test_chat_direct_remote.cjs
 */
const fs = require('fs');
const path = require('path');
const os = require('os');
const https = require('https');

function getCodexAccountsDir() {
    const base = process.env.LOCALAPPDATA || (os.platform() === 'darwin' ? path.join(os.homedir(), 'Library', 'Application Support') : path.join(os.homedir(), '.local', 'share'));
    return path.join(base, 'com.antigravity.cockpit-tools', 'codex_accounts');
}

function getKiroAccountsDir() {
    return path.join(os.homedir(), '.antigravity_cockpit', 'kiro_accounts');
}

function loadFirstCodexAccount() {
    const dir = getCodexAccountsDir();
    if (!fs.existsSync(dir)) return null;
    const files = fs.readdirSync(dir).filter(f => f.endsWith('.json'));
    for (const f of files) {
        try {
            const acc = JSON.parse(fs.readFileSync(path.join(dir, f), 'utf8'));
            if (acc.tokens?.access_token) {
                return { access_token: acc.tokens.access_token, account_id: acc.account_id || null };
            }
        } catch (_) {}
    }
    return null;
}

function loadFirstKiroAccount() {
    const dir = getKiroAccountsDir();
    if (!fs.existsSync(dir)) return null;
    const files = fs.readdirSync(dir).filter(f => f.endsWith('.json'));
    for (const f of files) {
        try {
            const acc = JSON.parse(fs.readFileSync(path.join(dir, f), 'utf8'));
            if (acc.access_token) {
                const profileArn = (acc.kiro_auth_token_raw && acc.kiro_auth_token_raw.profileArn) || (acc.kiro_profile_raw && acc.kiro_profile_raw.profileArn) || null;
                return { access_token: acc.access_token, profile_arn: profileArn };
            }
        } catch (_) {}
    }
    return null;
}

function httpsRequest(url, options, body) {
    return new Promise((resolve, reject) => {
        const u = new URL(url);
        const reqOpt = {
            hostname: u.hostname,
            port: 443,
            path: u.pathname + u.search,
            method: options.method || 'GET',
            headers: options.headers || {},
            timeout: options.timeout || 35000,
        };
        const req = https.request(reqOpt, (res) => {
            let data = '';
            res.on('data', chunk => data += chunk);
            res.on('end', () => resolve({ status: res.statusCode, headers: res.headers, body: data }));
        });
        req.on('timeout', () => { req.destroy(); reject(new Error('timeout')); });
        req.on('error', reject);
        if (body) req.write(body);
        req.end();
    });
}

async function testCodexDirect() {
    console.log('\n========== Codex 远程直连 ==========');
    const url = 'https://chatgpt.com/backend-api/codex/responses';
    const acc = loadFirstCodexAccount();
    const access_token = acc ? acc.access_token : 'placeholder-no-account';
    const account_id = acc ? acc.account_id : null;

    const payload = JSON.stringify({
        model: 'gpt-5.1-codex',
        input: [{ role: 'user', content: [{ type: 'input_text', text: 'hi' }] }],
        stream: true,
        store: false,
        instructions: 'You are a helpful assistant.',
    });

    const headers = {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${access_token}`,
        'User-Agent': 'codex_cli_rs/0.104.0',
        'Accept': 'text/event-stream',
    };
    if (account_id) headers['ChatGPT-Account-Id'] = account_id;

    console.log('请求 URL:', url);
    console.log('使用账号:', acc ? '本地已保存' : '无(占位)，仅看服务器返回)');
    try {
        const resp = await httpsRequest(url, { method: 'POST', headers }, payload);
        console.log('HTTP 状态:', resp.status);
        console.log('--- Codex 服务器返回 (前 2000 字符) ---');
        console.log(resp.body.slice(0, 2000));
        if (resp.body.length > 2000) console.log('... [共 ' + resp.body.length + ' 字符]');
        console.log('---');
        return resp;
    } catch (e) {
        console.log('请求异常:', e.message);
        return null;
    }
}

async function testKiroDirect() {
    console.log('\n========== Kiro 远程直连 ==========');
    const endpoint = 'https://q.us-east-1.amazonaws.com';
    const url = endpoint + '/generateAssistantResponse';
    const acc = loadFirstKiroAccount();
    const access_token = acc ? acc.access_token : 'placeholder-no-account';

    const conversationId = 'conv_' + Date.now();
    const payload = JSON.stringify({
        conversationState: {
            chatTriggerType: 'MANUAL',
            conversationId,
            currentMessage: {
                userInputMessage: {
                    content: 'hi',
                    modelId: 'auto',
                    origin: 'AI_EDITOR',
                },
            },
        },
        ...(acc && acc.profile_arn ? { profileArn: acc.profile_arn } : {}),
    });

    const headers = {
        'Content-Type': 'application/x-amzn-json-1.0',
        'Accept': 'application/json',
        'Authorization': `Bearer ${access_token}`,
        'X-Amz-Target': 'AmazonCodeWhispererStreamingService.GenerateAssistantResponse',
        'amz-sdk-request': 'attempt=1; max=3',
        'x-amzn-kiro-agent-mode': 'vibe',
        'x-amz-user-agent': 'aws-sdk-js/1.0.27 KiroIDE-0.7.45-fetch',
        'User-Agent': 'aws-sdk-js/1.0.27 ua/2.1 os/win32#10.0.19044 lang/js md/nodejs#22.21.1 api/codewhispererstreaming#1.0.27',
    };

    console.log('请求 URL:', url);
    console.log('使用账号:', acc ? '本地已保存' : '无(占位)，仅看服务器返回)');
    try {
        const resp = await httpsRequest(url, { method: 'POST', headers }, payload);
        console.log('HTTP 状态:', resp.status);
        console.log('--- Kiro 服务器原始返回 (前 2000 字符) ---');
        console.log(resp.body.slice(0, 2000));
        if (resp.body.length > 2000) console.log('... [共 ' + resp.body.length + ' 字符]');
        // 解析 event 中的 content 拼成一句
        const contentMatches = resp.body.matchAll(/"content"\s*:\s*"([^"]*)"/g);
        const parts = [];
        for (const m of contentMatches) parts.push(m[1]);
        if (parts.length) {
            console.log('--- Kiro 解析出的回复内容 ---');
            console.log(parts.join(''));
            console.log('---');
        }
        return resp;
    } catch (e) {
        console.log('请求异常:', e.message);
        return null;
    }
}

async function main() {
    console.log('========================================');
    console.log('  直接请求远程 Codex / Kiro 服务器');
    console.log('  发送内容: "hi"');
    console.log('========================================');

    await testCodexDirect();
    await testKiroDirect();

    console.log('\n========================================');
    console.log('  结束');
    console.log('========================================');
}

main();
