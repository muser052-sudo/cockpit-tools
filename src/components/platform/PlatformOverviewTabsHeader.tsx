import { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Github, Layers } from 'lucide-react';
import { CodexIcon } from '../icons/CodexIcon';
import { WindsurfIcon } from '../icons/WindsurfIcon';
import { KiroIcon } from '../icons/KiroIcon';

export type PlatformOverviewTab = 'overview' | 'instances';
export type PlatformOverviewHeaderId = 'codex' | 'github-copilot' | 'windsurf' | 'kiro';

interface PlatformOverviewTabsHeaderProps {
  platform: PlatformOverviewHeaderId;
  active: PlatformOverviewTab;
  onTabChange?: (tab: PlatformOverviewTab) => void;
}

interface PlatformOverviewConfig {
  titleKey: string;
  titleDefault: string;
  overviewTabKey: string;
  overviewTabDefault: string;
  instancesTabKey: string;
  instancesTabDefault: string;
  overviewSubtitleKey: string;
  overviewSubtitleDefault: string;
  instancesSubtitleKey: string;
  instancesSubtitleDefault: string;
  overviewIcon: ReactNode;
}

interface TabSpec {
  key: PlatformOverviewTab;
  label: string;
  icon: ReactNode;
}

const CONFIGS: Record<PlatformOverviewHeaderId, PlatformOverviewConfig> = {
  codex: {
    titleKey: 'codex.title',
    titleDefault: 'Codex 账号管理',
    overviewTabKey: 'overview.title',
    overviewTabDefault: '账号总览',
    instancesTabKey: 'instances.title',
    instancesTabDefault: '多开实例',
    overviewSubtitleKey: 'codex.subtitle',
    overviewSubtitleDefault: '实时监控所有Codex账号的模型配额状态。',
    instancesSubtitleKey: 'codex.instances.subtitle',
    instancesSubtitleDefault: '多实例独立配置，多账号并行运行。',
    overviewIcon: <CodexIcon className="tab-icon" />,
  },
  'github-copilot': {
    titleKey: 'githubCopilot.title',
    titleDefault: 'GitHub Copilot 账号管理',
    overviewTabKey: 'githubCopilot.overview.title',
    overviewTabDefault: '账号总览',
    instancesTabKey: 'githubCopilot.instances.title',
    instancesTabDefault: '多开实例',
    overviewSubtitleKey: 'githubCopilot.subtitle',
    overviewSubtitleDefault: '实时监控所有账号的配额状态。',
    instancesSubtitleKey: 'githubCopilot.instances.subtitle',
    instancesSubtitleDefault: '多实例独立配置，多账号并行运行。',
    overviewIcon: <Github className="tab-icon" />,
  },
  windsurf: {
    titleKey: 'windsurf.title',
    titleDefault: 'Windsurf 账号管理',
    overviewTabKey: 'windsurf.overview.title',
    overviewTabDefault: '账号总览',
    instancesTabKey: 'windsurf.instances.title',
    instancesTabDefault: '多开实例',
    overviewSubtitleKey: 'windsurf.subtitle',
    overviewSubtitleDefault: '实时监控所有账号的配额状态。',
    instancesSubtitleKey: 'windsurf.instances.subtitle',
    instancesSubtitleDefault: '多实例独立配置，多账号并行运行。',
    overviewIcon: <WindsurfIcon className="tab-icon" />,
  },
  kiro: {
    titleKey: 'kiro.title',
    titleDefault: 'Kiro 账号管理',
    overviewTabKey: 'kiro.overview.title',
    overviewTabDefault: '账号总览',
    instancesTabKey: 'kiro.instances.title',
    instancesTabDefault: '多开实例',
    overviewSubtitleKey: 'kiro.subtitle',
    overviewSubtitleDefault: '实时监控所有账号的配额状态。',
    instancesSubtitleKey: 'kiro.instances.subtitle',
    instancesSubtitleDefault: '多实例独立配置，多账号并行运行。',
    overviewIcon: <KiroIcon className="tab-icon" />,
  },
};

export function PlatformOverviewTabsHeader({
  platform,
  active,
  onTabChange,
}: PlatformOverviewTabsHeaderProps) {
  const { t } = useTranslation();
  const config = CONFIGS[platform];
  const tabs: TabSpec[] = [
    {
      key: 'overview',
      label: t(config.overviewTabKey, config.overviewTabDefault),
      icon: config.overviewIcon,
    },
    {
      key: 'instances',
      label: t(config.instancesTabKey, config.instancesTabDefault),
      icon: <Layers className="tab-icon" />,
    },
  ];

  const subtitle =
    active === 'instances'
      ? t(config.instancesSubtitleKey, config.instancesSubtitleDefault)
      : t(config.overviewSubtitleKey, config.overviewSubtitleDefault);

  return (
    <>
      <div className="page-header">
        <div className="page-title">{t(config.titleKey, config.titleDefault)}</div>
        <div className="page-subtitle">{subtitle}</div>
      </div>
      <div className="page-tabs-row page-tabs-center">
        <div className="page-tabs filter-tabs">
          {tabs.map((tab) => (
            <button
              key={tab.key}
              className={`filter-tab${active === tab.key ? ' active' : ''}`}
              onClick={() => onTabChange?.(tab.key)}
            >
              {tab.icon}
              <span>{tab.label}</span>
            </button>
          ))}
        </div>
      </div>
    </>
  );
}
