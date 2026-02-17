import { useTranslation } from 'react-i18next';
import { PlatformInstancesContent } from '../components/platform/PlatformInstancesContent';
import { useCodexInstanceStore } from '../stores/useCodexInstanceStore';
import { useCodexAccountStore } from '../stores/useCodexAccountStore';
import type { CodexAccount } from '../types/codex';
import { getCodexQuotaClass } from '../types/codex';
import { usePlatformRuntimeSupport } from '../hooks/usePlatformRuntimeSupport';

/**
 * Codex 多开实例内容组件（不包含 header）
 * 用于嵌入到 CodexAccountsPage 中
 */
export function CodexInstancesContent() {
  const { t } = useTranslation();
  const instanceStore = useCodexInstanceStore();
  const { accounts, fetchAccounts } = useCodexAccountStore();
  const isSupportedPlatform = usePlatformRuntimeSupport('macos-only');

  const resolveQuotaClass = (percentage: number) => {
    const mapped = getCodexQuotaClass(percentage);
    return mapped === 'critical' ? 'low' : mapped;
  };

  const renderCodexQuotaPreview = (account: CodexAccount) => {
    if (!account.quota) {
      return <span className="account-quota-empty">{t('instances.quota.empty', '暂无配额缓存')}</span>;
    }
    const hourly = account.quota.hourly_percentage;
    const weekly = account.quota.weekly_percentage;
    return (
      <div className="account-quota-preview">
        <span className="account-quota-item">
          <span className={`quota-dot ${resolveQuotaClass(hourly)}`} />
          <span className={`quota-text ${resolveQuotaClass(hourly)}`}>
            {t('codex.instances.quota.hourly', '5h')} {hourly}%
          </span>
        </span>
        <span className="account-quota-item">
          <span className={`quota-dot ${resolveQuotaClass(weekly)}`} />
          <span className={`quota-text ${resolveQuotaClass(weekly)}`}>
            {t('codex.instances.quota.weekly', '周')} {weekly}%
          </span>
        </span>
      </div>
    );
  };

  return (
    <PlatformInstancesContent
      instanceStore={instanceStore}
      accounts={accounts}
      fetchAccounts={fetchAccounts}
      renderAccountQuotaPreview={renderCodexQuotaPreview}
      getAccountSearchText={(account) => account.email}
      appType="codex"
      isSupported={isSupportedPlatform}
      unsupportedTitleKey="codex.instances.unsupported.title"
      unsupportedTitleDefault="暂不支持当前系统"
      unsupportedDescKey="codex.instances.unsupported.desc"
      unsupportedDescDefault="Codex 多开实例仅支持 macOS。"
    />
  );
}
