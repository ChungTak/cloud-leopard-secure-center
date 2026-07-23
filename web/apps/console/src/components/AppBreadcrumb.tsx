import type { ReactNode } from 'react';
import { useMatches, NavLink } from 'react-router-dom';
import { useTranslation } from 'react-i18next';

type BreadcrumbKey =
  | 'dashboard'
  | 'tenants'
  | 'users'
  | 'settings'
  | 'breadcrumbHome';

interface RouteHandle {
  breadcrumb?: BreadcrumbKey;
}

export default function AppBreadcrumb(): ReactNode {
  const { t } = useTranslation('common');
  const matches = useMatches();

  const crumbs = matches
    .map((match) => ({
      path: match.pathname,
      key: (match.handle as RouteHandle | undefined)?.breadcrumb,
    }))
    .filter((crumb) => crumb.key != null)
    .map((crumb) => ({
      path: crumb.path,
      label: t(crumb.key as BreadcrumbKey),
    }));

  if (crumbs.length === 0) return null;

  return (
    <nav aria-label={t('breadcrumb')}>
      <ol
        style={{
          display: 'flex',
          gap: 'var(--clsc-spacing-small)',
          listStyle: 'none',
          padding: 0,
          margin: '0 0 var(--clsc-spacing-medium)',
        }}
      >
        <li>
          <NavLink to="/admin/dashboard">{t('breadcrumbHome')}</NavLink>
        </li>
        {crumbs.map((crumb, index) => (
          <li key={`${crumb.path}-${index}`}>
            <span aria-hidden="true"> / </span>
            <NavLink to={crumb.path}>{crumb.label}</NavLink>
          </li>
        ))}
      </ol>
    </nav>
  );
}
