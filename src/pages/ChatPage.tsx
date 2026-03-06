import { useState, useRef, useEffect, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Send, Trash2, Loader, Server, ServerOff, PlayCircle, StopCircle, X, Settings, Keyboard, Image as ImageIcon } from 'lucide-react';
import './ChatPage.css';

/** 模型配额状态 */
interface QuotaModelInfo {
    id: string;
    remainingFraction?: number;
    resetTime?: string;
}

/** API 代理状态 */
interface ApiProxyStatus {
    running: boolean;
    port: number;
    actual_port: number | null;
    enabled_providers: string[];
}

interface ChatMessage {
    id: string;
    role: 'user' | 'assistant';
    content: string;
    timestamp: number;
    provider?: string;
    streaming?: boolean;
    thinking?: string;
    images?: { data: string; mime_type: string }[];
}

const PROVIDER_OPTIONS = [
    { value: 'antigravity', label: 'Antigravity (Gemini)', defaultModel: 'gemini-2.5-flash' },
    { value: 'codex', label: 'Codex (OpenAI)', defaultModel: 'gpt-5.1-codex' },
    { value: 'kiro', label: 'Kiro (Amazon Q)', defaultModel: 'claude-sonnet-4.5' },
    { value: 'windsurf', label: 'Windsurf (Copilot)', defaultModel: 'claude-sonnet-4-20250514' },
    { value: 'warp', label: 'Warp (Multi-Agent)', defaultModel: 'gpt-5' },
];

