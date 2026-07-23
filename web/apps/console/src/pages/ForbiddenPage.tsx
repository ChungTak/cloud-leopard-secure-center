import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { NavLink } from 'react-router-dom';

export default function ForbiddenPage(): ReactNode {
  const { t } = useTranslation('common');
  return (
    <main
      role="alert"
      style={{ padding: 'var(--clsc-spacing-large)', textAlign: 'center' }}
    >
      <h1>403</h1>
      <p>{t('forbidden')}</p>
      <NavLink to="/admin/dashboard">{t('backHome')}</NavLink>
    </main>
  );
}
