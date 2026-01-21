import { QuotaData } from '../types/account';

export const DISPLAY_MODEL_ORDER = [
  { ids: ['gemini-3-pro-high'], label: 'G3 Pro' },
  { ids: ['gemini-3-pro-image'], label: 'G3 Image' },
  { ids: ['gemini-3-flash'], label: 'G3 Flash' },
  { ids: ['claude-sonnet-4-5-thinking', 'claude-sonnet-4-5'], label: 'Claude 4.5' },
];

export function matchModelName(modelName: string, target: string): boolean {
  return modelName === target || modelName.startsWith(`${target}-`);
}

export function getSubscriptionTier(quota?: QuotaData): string {
  const tier = quota?.subscription_tier || 'FREE';
  // 映射等级名称
  if (tier.includes('PRO') || tier.includes('pro')) return 'PRO';
  if (tier.includes('ULTRA') || tier.includes('ultra')) return 'ULTRA';
  return 'FREE';
}

export function getQuotaClass(percentage: number): string {
  if (percentage >= 70) return 'high';
  if (percentage >= 30) return 'medium';
  return 'low';
}

export function formatResetTime(resetTime: string): string {
  if (!resetTime) return '';
  try {
    const reset = new Date(resetTime);
    if (Number.isNaN(reset.getTime())) return '';
    const now = new Date();
    const diffMs = reset.getTime() - now.getTime();
    if (diffMs <= 0) return '已重置';

    const totalMinutes = Math.floor(diffMs / (1000 * 60));
    const days = Math.floor(totalMinutes / (60 * 24));
    const hours = Math.floor((totalMinutes % (60 * 24)) / 60);
    const minutes = totalMinutes % 60;

    if (days >= 1) {
      if (minutes > 0) {
        return `${days}d ${hours}h ${minutes}m`;
      }
      return `${days}d ${hours}h`;
    }
    if (hours >= 1) return `${hours}h ${minutes}m`;
    if (minutes >= 1) return `${minutes}m`;
    return '<1m';
  } catch {
    return '';
  }
}

export function formatResetTimeAbsolute(resetTime: string): string {
  if (!resetTime) return '';
  const reset = new Date(resetTime);
  if (Number.isNaN(reset.getTime())) return '';
  const pad = (value: number) => String(value).padStart(2, '0');
  const year = reset.getFullYear();
  const month = pad(reset.getMonth() + 1);
  const day = pad(reset.getDate());
  const hours = pad(reset.getHours());
  const minutes = pad(reset.getMinutes());
  return `${year}-${month}-${day} ${hours}:${minutes}`;
}

export function formatResetTimeDisplay(resetTime: string): string {
  const relative = formatResetTime(resetTime);
  const absolute = formatResetTimeAbsolute(resetTime);
  if (!relative && !absolute) return '';
  if (relative === '已重置') return relative;
  if (!absolute) return relative;
  if (!relative) return absolute;
  return `${absolute} (${relative})`;
}

export function getDisplayModels(quota?: QuotaData) {
  if (!quota?.models) {
    console.log('[getDisplayModels] quota 或 models 为空:', { quota });
    return [];
  }
  
  const normalized = quota.models.map((model) => ({
    model,
    nameLower: model.name.toLowerCase(),
  }));
  
  const pickModel = (ids: string[]) =>
    normalized.find((item) => ids.some((id) => matchModelName(item.nameLower, id)))?.model;
  
  const result = DISPLAY_MODEL_ORDER
    .map((item) => pickModel(item.ids))
    .filter((model): model is (typeof quota.models)[number] => Boolean(model));
  
  // 调试日志：显示匹配过程
  if (result.length === 0 && quota.models.length > 0) {
    console.log('[getDisplayModels] 有模型数据但匹配失败:', {
      availableModels: quota.models.map(m => m.name),
      expectedIds: DISPLAY_MODEL_ORDER.flatMap(item => item.ids),
    });
  }
  
  return result;
}

export function getModelShortName(name: string): string {
  const normalized = name.toLowerCase();
  for (const item of DISPLAY_MODEL_ORDER) {
    if (item.ids.some((id) => matchModelName(normalized, id))) {
      return item.label;
    }
  }
  return name;
}
