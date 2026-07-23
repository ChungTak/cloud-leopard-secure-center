import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

export default function DashboardPage(): ReactNode {
  const { t } = useTranslation('common');
  return (
    <section aria-labelledby="dashboard-heading">
      <h1 id="dashboard-heading">{t('dashboard')}</h1>
      <p>Welcome to the management console.</p>
    </section>
  );
}
