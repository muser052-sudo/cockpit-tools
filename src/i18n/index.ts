import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';

import zhCN from '../locales/zh-CN.json';
import zhTW from '../locales/zh-tw.json';
import en from '../locales/en.json';
import ja from '../locales/ja.json';
import es from '../locales/es.json';
import de from '../locales/de.json';
import fr from '../locales/fr.json';
import ptBR from '../locales/pt-br.json';
import ru from '../locales/ru.json';
import ko from '../locales/ko.json';
import it from '../locales/it.json';
import tr from '../locales/tr.json';
import pl from '../locales/pl.json';
import cs from '../locales/cs.json';
import vi from '../locales/vi.json';
import ar from '../locales/ar.json';

const languageAliases: Record<string, string> = {
  'zh-CN': 'zh-cn',
  'zh-TW': 'zh-tw',
  'en-US': 'en',
  'pt-BR': 'pt-br',
  'vi-VN': 'vi',
  'vi-vn': 'vi',
};

export const supportedLanguages = [
  'en',
  'zh-cn',
  'zh-tw',
  'ja',
  'es',
  'de',
  'fr',
  'pt-br',
  'ru',
  'ko',
  'it',
  'tr',
  'pl',
  'cs',
  'vi',
  'ar',
];

export function normalizeLanguage(lang: string): string {
  const trimmed = lang.trim();
  if (!trimmed) {
    return 'zh-cn';
  }

  if (languageAliases[trimmed]) {
    return languageAliases[trimmed];
  }

  const lower = trimmed.toLowerCase();
  if (languageAliases[lower]) {
    return languageAliases[lower];
  }

  return lower;
}

// 从 localStorage 读取语言设置，默认中文
const savedLanguage = normalizeLanguage(localStorage.getItem('app-language') || 'zh-CN');

i18n
  .use(initReactI18next)
  .init({
    resources: {
      'zh-cn': { translation: zhCN },
      'zh-CN': { translation: zhCN },
      'zh-tw': { translation: zhTW },
      'zh-TW': { translation: zhTW },
      'en': { translation: en },
      'en-US': { translation: en },
      'ja': { translation: ja },
      'es': { translation: es },
      'de': { translation: de },
      'fr': { translation: fr },
      'pt-br': { translation: ptBR },
      'pt-BR': { translation: ptBR },
      'ru': { translation: ru },
      'ko': { translation: ko },
      'it': { translation: it },
      'tr': { translation: tr },
      'pl': { translation: pl },
      'cs': { translation: cs },
      'vi': { translation: vi },
      'ar': { translation: ar },
    },
    lng: savedLanguage,
    fallbackLng: 'zh-cn',
    interpolation: {
      escapeValue: false, // React 已经处理了 XSS
    },
  });

/**
 * 切换语言
 */
export function changeLanguage(lang: string) {
  const normalized = normalizeLanguage(lang);
  i18n.changeLanguage(normalized);
  localStorage.setItem('app-language', normalized);
}

/**
 * 获取当前语言
 */
export function getCurrentLanguage(): string {
  return normalizeLanguage(i18n.language || 'zh-CN');
}

export default i18n;
