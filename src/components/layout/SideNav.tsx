import { LayoutGrid, Fingerprint, Settings, Rocket, AlarmClock } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useState, useRef, useCallback } from 'react';
import { Page } from '../../types/navigation';

interface FlyingRocket {
  id: number;
  x: number;
  y: number;
}

interface SideNavProps {
  page: Page;
  setPage: (page: Page) => void;
}

export function SideNav({ page, setPage }: SideNavProps) {
  const { t } = useTranslation();
  const [clickCount, setClickCount] = useState(0);
  const [flyingRockets, setFlyingRockets] = useState<FlyingRocket[]>([]);
  const resetTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const rocketIdRef = useRef(0);
  const logoRef = useRef<HTMLDivElement>(null);

  const handleLogoClick = useCallback(() => {
    // 清除之前的重置计时器
    if (resetTimerRef.current) {
      clearTimeout(resetTimerRef.current);
    }

    // 增加点击计数
    setClickCount(prev => prev + 1);

    // 创建新的飞行火箭
    const newRocket: FlyingRocket = {
      id: rocketIdRef.current++,
      x: (Math.random() - 0.5) * 40, // 随机水平偏移
      y: 0,
    };
    
    setFlyingRockets(prev => [...prev, newRocket]);

    // 动画完成后移除火箭 (1.5秒)
    setTimeout(() => {
      setFlyingRockets(prev => prev.filter(r => r.id !== newRocket.id));
    }, 1500);

    // 设置新的重置计时器 (2秒不点击后重置)
    resetTimerRef.current = setTimeout(() => {
      setClickCount(0);
    }, 2000);
  }, []);

  return (
    <nav className="side-nav">
      <div className="nav-brand" style={{ position: 'relative', zIndex: 10 }}>
         <div 
           ref={logoRef}
           className="brand-logo rocket-easter-egg" 
           onClick={handleLogoClick}
         >
           <Rocket size={20} />
           {/* 点击计数器保持在里面，跟随缩放 */}
           {clickCount > 0 && (
             <span className="rocket-click-count">{clickCount}</span>
           )}
         </div>

         {/* 把火箭层移到外面，放在后面以自然层叠在上方，使用 pointer-events-none 防止遮挡点击 */}
         <div style={{ position: 'absolute', top: 0, left: 0, width: '100%', height: '100%', pointerEvents: 'none' }}>
           {flyingRockets.map(rocket => (
             <span 
               key={rocket.id} 
               className="flying-rocket"
               style={{ '--rocket-x': `${rocket.x}px` } as React.CSSProperties}
             >
               🚀
             </span>
           ))}
         </div>
      </div>
      
      <div className="nav-items">
        <button 
          className={`nav-item ${page === 'overview' ? 'active' : ''}`} 
          onClick={() => setPage('overview')}
          title={t('nav.overview')}
        >
          <LayoutGrid size={20} />
          <span className="tooltip">{t('nav.overview')}</span>
        </button>
        
        <button 
          className={`nav-item ${page === 'fingerprints' ? 'active' : ''}`} 
          onClick={() => setPage('fingerprints')}
          title={t('nav.fingerprints')}
        >
          <Fingerprint size={20} />
          <span className="tooltip">{t('nav.fingerprints')}</span>
        </button>

        <button 
          className={`nav-item ${page === 'wakeup' ? 'active' : ''}`} 
          onClick={() => setPage('wakeup')}
          title={t('nav.wakeup')}
        >
          <AlarmClock size={20} />
          <span className="tooltip">{t('nav.wakeup')}</span>
        </button>
        
        <button 
          className={`nav-item ${page === 'settings' ? 'active' : ''}`} 
          onClick={() => setPage('settings')}
          title={t('nav.settings')}
        >
          <Settings size={20} />
          <span className="tooltip">{t('nav.settings')}</span>
        </button>
      </div>

    </nav>
  );
}
