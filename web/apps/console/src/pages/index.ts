import { lazy } from 'react';

export const LoginPage = lazy(() => import('./LoginPage.tsx'));
export const DashboardPage = lazy(() => import('./DashboardPage.tsx'));
export const NotFoundPage = lazy(() => import('./NotFoundPage.tsx'));
export const ForbiddenPage = lazy(() => import('./ForbiddenPage.tsx'));
export const ErrorPage = lazy(() => import('./ErrorPage.tsx'));
export const OrganizationPage = lazy(() => import('./OrganizationPage.tsx'));
export const SpatialPage = lazy(() => import('./SpatialPage.tsx'));
