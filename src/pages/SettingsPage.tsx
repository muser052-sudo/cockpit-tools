import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { openUrl } from '@tauri-apps/plugin-opener';
import { invoke } from '@tauri-apps/api/core';
import { changeLanguage, getCurrentLanguage, normalizeLanguage } from '../i18n';
import * as accountService from '../services/accountService';
import './settings/Settings.css';
import { 
  Github, User, Rocket, Save, FolderOpen,
  AlertCircle
} from 'lucide-react';

const APP_VERSION = "v0.1.0";

/** 网络配置类型 */
interface NetworkConfig {
  ws_enabled: boolean;
  ws_port: number;
  actual_port: number | null;
  default_port: number;
}

/** 通用配置类型 */
interface GeneralConfig {
  language: string;
  theme: string;
  auto_refresh_minutes: number;
}

export function SettingsPage() {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<'general' | 'network' | 'about'>('general');

  const languageOptions = [
    { value: 'zh-cn', label: '简体中文' },
    { value: 'zh-tw', label: '繁體中文' },
    { value: 'en', label: 'English' },
    { value: 'ja', label: '日本語' },
    { value: 'ko', label: '한국어' },
    { value: 'de', label: 'Deutsch' },
    { value: 'fr', label: 'Français' },
    { value: 'es', label: 'Español' },
    { value: 'pt-br', label: 'Português (Brasil)' },
    { value: 'ru', label: 'Русский' },
    { value: 'it', label: 'Italiano' },
    { value: 'tr', label: 'Türkçe' },
    { value: 'pl', label: 'Polski' },
    { value: 'cs', label: 'Čeština' },
    { value: 'vi', label: 'Tiếng Việt' },
    { value: 'ar', label: 'العربية' },
  ];
  
  // General Settings States
  const [language, setLanguage] = useState(getCurrentLanguage());
  const [theme, setTheme] = useState('system');
  const [autoRefresh, setAutoRefresh] = useState('10');
  const [generalLoaded, setGeneralLoaded] = useState(false);
  const generalSaveTimerRef = useRef<number | null>(null);
  const suppressGeneralSaveRef = useRef(false);
  
  // Network States
  const [wsEnabled, setWsEnabled] = useState(true);
  const [wsPort, setWsPort] = useState('19528');
  const [actualPort, setActualPort] = useState<number | null>(null);
  const [defaultPort, setDefaultPort] = useState(19528);
  const [needsRestart, setNeedsRestart] = useState(false);
  const [networkSaving, setNetworkSaving] = useState(false);
  
  // 加载配置
  useEffect(() => {
    loadGeneralConfig();
    loadNetworkConfig();
  }, []);
  
  useEffect(() => {
    if (!generalLoaded) {
      return;
    }
    changeLanguage(language);
    applyTheme(theme);
  }, [generalLoaded, language, theme]);

  useEffect(() => {
    if (!generalLoaded) {
      return;
    }

    if (generalSaveTimerRef.current) {
      window.clearTimeout(generalSaveTimerRef.current);
    }

    if (!autoRefresh.trim()) {
      return;
    }

    const autoRefreshNum = parseInt(autoRefresh, 10) || -1;

    if (suppressGeneralSaveRef.current) {
      suppressGeneralSaveRef.current = false;
      return;
    }

    generalSaveTimerRef.current = window.setTimeout(async () => {
      try {
        await invoke('save_general_config', {
          language,
          theme,
          autoRefreshMinutes: autoRefreshNum,
        });
        window.dispatchEvent(new Event('config-updated'));
      } catch (err) {
        console.error('保存通用配置失败:', err);
        alert(`${t('settings.network.saveFailed').replace('{error}', String(err))}`);
      }
    }, 300);

    return () => {
      if (generalSaveTimerRef.current) {
        window.clearTimeout(generalSaveTimerRef.current);
      }
    };
  }, [autoRefresh, generalLoaded, language, theme, t]);

  useEffect(() => {
    const handleLanguageUpdated = (event: Event) => {
      const detail = (event as CustomEvent<{ language?: string }>).detail;
      if (!detail?.language) {
        return;
      }
      suppressGeneralSaveRef.current = true;
      setLanguage(detail.language);
    };

    window.addEventListener('general-language-updated', handleLanguageUpdated);
    return () => {
      window.removeEventListener('general-language-updated', handleLanguageUpdated);
    };
  }, []);
  
  const applyTheme = (newTheme: string) => {
    if (newTheme === 'system') {
      const isDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
      document.documentElement.setAttribute('data-theme', isDark ? 'dark' : 'light');
    } else {
      document.documentElement.setAttribute('data-theme', newTheme);
    }
  };
  
  const loadGeneralConfig = async () => {
    try {
      const config = await invoke<GeneralConfig>('get_general_config');
      setLanguage(normalizeLanguage(config.language));
      setTheme(config.theme);
      setAutoRefresh(String(config.auto_refresh_minutes));
      // 同步语言
      changeLanguage(config.language);
      applyTheme(config.theme);
      setGeneralLoaded(true);
    } catch (err) {
      console.error('加载通用配置失败:', err);
    }
  };
  
  const loadNetworkConfig = async () => {
    try {
      const config = await invoke<NetworkConfig>('get_network_config');
      setWsEnabled(config.ws_enabled);
      setWsPort(String(config.ws_port));
      setActualPort(config.actual_port);
      setDefaultPort(config.default_port);
      setNeedsRestart(false);
    } catch (err) {
      console.error('加载网络配置失败:', err);
    }
  };
  
  // 保存网络配置
  const handleSaveNetworkConfig = async () => {
    setNetworkSaving(true);
    try {
      const portNum = parseInt(wsPort, 10) || defaultPort;
      const result = await invoke<boolean>('save_network_config', {
        wsEnabled,
        wsPort: portNum,
      });
      
      if (result) {
        setNeedsRestart(true);
        alert(t('settings.network.saveSuccessRestart'));
      } else {
        alert(t('settings.network.saveSuccess'));
      }
    } catch (err) {
      alert(t('settings.network.saveFailed').replace('{error}', String(err)));
    } finally {
      setNetworkSaving(false);
    }
  };

  const openLink = (url: string) => {
    openUrl(url);
  };

  return (
    <main className="main-content">
      <section className="page-heading">
        <div>
          <h1>{t('settings.title')}</h1>
          <p>{t('settings.subtitle')}</p>
        </div>
      </section>
      
      {/* 1. Tab Navigation */}
      <div className="settings-tabs-wrapper">
        <div className="settings-tabs">
          <button 
            className={`settings-tab ${activeTab === 'general' ? 'active' : ''}`}
            onClick={() => setActiveTab('general')}
          >
            {t('settings.tabs.general')}
          </button>
          <button 
            className={`settings-tab ${activeTab === 'network' ? 'active' : ''}`}
            onClick={() => setActiveTab('network')}
          >
            {t('settings.tabs.network')}
          </button>
          <button 
            className={`settings-tab ${activeTab === 'about' ? 'active' : ''}`}
            onClick={() => setActiveTab('about')}
          >
            {t('settings.tabs.about')}
          </button>
        </div>
      </div>

      {/* 2. Content Area */}
      <div className="settings-container">
        <div className="settings-content">
        {/* === General Tab === */}
        {activeTab === 'general' && (
          <>
            <div className="settings-group">
              <div className="settings-row">
                <div className="row-label">
                  <div className="row-title">{t('settings.general.language')}</div>
                  <div className="row-desc">{t('settings.general.languageDesc')}</div>
                </div>
                <div className="row-control">
                  <select 
                    className="settings-select" 
                    value={language} 
                    onChange={(e) => setLanguage(normalizeLanguage(e.target.value))}
                  >
                    {languageOptions.map((option) => (
                      <option key={option.value} value={option.value}>{option.label}</option>
                    ))}
                  </select>
                </div>
              </div>

              <div className="settings-row">
                <div className="row-label">
                  <div className="row-title">{t('settings.general.theme')}</div>
                  <div className="row-desc">{t('settings.general.themeDesc')}</div>
                </div>
                <div className="row-control">
                  <select 
                    className="settings-select" 
                    value={theme} 
                    onChange={(e) => setTheme(e.target.value)}
                  >
                    <option value="light">{t('settings.general.themeLight')}</option>
                    <option value="dark">{t('settings.general.themeDark')}</option>
                    <option value="system">{t('settings.general.themeSystem')}</option>
                  </select>
                </div>
              </div>
            </div>

            <div className="group-title">{t('settings.general.accountManagement')}</div>
            <div className="settings-group">
              <div className="settings-row">
                <div className="row-label">
                  <div className="row-title">{t('settings.general.autoRefresh')}</div>
                  <div className="row-desc">{t('settings.general.autoRefreshDesc')}</div>
                </div>
                <div className="row-control">
                  <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                    <select 
                      className="settings-select" 
                      style={{ minWidth: '120px', width: 'auto' }}
                      value={['-1', '2', '5', '10', '15'].includes(autoRefresh) ? autoRefresh : 'custom'} 
                      onChange={(e) => {
                        const val = e.target.value;
                        if (val === 'custom') {
                          if (['-1', '2', '5', '10', '15'].includes(autoRefresh)) {
                            setAutoRefresh('12');
                          }
                        } else {
                          setAutoRefresh(val);
                        }
                      }}
                    >
                      <option value="-1">{t('settings.general.autoRefreshDisabled')}</option>
                      <option value="2">2 {t('settings.general.minutes')}</option>
                      <option value="5">5 {t('settings.general.minutes')}</option>
                      <option value="10">10 {t('settings.general.minutes')}</option>
                      <option value="15">15 {t('settings.general.minutes')}</option>
                      <option value="custom">{t('settings.general.autoRefreshCustom')}</option>
                    </select>
                    
                    {!['-1', '2', '5', '10', '15'].includes(autoRefresh) && (
                      <div style={{ position: 'relative', display: 'flex', alignItems: 'center' }}>
                        <input 
                          type="number" 
                          min="1"
                          className="settings-input"
                          style={{ width: '80px', paddingRight: '24px' }}
                          value={autoRefresh}
                          onChange={(e) => setAutoRefresh(e.target.value)}
                        />
                        <span style={{ position: 'absolute', right: '8px', fontSize: '12px', color: 'var(--text-muted)' }}>
                          {t('settings.general.minutes')}
                        </span>
                      </div>
                    )}
                  </div>
                </div>
              </div>
            </div>

            <div className="group-title">{t('settings.general.storageTitle')}</div>
            <div className="settings-group">
              <div className="settings-row">
                <div className="row-label">
                  <div className="row-title">{t('settings.general.dataDir')}</div>
                  <div className="row-desc">{t('settings.general.dataDirDesc')}</div>
                </div>
                <div className="row-control">
                  <button className="btn btn-secondary" onClick={() => accountService.openDataFolder()}>
                    <FolderOpen size={16} />{t('common.open')}
                  </button>
                </div>
              </div>
              <div className="settings-row">
                <div className="row-label">
                  <div className="row-title">{t('settings.general.fpDir')}</div>
                  <div className="row-desc">{t('settings.general.fpDirDesc')}</div>
                </div>
                <div className="row-control">
                  <button className="btn btn-secondary" onClick={() => accountService.openDeviceFolder()}>
                    <FolderOpen size={16} />{t('common.open')}
                  </button>
                </div>
              </div>
            </div>

          </>
        )}

        {/* === Network Tab === */}
        {activeTab === 'network' && (
          <>
            <div className="group-title">{t('settings.network.apiTitle')}</div>
            <div className="settings-group">
              <div className="settings-row">
                <div className="row-label">
                  <div className="row-title">{t('settings.network.wsService')}</div>
                  <div className="row-desc">{t('settings.network.wsServiceDesc')}</div>
                </div>
                <div className="row-control">
                  <label className="switch">
                    <input 
                      type="checkbox" 
                      checked={wsEnabled} 
                      onChange={(e) => setWsEnabled(e.target.checked)} 
                    />
                    <span className="slider"></span>
                  </label>
                </div>
              </div>

              {wsEnabled && (
                <>
                  <div className="settings-row" style={{ animation: 'fadeUp 0.3s ease both' }}>
                    <div className="row-label">
                      <div className="row-title">{t('settings.network.preferredPort')}</div>
                      <div className="row-desc">
                        {t('settings.network.preferredPortDesc').replace('{port}', String(defaultPort))}
                      </div>
                    </div>
                    <div className="row-control">
                      <input 
                        type="number" 
                        className="settings-input"
                        value={wsPort}
                        onChange={(e) => setWsPort(e.target.value)}
                        placeholder={String(defaultPort)}
                        min="1024"
                        max="65535"
                      />
                    </div>
                  </div>
                  
                  {actualPort && (
                    <div className="settings-row" style={{ animation: 'fadeUp 0.3s ease both' }}>
                      <div className="row-label">
                        <div className="row-title">{t('settings.network.currentPort')}</div>
                        <div className="row-desc">
                          {actualPort === parseInt(wsPort, 10) 
                            ? t('settings.network.portNormal')
                            : t('settings.network.portFallback')
                                .replace('{configured}', wsPort)
                                .replace('{actual}', String(actualPort))}
                        </div>
                      </div>
                      <div className="row-control">
                        <span style={{ 
                          fontFamily: 'var(--font-mono)', 
                          fontSize: '14px',
                          color: actualPort === parseInt(wsPort, 10) ? 'var(--accent)' : 'var(--warning, #f59e0b)'
                        }}>
                          ws://127.0.0.1:{actualPort}
                        </span>
                      </div>
                    </div>
                  )}
                </>
              )}
            </div>
            
            {needsRestart && (
              <div style={{ 
                display: 'flex', 
                alignItems: 'center', 
                gap: '8px', 
                padding: '12px 16px',
                marginTop: '12px',
                background: 'rgba(245, 158, 11, 0.1)',
                borderRadius: '8px',
                color: 'var(--warning, #f59e0b)',
                fontSize: '14px'
              }}>
                <AlertCircle size={18} />
                {t('settings.network.restartRequired')}
              </div>
            )}

            <div style={{ display: 'flex', justifyContent: 'flex-end', marginTop: '12px' }}>
                <button 
                  className="btn btn-primary" 
                  onClick={handleSaveNetworkConfig}
                  disabled={networkSaving}
                >
                    <Save size={16} /> {networkSaving ? t('common.saving') : t('settings.saveSettings')}
                </button>
            </div>
          </>
        )}

        {/* === About Tab === */}
        {activeTab === 'about' && (
          <div className="about-container">
            <div className="about-logo-section">
              <div className="app-icon-squircle">
                <Rocket size={40} />
              </div>
              <div className="app-info">
                <h2>{t('settings.about.appName')}</h2>
                <div className="version-tag">{APP_VERSION}</div>
              </div>
              <p style={{ color: 'var(--text-secondary)', fontSize: '14px' }}>
                {t('settings.about.slogan')}
              </p>
            </div>

            <div className="credits-list">
              <button className="credit-item" onClick={() => openLink('https://github.com/jlcodes99')}>
                <div className="credit-icon"><User size={24} /></div>
                <h3>{t('settings.about.author')}</h3>
                <p>jlcodes99</p>
              </button>
              
              <button className="credit-item" onClick={() => openLink('https://github.com/jlcodes99/antigravity-cockpit-tools')}>
                <div className="credit-icon" style={{ color: '#0f172a' }}><Github size={24} /></div>
                <h3>{t('settings.about.github')}</h3>
                <p>antigravity-cockpit-tools</p>
              </button>
            </div>
          </div>
        )}
        </div>
      </div>
    </main>
  );
}
