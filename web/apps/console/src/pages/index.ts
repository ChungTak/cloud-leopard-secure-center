import { lazy } from 'react';

export const LoginPage = lazy(() => import('./LoginPage.tsx'));
export const DashboardPage = lazy(() => import('./DashboardPage.tsx'));
export const NotFoundPage = lazy(() => import('./NotFoundPage.tsx'));
export const ForbiddenPage = lazy(() => import('./ForbiddenPage.tsx'));
export const ErrorPage = lazy(() => import('./ErrorPage.tsx'));
export const OrganizationPage = lazy(() => import('./OrganizationPage.tsx'));
export const SpatialPage = lazy(() => import('./SpatialPage.tsx'));
export const UsersPage = lazy(() => import('./UsersPage.tsx'));
export const RolesPage = lazy(() => import('./RolesPage.tsx'));
export const RoleBindingsPage = lazy(() => import('./RoleBindingsPage.tsx'));
export const DevicesPage = lazy(() => import('./DevicesPage.tsx'));
export const CamerasPage = lazy(() => import('./CamerasPage.tsx'));
