import type { ReactNode } from 'react';
import { Outlet } from 'react-router-dom';
import AppNavigation from '../components/AppNavigation.tsx';
import AppBreadcrumb from '../components/AppBreadcrumb.tsx';
import ErrorBoundary from '../components/ErrorBoundary.tsx';
import Suspended from '../components/Suspended.tsx';
import PermissionGuard from '../components/PermissionGuard.tsx';

export default function AdminLayout(): ReactNode {
  return (
    <div className="admin-layout" style={{ minHeight: '100vh' }}>
      <AppNavigation />
      <main style={{ padding: 'var(--clsc-spacing-medium)' }}>
        <AppBreadcrumb />
        <ErrorBoundary>
          <Suspended>
            <PermissionGuard>
              <Outlet />
            </PermissionGuard>
          </Suspended>
        </ErrorBoundary>
      </main>
    </div>
  );
}
