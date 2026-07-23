import type { ReactNode } from 'react';
import { useSessionStore } from '../stores/session.ts';

export interface CanProps {
  permission: string;
  capability?: string;
  fallback?: ReactNode;
  children: ReactNode;
}

export default function Can({
  permission,
  capability,
  fallback = null,
  children,
}: CanProps): ReactNode {
  const capabilities = useSessionStore((state) => state.capabilities);
  const required = capability ?? permission;
  const allowed =
    capabilities.includes(required) || capabilities.includes(permission);
  return allowed ? children : fallback;
}