export function ChatPage() {
    const [messages, setMessages] = useState<ChatMessage[]>([]);
    const [input, setInput] = useState('');
    const [provider, setProvider] = useState('antigravity');
    const [model, setModel] = useState('gemini-2.5-flash');
    const [streaming, setStreaming] = useState(false);
    const [proxyStatus, setProxyStatus] = useState<ApiProxyStatus | null>(null);
    const [proxyPort, setProxyPort] = useState<number | null>(null);
    const [showStartModal, setShowStartModal] = useState(false);
    const [proxyStarting, setProxyStarting] = useState(false);
    const [proxyConfig, setProxyConfig] = useState<{
        port: number;
        api_key: string;
        request_timeout: number;
        warp_api_url: string;
    }>({ port: 19531, api_key: 'chat-test', request_timeout: 120, warp_api_url: 'http://127.0.0.1:8010' });
    const [providerEnabled, setProviderEnabled] = useState<{ antigravity: boolean; codex: boolean; kiro: boolean; windsurf: boolean; warp: boolean; }>({
        antigravity: true,
        codex: true,
        kiro: true,
        windsurf: true,
        warp: true,
    });
    const [showShortcuts, setShowShortcuts] = useState(false);
    const [selectedAccountEmail, setSelectedAccountEmail] = useState('');
    const [modelsLoading, setModelsLoading] = useState(false);

    // Antigravity 状态
    const [allAntigravityAccounts, setAllAntigravityAccounts] = useState<{ email: string; id: string; models: QuotaModelInfo[] }[]>([]);

    // Codex 状态
    const [allCodexAccounts, setAllCodexAccounts] = useState<{ email: string; id: string; quota?: { hourly_percentage: number }; plan_type?: string }[]>([]);
    const [codexModelsList, setCodexModelsList] = useState<string[]>([]);

    // Kiro 状态
    const [allKiroAccounts, setAllKiroAccounts] = useState<{ email: string; id: string; plan_name?: string }[]>([]);
    const [kiroModelsList, setKiroModelsList] = useState<string[]>(['auto', 'claude-sonnet-4.5', 'claude-sonnet-4', 'claude-haiku-4.5', 'deepseek-3.2', 'minimax-m2.1', 'qwen3-coder-next']);

    // Windsurf 状态
    const [allWindsurfAccounts, setAllWindsurfAccounts] = useState<{ github_login: string; id: string; copilot_plan?: string }[]>([]);
    const [windsurfModelsList] = useState<string[]>(['claude-sonnet-4-20250514', 'gpt-4o', 'o3', 'claude-3.5-sonnet']);

    // Warp 状态
    const [warpModelsList] = useState<string[]>(['gpt-5', 'claude-4-sonnet', 'claude-4.1-opus', 'o3', 'o4-mini', 'gpt-4o', 'gemini-2.5-pro']);

    const [attachedImages, setAttachedImages] = useState<{ data: string; mime_type: string }[]>([]);
    const messagesEndRef = useRef<HTMLDivElement>(null);
    const textareaRef = useRef<HTMLTextAreaElement>(null);
    const fileInputRef = useRef<HTMLInputElement>(null);
    const abortControllerRef = useRef<AbortController | null>(null);

    // 加载代理状态与配置（端口用于请求与展示，配置端口在 actual_port 未同步时作为回退）
    useEffect(() => {
        const checkStatus = async () => {
            try {
                const [status, config] = await Promise.all([
                    invoke<ApiProxyStatus>('get_api_proxy_status'),
                    invoke<any>('get_api_proxy_config').catch(() => null),
                ]);
                setProxyStatus(status);
                setProxyPort(status.actual_port);
                if (config && !proxyConfig) {
                    setProxyConfig({
                        port: config.port ?? 19531,
                        api_key: config.api_key ?? 'chat-test',
                        request_timeout: config.request_timeout ?? 120,
                        warp_api_url: config.warp_api_url ?? 'http://127.0.0.1:8010',
                    });
                }
            } catch (err) {
                console.error('获取代理状态失败:', err);
            }
        };
        checkStatus();
        const timer = setInterval(checkStatus, 3000);
        return () => clearInterval(timer);
    }, []);

    // 自动滚动到底部
    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [messages]);

    // 数据加载与初始化
    useEffect(() => {
        let mounted = true;

        const loadData = async () => {
            setModelsLoading(true);

            // 1. 加载 Antigravity
            try {
                const accs = await invoke<any[]>('list_accounts');
                const validAccs = (accs || []).filter((a: any) => !a.disabled && a.token?.access_token);

                const fetchPromises = validAccs.map(async (acc: any) => {
                    try {
                        const models = await invoke<QuotaModelInfo[]>('fetch_models_for_account', { email: acc.email });
                        return { email: acc.email, id: acc.id, models };
                    } catch (e) {
                        return null;
                    }
                });

                const results = await Promise.all(fetchPromises);
                if (mounted) {
                    setAllAntigravityAccounts(
                        results.filter((res): res is { email: string; id: string; models: QuotaModelInfo[] } => res !== null)
                    );
                }
            } catch (err) {
                console.warn('Load Antigravity failed:', err);
            }

            // 2. 加载 Codex 账号
            try {
                const codexAccs = await invoke<any[]>('list_codex_accounts');
                if (mounted) {
                    setAllCodexAccounts(codexAccs || []);
                }
            } catch (err) {
                console.warn('Load Codex accounts failed:', err);
            }

            // 3. 加载 Codex 模型列表
            try {
                const models = await invoke<string[]>('fetch_codex_models');
                if (mounted) {
                    setCodexModelsList(models);
                }
            } catch (err) {
                console.warn('Load Codex models failed:', err);
            }

            // 4. 加载 Kiro 账号以及动态拉取可用模型
            try {
                const kiroAccs = await invoke<any[]>('list_kiro_accounts');
                if (mounted) {
                    setAllKiroAccounts(kiroAccs || []);

                    // 找到了未被禁用的账号就可以拉取模型列表
                    const activeKiroAcc = (kiroAccs || []).find((a: any) => {
                        const status = a.status?.toLowerCase();
                        const isBanned = status === 'banned' || status === 'ban' || status === 'forbidden';
                        return !isBanned;
                    });

                    if (activeKiroAcc) {
                        try {
                            const kiroModels = await invoke<string[]>('fetch_kiro_models', { email: activeKiroAcc.email });
                            if (mounted && kiroModels && kiroModels.length > 0) {
                                setKiroModelsList(kiroModels);
                            }
                        } catch (e) {
                            console.warn('Failed to fetch real remote Kiro models:', e);
                        }
                    }
                }
            } catch (err) {
                console.warn('Load Kiro accounts failed:', err);
            }

            // 5. 加载 Windsurf 账号
            try {
                const windsurfAccs = await invoke<any[]>('list_windsurf_accounts');
                if (mounted) {
                    setAllWindsurfAccounts(windsurfAccs || []);
                }
            } catch (err) {
                console.warn('Load Windsurf accounts failed:', err);
            }

            if (mounted) {
                setModelsLoading(false);
            }
        };

        loadData();

        return () => { mounted = false; };
    }, []);

    // 派生可用模型列表（去重 + 合并额度）
    const availableModels = useMemo(() => {
        if (provider === 'antigravity') {
            const modelMap = new Map<string, QuotaModelInfo>();
            allAntigravityAccounts.forEach(acc => {
                acc.models.forEach(m => {
                    if (!modelMap.has(m.id)) {
                        modelMap.set(m.id, { ...m });
                    } else {
                        // 保留具有最大额度的记录作为全局可用性参考
                        const existing = modelMap.get(m.id)!;
                        const currentRem = m.remainingFraction ?? 0;
                        const existingRem = existing.remainingFraction ?? 0;
                        if (currentRem > existingRem) {
                            modelMap.set(m.id, { ...m });
                        }
                    }
                });
            });
            return Array.from(modelMap.values()).sort((a, b) => a.id.localeCompare(b.id));
        } else if (provider === 'codex') {
            return codexModelsList.map(id => ({ id } as QuotaModelInfo));
        } else if (provider === 'kiro') {
            return kiroModelsList.map(id => ({ id } as QuotaModelInfo));
        } else if (provider === 'windsurf') {
            return windsurfModelsList.map(id => ({ id } as QuotaModelInfo));
        } else if (provider === 'warp') {
            return warpModelsList.map(id => ({ id } as QuotaModelInfo));
        }
        return [];
    }, [provider, allAntigravityAccounts, codexModelsList, kiroModelsList, windsurfModelsList, warpModelsList]);

    // 派生适用账号列表及对应额度
    const applicableAccounts = useMemo(() => {
        if (provider === 'antigravity') {
            return allAntigravityAccounts.map(acc => {
                const m = acc.models.find(x => x.id === model);
                const hasQuota = m ? (m.remainingFraction === undefined || m.remainingFraction > 0) : false;
                const percentage = m?.remainingFraction !== undefined ? Math.round(m.remainingFraction * 100) : null;
                return {
                    email: acc.email,
                    id: acc.id,
                    hasQuota,
                    desc: hasQuota ? (percentage !== null ? `(余额 ${percentage}%)` : '') : '(额度耗尽)'
                };
            });
        } else if (provider === 'codex') {
            return allCodexAccounts.map(acc => {
                // Codex 如果还没查到过 quota，默认允许尝试。一旦获取，按 hourly_percentage 判断
                const hasQuota = acc.quota ? acc.quota.hourly_percentage > 0 : true;
                const percentage = acc.quota?.hourly_percentage;
                return {
                    email: acc.email,
                    id: acc.id,
                    hasQuota,
                    desc: hasQuota ? (percentage !== undefined ? `(余额 ${percentage}%)` : '') : '(额度耗尽)',
                    plan: acc.plan_type
                };
            });
        } else if (provider === 'kiro') {
            return allKiroAccounts.map(acc => {
                let hasQuota = true;
                let desc = acc.plan_name ? `(${acc.plan_name})` : '';

                // 尝试计算剩余额度 (考虑 Prompt Credits 和 Add-on Credits)
                // TypeScript 类型推导可能不知道 credits_total 的存在，所以使用了 any 或 ts-ignore，
                // 但为了严谨，我们先通过计算剩余百分比
                const anyAcc = acc as any;
                let pct: number | null = null;

                if (anyAcc.credits_total && anyAcc.credits_total > 0) {
                    const total = anyAcc.credits_total;
                    const used = anyAcc.credits_used || 0;
                    const rem = Math.max(0, total - used);
                    pct = Math.round((rem / total) * 100);
                } else if (anyAcc.bonus_total && anyAcc.bonus_total > 0) {
                    const total = anyAcc.bonus_total;
                    const used = anyAcc.bonus_used || 0;
                    const rem = Math.max(0, total - used);
                    pct = Math.round((rem / total) * 100);
                }

                if (pct !== null) {
                    hasQuota = pct > 0;
                    desc += hasQuota ? ` (余额 ${pct}%)` : ' (额度耗尽)';
                }

                return {
                    email: acc.email,
                    id: acc.id,
                    hasQuota,
                    desc: desc.trim()
                };
            });
        } else if (provider === 'windsurf') {
            return allWindsurfAccounts.map(acc => ({
                email: acc.github_login,
                id: acc.id,
                hasQuota: true, // 暂不检查 windsurf 配额
                desc: acc.copilot_plan ? `(${acc.copilot_plan})` : ''
            }));
        } else if (provider === 'warp') {
            // Warp 只有单一全局会话/凭据（或未来配置项），此列表仅作兼容占位
            return [{ email: 'Global Warp Pool', id: 'global-warp', hasQuota: true, desc: '' }];
        }
        return [];
    }, [provider, model, allAntigravityAccounts, allCodexAccounts, allKiroAccounts, allWindsurfAccounts]);

    // 切换平台时清空对话，避免不同平台的模型和协议上下文混淆
    useEffect(() => {
        setMessages([]);
    }, [provider]);

    // 自动选择模型联动
    useEffect(() => {
        if (availableModels.length > 0) {
            // 当切换平台后，之前的模型可能不在新的 availableModels 中
            if (!availableModels.some(m => m.id === model)) {
                const firstAvail = availableModels.find(m => m.remainingFraction === undefined || m.remainingFraction > 0);
                setModel(firstAvail ? firstAvail.id : availableModels[0].id);
            }
        }
    }, [provider, availableModels, model]);

    // 自动选择账号联动
    useEffect(() => {
        if (applicableAccounts.length > 0) {
            // 允许用户显式选择“自动分配” (空字符串)
            if (selectedAccountEmail === '') return;

            // 当前选中的账号是否在新模型的列表中，并且有额度
            const hasSelected = applicableAccounts.find(a => a.email === selectedAccountEmail);
            if (!hasSelected || !hasSelected.hasQuota) {
                const firstAvail = applicableAccounts.find(a => a.hasQuota);
                setSelectedAccountEmail(firstAvail ? firstAvail.email : applicableAccounts[0].email);
            }
        } else {
            setSelectedAccountEmail('');
        }
    }, [provider, model, applicableAccounts, selectedAccountEmail]);

    // 当选中账号改变，更新代理配置以确保发往正确账号
    useEffect(() => {
        if (proxyStatus?.running && selectedAccountEmail !== undefined) {
            invoke('save_api_proxy_config', {
                config: {
                    enabled: true,
                    port: proxyConfig?.port || 19531,
                    api_key: proxyConfig?.api_key || '',
                    request_timeout: proxyConfig?.request_timeout || 120,
                    warp_api_url: proxyConfig?.warp_api_url || 'http://127.0.0.1:8010',
                    auto_start: true,
                    providers: {
                        antigravity: { enabled: providerEnabled.antigravity, strategy: 'round_robin' },
                        codex: { enabled: providerEnabled.codex, strategy: 'round_robin' },
                        kiro: { enabled: providerEnabled.kiro, strategy: 'round_robin' },
                        windsurf: { enabled: providerEnabled.windsurf, strategy: 'round_robin' },
                        warp: { enabled: providerEnabled.warp, strategy: 'round_robin' },
                    },
                    selected_account_email: selectedAccountEmail,
                },
            }).catch(() => { });
        }
    }, [selectedAccountEmail, proxyStatus, proxyConfig, providerEnabled]);

    // 全局快捷键
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {

            // Ctrl+Shift+X 清空对话
            if (e.ctrlKey && e.shiftKey && e.key === 'X') {
                e.preventDefault();
                setMessages([]);
            }
            // Ctrl+Shift+P 启动/停止代理
            if (e.ctrlKey && e.shiftKey && e.key === 'P') {
                e.preventDefault();
                if (proxyStatus?.running) {
                    handleStopProxy();
                } else {
                    handleOpenStartModal();
                }
            }
            // Escape 关闭弹窗
            if (e.key === 'Escape') {
                setShowStartModal(false);
                setShowShortcuts(false);
            }
        };
        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [proxyStatus]);

    // 监听全局快捷键面板事件（从 App.tsx 派发）
    useEffect(() => {
        const handleToggle = () => setShowShortcuts(prev => !prev);
        window.addEventListener('toggle-shortcuts-panel', handleToggle);
        return () => window.removeEventListener('toggle-shortcuts-panel', handleToggle);
    }, []);



    const processFiles = (files: File[]) => {
        files.forEach(file => {
            const reader = new FileReader();
            reader.onload = (e) => {
                const base64 = (e.target?.result as string).split(',')[1];
                setAttachedImages(prev => [...prev, { data: base64, mime_type: file.type }]);
            };
            reader.readAsDataURL(file);
        });
    };

    const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const files = Array.from(e.target.files || []).filter(f => f.type.startsWith('image/'));
        processFiles(files);
        if (fileInputRef.current) fileInputRef.current.value = '';
    };

    const handlePaste = (e: React.ClipboardEvent) => {
        const items = Array.from(e.clipboardData.items);
        const files = items.filter(item => item.type.startsWith('image/')).map(item => item.getAsFile()!).filter(Boolean);
        if (files.length > 0) {
            e.preventDefault();
            processFiles(files);
        }
    };

    const handleDrop = (e: React.DragEvent) => {
        e.preventDefault();
        const files = Array.from(e.dataTransfer.files).filter(f => f.type.startsWith('image/'));
        processFiles(files);
    };

    const handleDragOver = (e: React.DragEvent) => {
        e.preventDefault();
    };

    const handleSend = useCallback(async () => {
        if (!input.trim() && attachedImages.length === 0) return;
        if (streaming) return;
        if (!proxyStatus?.running) {
            alert('代理服务未启动，请先点击「启动代理」或去设置页启动 API 反向代理');
            return;
        }
        // 优先用 actual_port，若未同步则用配置端口，确保「运行中」时一定能发请求
        const port = proxyPort ?? proxyConfig?.port ?? 19531;
        if (!port) {
            alert('无法确定代理端口，请重新启动代理');
            return;
        }

        const userMsg: ChatMessage = {
            id: Date.now().toString(),
            role: 'user',
            content: input.trim(),
            timestamp: Date.now(),
            provider,
            images: attachedImages.length > 0 ? [...attachedImages] : undefined,
        };

        const assistantMsg: ChatMessage = {
            id: (Date.now() + 1).toString(),
            role: 'assistant',
            content: '',
            thinking: '', // 新增字段：思考过程
            timestamp: Date.now(),
            provider,
            streaming: true,
        };

        setMessages(prev => [...prev, userMsg, assistantMsg]);
        setInput('');
        setAttachedImages([]);
        setStreaming(true);

        const controller = new AbortController();
        abortControllerRef.current = controller;

        const buildAnthropicContent = (msg: ChatMessage) => {
            if (!msg.images || msg.images.length === 0) return msg.content;
            const arr: any[] = msg.images.map(img => ({
                type: 'image',
                source: { type: 'base64', media_type: img.mime_type, data: img.data }
            }));
            if (msg.content) arr.push({ type: 'text', text: msg.content });
            return arr;
        };

        const buildCodexContent = (msg: ChatMessage) => {
            if (!msg.images || msg.images.length === 0) return msg.content;
            const arr: any[] = [];
            if (msg.content) arr.push({ type: 'text', text: msg.content });
            msg.images.forEach(img => {
                arr.push({ type: 'image_url', image_url: { url: `data:${img.mime_type};base64,${img.data}` } });
            });
            return arr;
        };

        try {
            const baseUrl = `http://127.0.0.1:${port}`;

            if (provider === 'antigravity' || provider === 'kiro') {
                // Anthropic Messages API (Antigravity & Kiro)
                const apiPath = provider === 'antigravity' ? '/antigravity/v1/messages' : '/kiro/v1/messages';
                const headers: Record<string, string> = {
                    'Content-Type': 'application/json',
                    'x-api-key': 'chat-test',
                    'anthropic-version': '2023-06-01',
                };
                // 仅当当前选中的账号属于本 provider 的列表时才带该头，避免带上 Antigravity 邮箱导致 Kiro 侧过滤不到账号
                if (selectedAccountEmail && applicableAccounts.some(a => a.email === selectedAccountEmail)) {
                    headers['x-selected-account-email'] = selectedAccountEmail;
                }
                const response = await fetch(`${baseUrl}${apiPath}`, {
                    method: 'POST',
                    signal: controller.signal,
                    headers,
                    body: JSON.stringify({
                        model,
                        max_tokens: 4096,
                        stream: true,
                        messages: [
                            ...messages.filter(m => !m.streaming).map(m => ({
                                role: m.role,
                                content: buildAnthropicContent(m),
                            })),
                            { role: 'user', content: buildAnthropicContent(userMsg) },
                        ],
                    }),
                });

                if (!response.ok) {
                    const errText = await response.text();
                    throw new Error(`HTTP ${response.status}: ${errText}`);
                }

                // 解析 SSE 流
                const reader = response.body?.getReader();
                const decoder = new TextDecoder();
                let accumulatedText = '';
                let accumulatedThinking = '';

                if (reader) {
                    let buffer = '';
                    while (true) {
                        const { done, value } = await reader.read();
                        if (done) break;

                        buffer += decoder.decode(value, { stream: true });
                        let p = 0;
                        while ((p = buffer.indexOf('\n\n')) !== -1) {
                            const chunk = buffer.slice(0, p);
                            buffer = buffer.slice(p + 2);

                            const dataLines = chunk.split('\n');
                            for (const line of dataLines) {
                                if (line.startsWith('data: ')) {
                                    const dataStr = line.slice(6).trim();
                                    if (dataStr === '[DONE]') continue;
                                    try {
                                        const event = JSON.parse(dataStr);
                                        if (event.type === 'content_block_delta' && event.delta?.type === 'text_delta' && event.delta?.text) {
                                            accumulatedText += event.delta.text;
                                            setMessages(prev => prev.map(m => m.id === assistantMsg.id ? { ...m, content: accumulatedText } : m));
                                        }
                                        // 处理思考块内容
                                        else if (event.type === 'content_block_delta' && event.delta?.type === 'thinking_delta' && event.delta?.thinking) {
                                            accumulatedThinking += event.delta.thinking;
                                            setMessages(prev =>
                                                prev.map(m =>
                                                    m.id === assistantMsg.id
                                                        ? { ...m, thinking: accumulatedThinking }
                                                        : m
                                                )
                                            );
                                        }
                                    } catch (e) {
                                        console.warn("Failed to parse SSE JSON:", dataStr, e);
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // OpenAI Chat Completions API 兼容格式 (Codex, Windsurf, Warp)
                    // 根据不同 provider 调整请求路径
                    let apiPath = '/codex/v1/chat/completions';
                    const prov = provider as string;
                    if (prov === 'windsurf') apiPath = '/windsurf/v1/chat/completions';
                    else if (prov === 'warp') apiPath = '/warp/v1/chat/completions';

                    const headers: Record<string, string> = {
                        'Content-Type': 'application/json',
                        'Authorization': 'Bearer chat-test',
                    };
                    if (selectedAccountEmail && applicableAccounts.some(a => a.email === selectedAccountEmail)) {
                        headers['x-selected-account-email'] = selectedAccountEmail;
                    }
                    const response = await fetch(`${baseUrl}${apiPath}`, {
                        method: 'POST',
                        signal: controller.signal,
                        headers,
                        body: JSON.stringify({
                            model,
                            stream: true,
                            messages: [
                                ...messages.filter(m => !m.streaming).map(m => ({
                                    role: m.role,
                                    content: buildCodexContent(m),
                                })),
                                { role: 'user', content: buildCodexContent(userMsg) },
                            ],
                        }),
                    });

                    if (!response.ok) {
                        const errText = await response.text();
                        throw new Error(`HTTP ${response.status}: ${errText}`);
                    }

                    const reader = response.body?.getReader();
                    const decoder = new TextDecoder();
                    let accumulated = '';

                    if (reader) {
                        let buffer = '';
                        while (true) {
                            const { done, value } = await reader.read();
                            if (done) break;

                            buffer += decoder.decode(value, { stream: true });
                            const lines = buffer.split('\n');
                            buffer = lines.pop() || '';
                            for (const line of lines) {
                                if (!line.startsWith('data: ')) continue;
                                const data = line.slice(6).trim();
                                if (data === '[DONE]') continue;

                                try {
                                    const event = JSON.parse(data);
                                    const delta = event.choices?.[0]?.delta?.content;
                                    if (delta) {
                                        accumulated += delta;
                                        setMessages(prev => prev.map(m => m.id === assistantMsg.id ? { ...m, content: accumulated } : m));
                                    }
                                } catch {
                                    // 跳过
                                }
                            }
                        }
                    }
                }
            }

            // 标记流式结束
            setMessages(prev =>
                prev.map(m =>
                    m.id === assistantMsg.id
                        ? { ...m, streaming: false }
                        : m
                )
            );
        } catch (err: any) {
            if (err.name === 'AbortError') {
                setMessages(prev =>
                    prev.map(m =>
                        m.id === assistantMsg.id ? { ...m, streaming: false } : m
                    )
                );
            } else {
                const msg = String(err?.message || err);
                let hint = msg;
                if (/Failed to fetch|NetworkError|Load failed|ECONNREFUSED|连接被拒绝/i.test(msg)) {
                    hint = '无法连接代理（连接被拒绝或超时）。请确认：1) 已在本页或设置页点击「启动代理」且显示「代理运行中」；2) 端口与页面显示一致。';
                } else if (/403|token|invalid|permission/i.test(msg) && (provider === 'kiro' || provider === 'codex')) {
                    hint = `当前选中账号可能 token 已失效或无权访问。请在下拉框中选择与 IDE 中能正常使用相同的账号，或在账号管理里重新登录后再试。\n\n原始错误: ${msg}`;
                }
                setMessages(prev =>
                    prev.map(m =>
                        m.id === assistantMsg.id
                            ? { ...m, content: (m.content ? m.content + '\n\n' : '') + `❌ ${hint}`, streaming: false }
                            : m
                    )
                );
            }
        } finally {
            setStreaming(false);
            abortControllerRef.current = null;
        }
    }, [input, streaming, proxyStatus, proxyPort, proxyConfig, provider, model, messages, attachedImages, selectedAccountEmail, applicableAccounts]);

    const handleStop = () => {
        if (abortControllerRef.current) {
            abortControllerRef.current.abort();
            abortControllerRef.current = null;
        }
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            handleSend();
        }
    };

    const handleClear = () => {
        setMessages([]);
    };

    // 打开启动弹窗（先加载已保存的配置）
    const handleOpenStartModal = async () => {
        try {
            const savedConfig = await invoke<any>('get_api_proxy_config');
            setProxyConfig({
                port: savedConfig.port || 19531,
                api_key: savedConfig.api_key || 'chat-test',
                request_timeout: savedConfig.request_timeout || 120,
                warp_api_url: savedConfig.warp_api_url || 'http://127.0.0.1:8010',
            });
            if (savedConfig.selected_account_email) {
                setSelectedAccountEmail(savedConfig.selected_account_email);
            }
        } catch {
            // 使用默认值
        }
        // 加载账号列表
        try {
            const accs = await invoke<any[]>('list_accounts');
            const validAccs = (accs || []).filter((a: any) => !a.disabled && a.token?.access_token);
            // 这里原来是 setAccounts(validAccs) -> 既然现在不需要手动维护 accounts 了，我们可以直接用 applicableAccounts 或者由 backend 加载好的 allAntigravityAccounts
            // 如果未选择账号，默认选择第一个
            if (!selectedAccountEmail && validAccs.length > 0) {
                setSelectedAccountEmail(validAccs[0].email);
            }
        } catch {
            // 忽略
        }
        setShowStartModal(true);
    };

    // 启动代理
    const handleStartProxy = async () => {
        setProxyStarting(true);
        try {
            // 先保存配置
            await invoke('save_api_proxy_config', {
                config: {
                    enabled: true,
                    port: proxyConfig.port,
                    api_key: proxyConfig.api_key,
                    request_timeout: proxyConfig.request_timeout,
                    warp_api_url: proxyConfig.warp_api_url,
                    auto_start: true,
                    providers: {
                        antigravity: { enabled: providerEnabled.antigravity, strategy: 'round_robin' },
                        codex: { enabled: providerEnabled.codex, strategy: 'round_robin' },
                        kiro: { enabled: providerEnabled.kiro, strategy: 'round_robin' },
                        windsurf: { enabled: providerEnabled.windsurf, strategy: 'round_robin' },
                        warp: { enabled: providerEnabled.warp, strategy: 'round_robin' },
                    },
                    selected_account_email: selectedAccountEmail,
                },
            });
            // 启动
            const status = await invoke<ApiProxyStatus>('start_api_proxy');
            setProxyStatus(status);
            setProxyPort(status.actual_port);
            setShowStartModal(false);
        } catch (err) {
            alert(`启动失败: ${err}`);
        } finally {
            setProxyStarting(false);
        }
    };

    // 停止代理
    const handleStopProxy = async () => {
        try {
            await invoke('stop_api_proxy');
            setProxyStatus({ running: false, port: proxyConfig.port, actual_port: null, enabled_providers: [] });
            setProxyPort(null);
        } catch (err) {
            alert(`停止失败: ${err}`);
        }
    };

    return (
        <div className="chat-page">
            {/* 顶栏 */}
            <div className="chat-header">
                <div className="chat-header-left">
                    <h2>💬 Chat 测试</h2>
                    <div className="chat-status">
                        {proxyStatus?.running ? (
                            <>
                                <span className="status-badge status-online">
                                    <Server size={14} />
                                    代理运行中 :{proxyPort ?? proxyConfig?.port ?? '—'}
                                </span>
                                <button className="chat-btn chat-btn-stop" onClick={handleStopProxy} title="停止代理">
                                    <StopCircle size={14} /> 停止
                                </button>
                            </>
                        ) : (
                            <>
                                <span className="status-badge status-offline">
                                    <ServerOff size={14} />
                                    代理未启动
                                </span>
                                <button className="chat-btn chat-btn-start" onClick={handleOpenStartModal} title="启动代理">
                                    <PlayCircle size={14} /> 启动
                                </button>
                            </>
                        )}
                    </div>
                </div>
                <div className="chat-header-right">
                    <select
                        className="chat-select"
                        value={provider}
                        onChange={(e) => setProvider(e.target.value)}
                    >
                        {PROVIDER_OPTIONS.map(p => (
                            <option key={p.value} value={p.value}>{p.label}</option>
                        ))}
                    </select>
                    <select
                        className="chat-select"
                        value={model}
                        onChange={(e) => setModel(e.target.value)}
                        disabled={modelsLoading || availableModels.length === 0}
                    >
                        {modelsLoading ? (
                            <option>加载模型中...</option>
                        ) : availableModels.length === 0 ? (
                            <option>未获取到可用模型</option>
                        ) : (
                            availableModels.map(m => {
                                let label = m.id;
                                const disabled = provider === 'antigravity' && m.remainingFraction === 0;
                                if (provider === 'antigravity') {
                                    if (disabled) {
                                        label += ` (额度耗尽${m.resetTime ? `, ${new Date(m.resetTime).toLocaleString()}恢复` : ''})`;
                                    } else if (m.remainingFraction !== undefined) {
                                        label += ` (余额: ${Math.round(m.remainingFraction * 100)}%)`;
                                    }
                                }
                                return (
                                    <option key={m.id} value={m.id} disabled={disabled}>
                                        {label}
                                    </option>
                                );
                            })
                        )}
                    </select>
                    <select
                        className="chat-select"
                        value={selectedAccountEmail}
                        onChange={(e) => setSelectedAccountEmail(e.target.value)}
                        title="选择账号"
                        disabled={modelsLoading || applicableAccounts.length === 0}
                    >
                        <option value="">自动分配/全部账号</option>
                        {applicableAccounts.map(acc => (
                            <option key={acc.id} value={acc.email} disabled={!acc.hasQuota}>
                                {acc.email} {acc.desc}
                            </option>
                        ))}
                    </select>
                    <button className="chat-btn chat-btn-icon" onClick={handleClear} title="清空会话">
                        <Trash2 size={16} />
                    </button>
                </div>
            </div>

            {/* 消息区域 */}
            <div className="chat-messages">
                {messages.length === 0 && (
                    <div className="chat-empty">
                        <div className="chat-empty-icon">🤖</div>
                        <h3>开始对话</h3>
                        <p>通过本地代理向 AI 发送消息，测试反向代理功能</p>
                        <div className="chat-empty-tips">
                            <div className="tip">1. 确保设置页中 API 反向代理已启动</div>
                            <div className="tip">2. 选择要测试的 Provider 和模型</div>
                            <div className="tip">3. 输入消息开始对话</div>
                        </div>
                    </div>
                )}
                {messages.map((msg) => (
                    <div key={msg.id} className={`chat-message chat-message-${msg.role}`}>
                        <div className="chat-message-avatar">
                            {msg.role === 'user' ? '👤' : '🤖'}
                        </div>
                        <div className="chat-message-body">
                            <div className="chat-message-content">
                                {msg.thinking && (
                                    <details className="chat-thinking-block">
                                        <summary>🤔 思考过程</summary>
                                        <div className="chat-thinking-content">{msg.thinking}</div>
                                    </details>
                                )}
                                {msg.content || (msg.streaming && !msg.thinking && (
                                    <span className="chat-typing">
                                        <Loader size={14} className="spin" /> 思考中...
                                    </span>
                                ))}
                                {msg.images && msg.images.length > 0 && (
                                    <div className="chat-message-images">
                                        {msg.images.map((img, i) => (
                                            <img key={i} src={`data:${img.mime_type};base64,${img.data}`} alt="attachment" className="chat-message-image" />
                                        ))}
                                    </div>
                                )}
                            </div>
                            {msg.provider && (
                                <div className="chat-message-meta">
                                    {msg.provider} · {new Date(msg.timestamp).toLocaleTimeString()}
                                </div>
                            )}
                        </div>
                    </div>
                ))}
                <div ref={messagesEndRef} />
            </div>

            {/* 输入区域 */}
            <div className="chat-input-container">
                {attachedImages.length > 0 && (
                    <div className="chat-attachments-preview">
                        {attachedImages.map((img, index) => (
                            <div key={index} className="chat-attachment-item">
                                <img src={`data:${img.mime_type};base64,${img.data}`} alt="preview" />
                                <button className="chat-attachment-remove" onClick={() => setAttachedImages(prev => prev.filter((_, i) => i !== index))} title="移除附件">
                                    <X size={12} />
                                </button>
                            </div>
                        ))}
                    </div>
                )}
                <div
                    className="chat-input-area"
                    onPaste={handlePaste}
                    onDrop={handleDrop}
                    onDragOver={handleDragOver}
                >
                    <input
                        type="file"
                        ref={fileInputRef}
                        style={{ display: 'none' }}
                        accept="image/*"
                        multiple
                        onChange={handleFileChange}
                    />
                    <button
                        className="chat-btn chat-btn-icon chat-btn-attach"
                        onClick={() => fileInputRef.current?.click()}
                        title="上传图片"
                        disabled={streaming}
                    >
                        <ImageIcon size={18} />
                    </button>
                    <textarea
                        ref={textareaRef}
                        className="chat-textarea"
                        placeholder="输入消息... (支持粘贴和拖拽图片, Enter 发送, Shift+Enter 换行)"
                        value={input}
                        onChange={(e) => setInput(e.target.value)}
                        onKeyDown={handleKeyDown}
                        rows={2}
                        disabled={streaming}
                    />
                    {streaming ? (
                        <button
                            className="chat-btn chat-btn-stop-gen"
                            onClick={handleStop}
                            title="停止生成"
                        >
                            <StopCircle size={18} />
                        </button>
                    ) : (
                        <button
                            className="chat-btn chat-btn-send"
                            onClick={handleSend}
                            disabled={(!input.trim() && attachedImages.length === 0) || !proxyStatus?.running}
                        >
                            <Send size={18} />
                        </button>
                    )}
                </div>
            </div>

            {/* 启动代理弹窗 */}
            {showStartModal && (
                <div className="chat-modal-overlay" onClick={() => setShowStartModal(false)}>
                    <div className="chat-modal" onClick={e => e.stopPropagation()}>
                        <div className="chat-modal-header">
                            <h3><Settings size={18} /> 启动 API 反向代理</h3>
                            <button className="chat-btn chat-btn-icon" onClick={() => setShowStartModal(false)}>
                                <X size={16} />
                            </button>
                        </div>
                        <div className="chat-modal-body">
                            <div className="chat-modal-field">
                                <label>监听端口</label>
                                <input
                                    type="number"
                                    value={proxyConfig.port}
                                    onChange={e => setProxyConfig(c => ({ ...c, port: parseInt(e.target.value) || 19531 }))}
                                />
                            </div>
                            <div className="chat-modal-field">
                                <label>API Key（客户端认证）</label>
                                <input
                                    type="text"
                                    value={proxyConfig.api_key}
                                    onChange={e => setProxyConfig(c => ({ ...c, api_key: e.target.value }))}
                                    placeholder="留空则无需认证"
                                />
                            </div>
                            <div className="chat-modal-field">
                                <label>请求超时（秒）</label>
                                <input
                                    type="number"
                                    value={proxyConfig.request_timeout}
                                    onChange={e => setProxyConfig(c => ({ ...c, request_timeout: parseInt(e.target.value) || 120 }))}
                                />
                            </div>
                            <div className="chat-modal-field">
                                <label>Warp API 服务地址</label>
                                <input
                                    type="text"
                                    value={proxyConfig.warp_api_url}
                                    onChange={e => setProxyConfig(c => ({ ...c, warp_api_url: e.target.value }))}
                                    placeholder="默认: http://127.0.0.1:8010"
                                />
                            </div>
                            <div className="chat-modal-field">
                                <label>使用账号</label>
                                <select
                                    value={selectedAccountEmail}
                                    onChange={e => setSelectedAccountEmail(e.target.value)}
                                >
                                    <option value="">全部可用账号</option>
                                    {applicableAccounts.map(acc => (
                                        <option key={acc.id} value={acc.email}>{acc.email}</option>
                                    ))}
                                </select>
                            </div>
                            <div className="chat-modal-info">
                                <label className="chat-toggle-row">
                                    <input
                                        type="checkbox"
                                        checked={providerEnabled.antigravity}
                                        onChange={e => setProviderEnabled(p => ({ ...p, antigravity: e.target.checked }))}
                                    />
                                    <span>Antigravity (Claude)</span>
                                </label>
                                <label className="chat-toggle-row">
                                    <input
                                        type="checkbox"
                                        checked={providerEnabled.codex}
                                        onChange={e => setProviderEnabled(p => ({ ...p, codex: e.target.checked }))}
                                    />
                                    <span>Codex (OpenAI)</span>
                                </label>
                                <label className="chat-toggle-row">
                                    <input
                                        type="checkbox"
                                        checked={providerEnabled.kiro}
                                        onChange={e => setProviderEnabled(p => ({ ...p, kiro: e.target.checked }))}
                                    />
                                    <span>Kiro (Amazon Q)</span>
                                </label>
                                <label className="chat-toggle-row">
                                    <input
                                        type="checkbox"
                                        checked={providerEnabled.windsurf}
                                        onChange={e => setProviderEnabled(p => ({ ...p, windsurf: e.target.checked }))}
                                    />
                                    <span>Windsurf (GitHub Copilot)</span>
                                </label>
                                <label className="chat-toggle-row">
                                    <input
                                        type="checkbox"
                                        checked={providerEnabled.warp}
                                        onChange={e => setProviderEnabled(p => ({ ...p, warp: e.target.checked }))}
                                    />
                                    <span>Warp (Multi-Agent)</span>
                                </label>
                            </div>
                        </div>
                        <div className="chat-modal-footer">
                            <button
                                className="chat-btn chat-btn-cancel"
                                onClick={() => setShowStartModal(false)}
                                disabled={proxyStarting}
                            >
                                取消
                            </button>
                            <button
                                className="chat-btn chat-btn-primary"
                                onClick={handleStartProxy}
                                disabled={proxyStarting}
                            >
                                {proxyStarting ? <><Loader size={14} className="spin" /> 启动中...</> : <><PlayCircle size={14} /> 一键启动</>}
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* 快捷键面板 */}
            {showShortcuts && (
                <div className="chat-modal-overlay" onClick={() => setShowShortcuts(false)}>
                    <div className="chat-modal" onClick={e => e.stopPropagation()}>
                        <div className="chat-modal-header">
                            <h3><Keyboard size={18} /> 快捷键</h3>
                            <button className="chat-btn chat-btn-icon" onClick={() => setShowShortcuts(false)}>
                                <X size={16} />
                            </button>
                        </div>
                        <div className="chat-modal-body">
                            <div className="shortcut-list">
                                <div className="shortcut-item"><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>C</kbd><span>打开 Chat 页（全局）</span></div>
                                <div className="shortcut-item"><kbd>Ctrl</kbd>+<kbd>/</kbd><span>显示/隐藏快捷键面板</span></div>
                                <div className="shortcut-item"><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd><span>启动/停止代理</span></div>
                                <div className="shortcut-item"><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>X</kbd><span>清空对话</span></div>
                                <div className="shortcut-item"><kbd>Enter</kbd><span>发送消息</span></div>
                                <div className="shortcut-item"><kbd>Shift</kbd>+<kbd>Enter</kbd><span>换行</span></div>
                                <div className="shortcut-item"><kbd>Esc</kbd><span>关闭弹窗</span></div>
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
