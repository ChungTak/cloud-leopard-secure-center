import { Suspense } from 'react';
import type { ReactNode } from 'react';

export default function Suspended({
  children,
}: {
  children: ReactNode;
}): ReactNode {
  return (
    <Suspense fallback={<div aria-live="polite">加载中...</div>}>
      {children}
    </Suspense>
  );
}
