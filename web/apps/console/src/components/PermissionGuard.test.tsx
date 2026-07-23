import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { render, screen, cleanup } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { routes } from '../routes/index.tsx';
import { useSessionStore } from '../stores/session.ts';

describe('PermissionGuard', () => {
  beforeEach(() => {
    useSessionStore.setState({
      capabilities: [],
      tenant: 't1',
      accessToken: 'a',
      refreshToken: 'r',
    });
  });

  afterEach(() => cleanup());

  function renderPath(initialEntry: string) {
    const router = createMemoryRouter(routes, {
      initialEntries: [initialEntry],
    });
    return render(<RouterProvider router={router} />);
  }

  it('allows dashboard without capability', async () => {
    renderPath('/admin/dashboard');
    expect(await screen.findByRole('heading')).toBeDefined();
  });

  it('redirects to forbidden on deep links without capability', async () => {
    renderPath('/admin/users');
    expect(await screen.findByText('403')).toBeDefined();
  });

  it('allows deep links when capability is present', async () => {
    useSessionStore.setState({ capabilities: ['tenant:user:read'] });
    renderPath('/admin/users');
    expect(await screen.findByRole('heading')).toBeDefined();
  });
});
