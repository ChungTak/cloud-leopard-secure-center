import '../i18n/index.ts';

import type { RouteObject } from 'react-router-dom';
import { createBrowserRouter } from 'react-router-dom';

import Root from '../components/Root.tsx';
import LoginLayout from '../layouts/LoginLayout.tsx';
import AdminLayout from '../layouts/AdminLayout.tsx';
import {
  LoginPage,
  DashboardPage,
  NotFoundPage,
  ForbiddenPage,
  ErrorPage,
  OrganizationPage,
  SpatialPage,
  UsersPage,
  RolesPage,
  RoleBindingsPage,
  DevicesPage,
  CamerasPage,
} from '../pages/index.ts';

export const routes: RouteObject[] = [
  {
    path: '/',
    element: <Root />,
    errorElement: <ErrorPage />,
    children: [
      {
        element: <LoginLayout />,
        children: [
          { index: true, element: <LoginPage /> },
          { path: 'login', element: <LoginPage /> },
        ],
      },
      {
        path: 'admin',
        element: <AdminLayout />,
        children: [
          {
            index: true,
            element: <DashboardPage />,
            handle: { breadcrumb: 'dashboard' as const },
          },
          {
            path: 'dashboard',
            element: <DashboardPage />,
            handle: { breadcrumb: 'dashboard' as const },
          },
          {
            path: 'tenants',
            element: <DashboardPage />,
            handle: {
              breadcrumb: 'tenants' as const,
              permission: 'platform:tenant:read' as const,
            },
          },
          {
            path: 'users',
            element: <DashboardPage />,
            handle: {
              breadcrumb: 'users' as const,
              permission: 'tenant:user:read' as const,
            },
          },
          {
            path: 'settings',
            element: <DashboardPage />,
            handle: {
              breadcrumb: 'settings' as const,
              permission: 'tenant:config:read' as const,
            },
          },
          {
            path: 'organizations',
            element: <OrganizationPage />,
            handle: {
              breadcrumb: 'organizations' as const,
              permission: 'tenant:organization:read' as const,
            },
          },
          {
            path: 'spatial',
            element: <SpatialPage />,
            handle: {
              breadcrumb: 'spatial' as const,
              permission: 'tenant:site:read' as const,
            },
          },
          {
            path: 'users',
            element: <UsersPage />,
            handle: {
              breadcrumb: 'users' as const,
              permission: 'tenant:user:read' as const,
            },
          },
          {
            path: 'roles',
            element: <RolesPage />,
            handle: {
              breadcrumb: 'roles' as const,
              permission: 'tenant:role:read' as const,
            },
          },
          {
            path: 'role-bindings',
            element: <RoleBindingsPage />,
            handle: {
              breadcrumb: 'roleBindings' as const,
              permission: 'tenant:role:read' as const,
            },
          },
          {
            path: 'devices',
            element: <DevicesPage />,
            handle: {
              breadcrumb: 'devices' as const,
              permission: 'tenant:device:read' as const,
            },
          },
          {
            path: 'cameras',
            element: <CamerasPage />,
            handle: {
              breadcrumb: 'cameras' as const,
              permission: 'tenant:camera:read' as const,
            },
          },
          { path: 'forbidden', element: <ForbiddenPage /> },
          { path: '*', element: <NotFoundPage /> },
        ],
      },
      { path: '*', element: <NotFoundPage /> },
    ],
  },
];

export function createAppRouter() {
  return createBrowserRouter(routes);
}
