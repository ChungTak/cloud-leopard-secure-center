import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';

export const defaultNS = 'common';

export const resources = {
  'zh-CN': {
    common: {
      appTitle: 'Cloud Leopard Secure Center',
      login: '登录',
      username: '用户名',
      password: '密码',
      submit: '提交',
      dashboard: '仪表盘',
      tenants: '租户',
      users: '用户',
      settings: '设置',
      mainNavigation: '主导航',
      breadcrumb: '面包屑',
      logout: '退出',
      notFound: '页面不存在',
      forbidden: '无权访问',
      serverError: '服务器错误',
      backHome: '返回首页',
      breadcrumbHome: '首页',
      retry: '重试',
    },
  },
  'en-US': {
    common: {
      appTitle: 'Cloud Leopard Secure Center',
      login: 'Login',
      username: 'Username',
      password: 'Password',
      submit: 'Submit',
      dashboard: 'Dashboard',
      tenants: 'Tenants',
      users: 'Users',
      settings: 'Settings',
      mainNavigation: 'Main navigation',
      breadcrumb: 'Breadcrumb',
      logout: 'Logout',
      notFound: 'Page not found',
      forbidden: 'Forbidden',
      serverError: 'Server error',
      backHome: 'Back to home',
      breadcrumbHome: 'Home',
      retry: 'Retry',
    },
  },
} as const;

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources,
    fallbackLng: 'zh-CN',
    defaultNS,
    interpolation: { escapeValue: false },
    detection: {
      order: ['localStorage'],
      caches: ['localStorage'],
    },
    react: {
      useSuspense: false,
    },
  });

export default i18n;
