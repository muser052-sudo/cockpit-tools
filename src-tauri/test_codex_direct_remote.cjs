/**
 * 直接向远程 Codex 服务器发送 "hi"，打印完整返回
 * 自动使用当前系统代理（Windows 从 Internet 选项读取，或环境变量 HTTPS_PROXY）
 * 运行: node test_codex_direct_remote.cjs
 */
const fs = require('fs');
const path = require('path');
const os = require('os');
const https = require('https');
const { execSync } = require('child_process');
const { HttpsProxyAgent } = require('https-proxy-agent');

function getCodexAccountsDir() {
    const base = process.env.LOCALAPPDATA ||
        (os.platform() === 'darwin' ? path.join(os.homedir(), 'Library', 'Application Support') : path.join(os.homedir(), '.local', 'share'));
    return path.join(base, 'com.antigravity.cockpit-tools', 'codex_accounts');
}

function loadFirstCodexAccount() {
    const dir = getCodexAccountsDir();
    if (!fs.existsSync(dir)) return null;
    const files = fs.readdirSync(dir).filter(f => f.endsWith('.json'));
    for (const f of files) {
        try {
            const acc = JSON.parse(fs.readFileSync(path.join(dir, f), 'utf8'));
            if (acc.tokens?.access_token) {
                return {
                    access_token: acc.tokens.access_token,
                    account_id: acc.account_id || null,
                };
            }
        } catch (_) {}
    }
    return null;
}

/** 优先环境变量，否则 Windows 下读取系统代理（Internet 选项） */
function getProxyUrl() {
    const envUrl = process.env.HTTPS_PROXY || process.env.https_proxy ||
        process.env.HTTP_PROXY || process.env.http_proxy ||
        process.env.ALL_PROXY || process.env.all_proxy;
    if (envUrl && envUrl.trim()) return envUrl.trim();

    if (os.platform() === 'win32') {
        try {
            const key = 'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings';
            const enableOut = execSync(`reg query "${key}" /v ProxyEnable 2>nul`, { encoding: 'utf8', timeout: 2000 });
            if (!/REG_DWORD\s+0x1\b/.test(enableOut)) return null;
            const serverOut = execSync(`reg query "${key}" /v ProxyServer 2>nul`, { encoding: 'utf8', timeout: 2000 });
            const m = serverOut.match(/REG_SZ\s+(.+)/);
            if (!m) return null;
            const s = m[1].trim();
            if (!s) return null;
            return s.includes('://') ? s : `http://${s}`;
        } catch (_) {
            return null;
        }
    }
    return null;
}

function httpsRequestWithProxy(url, options, body, proxyUrl) {
    return new Promise((resolve, reject) => {
        const u = new URL(url);
        const reqOpt = {
            hostname: u.hostname,
            port: 443,
            path: u.pathname + u.search,
            method: options.method || 'GET',
            headers: options.headers || {},
            timeout: options.timeout || 60000,
        };
        if (proxyUrl) {
            reqOpt.agent = new HttpsProxyAgent(proxyUrl);
        }
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
    console.log('========================================');
    console.log('  直接向远程 Codex 发送 "hi"');
    console.log('========================================\n');

    const url = 'https://chatgpt.com/backend-api/codex/responses';
    const acc = loadFirstCodexAccount();
    const access_token = acc ? acc.access_token : 'placeholder-no-account';
    const account_id = acc ? acc.account_id : null;
    const proxyUrl = getProxyUrl();

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
        'Origin': 'https://chatgpt.com',
        'Referer': 'https://chatgpt.com/',
        'originator': 'codex_cli_rs',
        'Accept': 'text/event-stream',
    };
    if (account_id) headers['ChatGPT-Account-Id'] = account_id;

    console.log('请求 URL:', url);
    console.log('账号:', acc ? '本地已保存' : '无(占位)');
    console.log('代理:', proxyUrl || '未设置(直连)');
    console.log('');

    try {
        const resp = await httpsRequestWithProxy(url, { method: 'POST', headers }, payload, proxyUrl);
        console.log('HTTP 状态:', resp.status);
        console.log('\n--- Codex 服务器原始返回 (前 3000 字符) ---\n');
        console.log(resp.body.slice(0, 3000));
        if (resp.body.length > 3000) {
            console.log('\n... [共 ' + resp.body.length + ' 字符，已截断]');
        }
        console.log('\n--- 解析出的 SSE 文本内容 ---');
        const lines = resp.body.split('\n');
        let text = '';
        for (const line of lines) {
            if (!line.startsWith('data: ')) continue;
            const data = line.slice(6).trim();
            if (data === '[DONE]') continue;
            try {
                const obj = JSON.parse(data);
                const delta = obj.delta?.text || obj.delta?.content ||
                    obj.choices?.[0]?.delta?.content;
                if (typeof delta === 'string') text += delta;
            } catch (_) {}
        }
        console.log(text || '(无)');
        console.log('\n========================================');
        return resp;
    } catch (e) {
        console.log('请求异常:', e.message);
        console.log('\n若需走代理，请设置环境变量后重试，例如:');
        console.log('  set HTTPS_PROXY=http://127.0.0.1:7890');
        console.log('  node test_codex_direct_remote.cjs');
        console.log('\n========================================');
        return null;
    }
}

async function testCodexModels() {
    console.log('\n========================================');
    console.log('  GET Codex 模型列表 /backend-api/codex/models');
    console.log('========================================\n');
    const acc = loadFirstCodexAccount();
    const access_token = acc ? acc.access_token : 'placeholder';
    const account_id = acc ? acc.account_id : null;
    const proxyUrl = getProxyUrl();
    const url = 'https://chatgpt.com/backend-api/codex/models';
    const headers = {
        'Authorization': `Bearer ${access_token}`,
        'User-Agent': 'codex_cli_rs/0.104.0',
        'Origin': 'https://chatgpt.com',
        'Referer': 'https://chatgpt.com/',
        'originator': 'codex_cli_rs',
        'Accept': 'application/json',
    };
    if (account_id) headers['ChatGPT-Account-Id'] = account_id;
    try {
        const resp = await httpsRequestWithProxy(url, { method: 'GET', headers }, null, proxyUrl);
        console.log('HTTP 状态:', resp.status);
        console.log('--- 原始 body (前 2000 字符) ---');
        console.log(resp.body.slice(0, 2000));
        if (resp.body.length > 2000) console.log('... [共 ' + resp.body.length + ' 字符]');
        if (resp.status === 200 && resp.body) {
            const j = JSON.parse(resp.body);
            const list = j?.data ?? j?.result?.data ?? j?.models ?? [];
            const ids = Array.isArray(list) ? list.map(m => m?.model ?? m?.id ?? m).filter(Boolean) : [];
            console.log('--- 解析出的模型 id 列表 ---');
            console.log(ids.length ? ids : '(空或结构不同)');
        }
    } catch (e) {
        console.log('请求异常:', e.message);
    }
    console.log('\n========================================');
}

(async () => {
    await testCodexDirect();
    await testCodexModels();
})();
