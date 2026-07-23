import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { NavLink, useRouteError } from 'react-router-dom';

export default function ErrorPage(): ReactNode {
  const { t } = useTranslation('common');
  const error = useRouteError();
  // eslint-disable-next-line no-console
  console.error(error);

  return (
    <main
      role="alert"
      style={{ padding: 'var(--clsc-spacing-large)', textAlign: 'center' }}
    >
      <h1>500</h1>
      <p>{t('serverError')}</p>
      <NavLink to="/admin/dashboard">{t('backHome')}</NavLink>
    </main>
  );
}
