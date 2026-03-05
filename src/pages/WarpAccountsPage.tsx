import { useState, useEffect } from 'react';
import { RefreshCw, Trash2, Tag, Copy, LayoutGrid, List } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useWarpAccountStore } from '../stores/useWarpAccountStore';
import { TagEditModal } from '../components/TagEditModal';
import { maskSensitiveValue, isPrivacyModeEnabledByDefault } from '../utils/privacy';

export function WarpAccountsPage() {
  const { t, i18n } = useTranslation();
  const locale = i18n.language || 'zh-CN';

  const {
    accounts,
    loading,
    fetchAccounts,
    deleteAccounts,
    updateAccountTags,
  } = useWarpAccountStore();

  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [viewMode, setViewMode] = useState<'grid' | 'list'>('grid');
  const [privacyModeEnabled] = useState(isPrivacyModeEnabledByDefault);
  const [showTagModal, setShowTagModal] = useState<string | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState<{ ids: string[]; message: string } | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [copiedToken, setCopiedToken] = useState<string | null>(null);

  useEffect(() => {
    fetchAccounts();
  }, [fetchAccounts]);

  const handleDelete = (accountId: string) => {
    setDeleteConfirm({
      ids: [accountId],
      message: t('messages.deleteConfirm', '确定要删除此账号吗？'),
    });
  };

  const handleBatchDelete = () => {
    if (selected.size === 0) return;
    setDeleteConfirm({
      ids: Array.from(selected),
      message: t('messages.batchDeleteConfirm', { count: selected.size }),
    });
  };

  const confirmDelete = async () => {
    if (!deleteConfirm || deleting) return;
    setDeleting(true);
    try {
      await deleteAccounts(deleteConfirm.ids);
      setSelected((prev) => {
        const next = new Set(prev);
        deleteConfirm.ids.forEach((id) => next.delete(id));
        return next;
      });
      setDeleteConfirm(null);
    } finally {
      setDeleting(false);
    }
  };

  const toggleSelect = (id: string) => {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setSelected(next);
  };

  const handleCopyToken = async (id: string, token: string) => {
    try {
      await navigator.clipboard.writeText(token);
      setCopiedToken(id);
      setTimeout(() => setCopiedToken(null), 2000);
    } catch (e) {
      console.error('Copy failed', e);
    }
  };

  const formatDate = (timestamp: number) => {
    const d = new Date(timestamp * 1000);
    return d.toLocaleDateString(locale) + ' ' + d.toLocaleTimeString(locale);
  };

  return (
    <div className="layout-content warp-accounts-page">
      <div className="main-content">
        <header className="page-header">
          <div className="header-left">
            <h1>Warp {t('nav.accounts')}</h1>
            <span className="account-count">
              {accounts.length} {t('accounts.total')}
            </span>
          </div>
          <div className="header-actions">
            <button className="icon-btn" onClick={fetchAccounts} disabled={loading} title={t('common.refresh')}>
              <RefreshCw size={18} className={loading ? 'animate-spin' : ''} />
            </button>
            <div className="view-controls">
              <button className={`view-btn ${viewMode === 'list' ? 'active' : ''}`} onClick={() => setViewMode('list')} title={t('common.listView')}>
                <List size={18} />
              </button>
              <button className={`view-btn ${viewMode === 'grid' ? 'active' : ''}`} onClick={() => setViewMode('grid')} title={t('common.gridView')}>
                <LayoutGrid size={18} />
              </button>
            </div>
            {selected.size > 0 && (
              <button className="btn btn-danger" onClick={handleBatchDelete}>
                <Trash2 size={16} />
                {t('common.delete')} ({selected.size})
              </button>
            )}
          </div>
        </header>

        {loading && accounts.length === 0 ? (
          <div className="loading-state">
            <RefreshCw className="animate-spin" size={24} />
            <p>{t('common.loading', 'Loading...')}</p>
          </div>
        ) : accounts.length === 0 ? (
          <div className="empty-state">
            <div className="empty-icon"></div>
            <h3>{t('accounts.noAccounts')}</h3>
            <p>由于 Warp 通过特定签名认证，目前暂只支持从接口导入或自动捕捉。</p>
          </div>
        ) : (
          <div className={`accounts-${viewMode}`}>
            {accounts.map((account: any) => (
              <div key={account.id} className={`account-card ${selected.has(account.id) ? 'selected' : ''}`}>
                <div className="card-header">
                  <div className="card-title-group">
                    <input type="checkbox" checked={selected.has(account.id)} onChange={() => toggleSelect(account.id)} className="account-checkbox" />
                    <h3 className="account-email" title={account.email || account.id}>
                      {maskSensitiveValue(account.email || account.id, privacyModeEnabled)}
                    </h3>
                  </div>
                  <div className="card-actions">
                    <button className="icon-btn" onClick={() => setShowTagModal(account.id)} title={t('common.editTags')}>
                      <Tag size={16} />
                    </button>
                    <button className="icon-btn danger" onClick={() => handleDelete(account.id)} title={t('common.delete')}>
                      <Trash2 size={16} />
                    </button>
                  </div>
                </div>

                <div className="card-body">
                  <div className="info-row">
                    <span className="info-label">Token:</span>
                    <span className="info-value token-value">
                      {maskSensitiveValue(account.auth_token, privacyModeEnabled)}
                    </span>
                    <button className={`icon-btn small ${copiedToken === account.id ? 'success' : ''}`} onClick={() => handleCopyToken(account.id, account.auth_token)}>
                      <Copy size={12} />
                    </button>
                  </div>

                  {account.plan_type && (
                    <div className="info-row">
                      <span className="info-label">Plan:</span>
                      <span className="info-value">{account.plan_type}</span>
                    </div>
                  )}

                  <div className="info-row">
                    <span className="info-label">{t('common.lastUsed')}:</span>
                    <span className="info-value">{formatDate(account.last_used)}</span>
                  </div>

                  {account.tags && account.tags.length > 0 && (
                    <div className="account-tags">
                      {account.tags.map((tag: string) => (
                        <span key={tag} className="account-tag">{tag as string}</span>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}

        {deleteConfirm && (
          <div className="modal-overlay">
            <div className="modal confirm-modal">
              <div className="modal-header">
                <h3>{t('common.warning')}</h3>
              </div>
              <div className="modal-content">
                <p>{deleteConfirm.message}</p>
              </div>
              <div className="modal-footer">
                <button className="btn btn-ghost" onClick={() => setDeleteConfirm(null)} disabled={deleting}>
                  {t('common.cancel')}
                </button>
                <button className="btn btn-danger" onClick={confirmDelete} disabled={deleting}>
                  {deleting ? <RefreshCw className="animate-spin" size={16} /> : <Trash2 size={16} />}
                  {t('common.delete')}
                </button>
              </div>
            </div>
          </div>
        )}

        {showTagModal && (
          <TagEditModal
            isOpen={true}
            initialTags={accounts.find((a: any) => a.id === showTagModal)?.tags || []}
            onSave={async (tags: string[]) => {
              await updateAccountTags(showTagModal, tags);
              setShowTagModal(null);
            }}
            onClose={() => setShowTagModal(null)}
          />
        )}
      </div>
    </div>
  );
}
