import { useEffect, useState } from 'react';
import './App.css';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { AccountsPage } from './pages/AccountsPage';
import { FingerprintsPage } from './pages/FingerprintsPage';
import { WakeupTasksPage } from './pages/WakeupTasksPage';
import { SettingsPage } from './pages/SettingsPage';
import { SideNav } from './components/layout/SideNav';
import { Page } from './types/navigation';
import { useAutoRefresh } from './hooks/useAutoRefresh';
import { changeLanguage, getCurrentLanguage, normalizeLanguage } from './i18n';

function App() {
  const [page, setPage] = useState<Page>('overview');
  
  // 启用自动刷新 hook
  useAutoRefresh();

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    listen<string>('settings:language_changed', (event) => {
      const nextLanguage = normalizeLanguage(String(event.payload || ''));
      if (!nextLanguage || nextLanguage === getCurrentLanguage()) {
        return;
      }
      changeLanguage(nextLanguage);
      window.dispatchEvent(new CustomEvent('general-language-updated', { detail: { language: nextLanguage } }));
    }).then((fn) => { unlisten = fn; });

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  // 窗口拖拽处理
  const handleDragStart = () => {
    getCurrentWindow().startDragging();
  };

  return (
    <div className="app-container">
      {/* 顶部固定拖拽区域 */}
      <div 
        className="drag-region"
        data-tauri-drag-region 
        onMouseDown={handleDragStart}
      />
      
      {/* 左侧悬浮导航 */}
      <SideNav page={page} setPage={setPage} />

      <div className="main-wrapper">
        {/* overview 现在是合并后的账号总览页面 */}
        {page === 'overview' && <AccountsPage onNavigate={setPage} />}
        {page === 'fingerprints' && <FingerprintsPage />}
        {page === 'wakeup' && <WakeupTasksPage onNavigate={setPage} />}
        {page === 'settings' && <SettingsPage />}
      </div>
    </div>
  );
}

export default App;
