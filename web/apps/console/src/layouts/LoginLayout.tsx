import type { ReactNode } from 'react';
import { Outlet } from 'react-router-dom';
import Suspended from '../components/Suspended.tsx';

export default function LoginLayout(): ReactNode {
  return (
    <div className="login-layout" style={{ minHeight: '100vh' }}>
      <Suspended>
        <Outlet />
      </Suspended>
    </div>
  );
}
