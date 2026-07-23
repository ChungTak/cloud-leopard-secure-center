import { useState } from 'react';
import type { ReactNode } from 'react';
import { NavLink } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMediaQuery } from '../hooks/useMediaQuery';
import Can from './Can.tsx';

interface NavItem {
  to: string;
  labelKey:
    | 'dashboard'
    | 'tenants'
    | 'users'
    | 'settings'
    | 'organizations'
    | 'spatial'
    | 'roles'
    | 'roleBindings'
    | 'devices'
    | 'cameras'
    | 'tags'
    | 'externalBindings'
    | 'projections'
    | 'audit'
    | 'config';
  permission?: string;
}

const items: NavItem[] = [
  { to: '/admin/dashboard', labelKey: 'dashboard' },
  {
    to: '/admin/tenants',
    labelKey: 'tenants',
    permission: 'platform:tenant:read',
  },
  { to: '/admin/users', labelKey: 'users', permission: 'tenant:user:read' },
  {
    to: '/admin/organizations',
    labelKey: 'organizations',
    permission: 'tenant:organization:read',
  },
  { to: '/admin/spatial', labelKey: 'spatial', permission: 'tenant:site:read' },
  { to: '/admin/roles', labelKey: 'roles', permission: 'tenant:role:read' },
  {
    to: '/admin/role-bindings',
    labelKey: 'roleBindings',
    permission: 'tenant:role:read',
  },
  {
    to: '/admin/devices',
    labelKey: 'devices',
    permission: 'tenant:device:read',
  },
  {
    to: '/admin/cameras',
    labelKey: 'cameras',
    permission: 'tenant:camera:read',
  },
  { to: '/admin/tags', labelKey: 'tags', permission: 'tenant:device:read' },
  {
    to: '/admin/external-bindings',
    labelKey: 'externalBindings',
    permission: 'tenant:device:read',
  },
  {
    to: '/admin/projections',
    labelKey: 'projections',
    permission: 'tenant:device:read',
  },
  { to: '/admin/audit', labelKey: 'audit', permission: 'tenant:audit:read' },
  { to: '/admin/config', labelKey: 'config', permission: 'tenant:config:read' },
  {
    to: '/admin/settings',
    labelKey: 'settings',
    permission: 'tenant:config:read',
  },
];

export default function AppNavigation(): ReactNode {
  const { t } = useTranslation('common');
  const isNarrow = useMediaQuery('(max-width: 600px)');
  const [mobileOpen, setMobileOpen] = useState(false);

  const showMenu = !isNarrow || mobileOpen;

  return (
    <header
      style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        padding: 'var(--clsc-spacing-medium)',
        backgroundColor: 'var(--clsc-color-surface)',
        borderBottom: '1px solid var(--clsc-color-border)',
      }}
    >
      <span style={{ fontWeight: 700 }}>{t('appTitle')}</span>
      <button
        type="button"
        aria-label="Toggle navigation"
        aria-expanded={mobileOpen}
        onClick={() => setMobileOpen((open) => !open)}
        style={{ display: isNarrow ? 'block' : 'none' }}
      >
        ☰
      </button>
      {showMenu && (
        <nav aria-label={t('mainNavigation')}>
          <ul
            style={{
              display: 'flex',
              gap: 'var(--clsc-spacing-medium)',
              listStyle: 'none',
            }}
          >
            {items.map((item) => {
              const link = (
                <NavLink
                  to={item.to}
                  style={({ isActive }) => ({
                    color: 'var(--clsc-color-text)',
                    textDecoration: isActive ? 'underline' : 'none',
                    fontWeight: isActive ? 700 : 400,
                  })}
                >
                  {t(item.labelKey)}
                </NavLink>
              );
              return (
                <li key={item.to}>
                  {item.permission ? (
                    <Can permission={item.permission}>{link}</Can>
                  ) : (
                    link
                  )}
                </li>
              );
            })}
          </ul>
        </nav>
      )}
    </header>
  );
}
