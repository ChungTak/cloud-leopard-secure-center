import type { ReactNode } from 'react';
import { Outlet } from 'react-router-dom';
import { ConfigProvider } from '@douyinfe/semi-ui';
import zh_CN from '@douyinfe/semi-ui/lib/es/locale/source/zh_CN';
import { ThemeProvider } from '@clsc/ui';

export default function Root(): ReactNode {
  return (
    <ConfigProvider locale={zh_CN}>
      <ThemeProvider>
        <Outlet />
      </ThemeProvider>
    </ConfigProvider>
  );
}
