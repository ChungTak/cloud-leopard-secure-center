import type { ReactNode } from 'react';
import { Navigate, useMatches } from 'react-router-dom';
import { useSessionStore } from '../stores/session.ts';

type RouteHandle = {
  permission?: string;
  capability?: string;
};

export default function PermissionGuard({
  children,
}: {
  children: ReactNode;
}): ReactNode {
  const matches = useMatches();
  const capabilities = useSessionStore((state) => state.capabilities);

  const required = matches
    .map(
      (match) =>
        (match.handle as RouteHandle | undefined)?.capability ??
        (match.handle as RouteHandle | undefined)?.permission,
    )
    .filter(Boolean)
    .at(-1);

  if (required && !capabilities.includes(required)) {
    return <Navigate to="/admin/forbidden" replace />;
  }

  return children;
}
