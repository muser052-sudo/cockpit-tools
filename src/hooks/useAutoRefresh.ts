import { useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useAccountStore } from '../stores/useAccountStore';

interface GeneralConfig {
  language: string;
  theme: string;
  auto_refresh_minutes: number;
}

export function useAutoRefresh() {
  const { refreshAllQuotas, syncCurrentFromClient } = useAccountStore();
  const intervalRef = useRef<number | null>(null);

  const setupAutoRefresh = async () => {
    try {
      const config = await invoke<GeneralConfig>('get_general_config');
      
      // 清除旧的定时器
      if (intervalRef.current) {
        window.clearInterval(intervalRef.current);
        intervalRef.current = null;
      }

      if (config.auto_refresh_minutes > 0) {
        console.log(`[AutoRefresh] 已启用: 每 ${config.auto_refresh_minutes} 分钟`);
        
        const ms = config.auto_refresh_minutes * 60 * 1000;
        
        intervalRef.current = window.setInterval(async () => {
          console.log('[AutoRefresh] 触发定时配额刷新...');
          try {
            // 先尝试同步本地客户端的当前账号
            await syncCurrentFromClient();
            
            // 然后刷新配额
            await refreshAllQuotas();
          } catch (e) {
            console.error('[AutoRefresh] 刷新失败:', e);
          }
        }, ms);
      } else {
        console.log('[AutoRefresh] 已禁用');
      }
    } catch (err) {
      console.error('[AutoRefresh] 加载配置失败:', err);
    }
  };

  useEffect(() => {
    // 初始设置
    setupAutoRefresh();

    // 监听配置变更事件
    const handleConfigUpdate = () => {
      console.log('[AutoRefresh] 检测到配置变更，重新设置定时器');
      setupAutoRefresh();
    };

    window.addEventListener('config-updated', handleConfigUpdate);

    return () => {
      if (intervalRef.current) {
        window.clearInterval(intervalRef.current);
      }
      window.removeEventListener('config-updated', handleConfigUpdate);
    };
  }, [refreshAllQuotas]);
}
