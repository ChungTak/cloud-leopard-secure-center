import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { render, screen, waitFor, cleanup } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactElement } from 'react';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { routes } from './routes/index';
import ErrorBoundary from './components/ErrorBoundary';
import axe from 'axe-core';
import './i18n/index.ts';

function createMatchMedia(matches = false) {
  return (query: string): MediaQueryList => {
    const listeners = new Set<EventListener>();
    return {
      matches,
      media: query,
      addEventListener: (_event: string, listener: EventListener) => {
        listeners.add(listener);
      },
      removeEventListener: (_event: string, listener: EventListener) => {
        listeners.delete(listener);
      },
      dispatchEvent: (event: Event) => {
        listeners.forEach((listener) => listener(event));
        return true;
      },
      onchange: null,
    } as unknown as MediaQueryList;
  };
}

function setupMatchMedia(matches = false): void {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: createMatchMedia(matches),
  });
}

function renderAdmin(
  initialEntry = '/admin/dashboard',
): ReturnType<typeof render> {
  const router = createMemoryRouter(routes, { initialEntries: [initialEntry] });
  return render(<RouterProvider router={router} />);
}

describe('App shell', () => {
  beforeEach(() => {
    setupMatchMedia(false);
    window.innerWidth = 1024;
  });

  afterEach(() => {
    cleanup();
  });

  it('renders dashboard and Chinese default translations', async () => {
    renderAdmin();
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: '仪表盘' })).toBeDefined();
    });
    expect(screen.getByText('Cloud Leopard Secure Center')).toBeDefined();
  });

  it('supports keyboard navigation in the admin nav', async () => {
    const user = userEvent.setup();
    renderAdmin();
    const nav = await screen.findByLabelText('主导航');
    const firstLink = nav.querySelector('a');
    expect(firstLink).not.toBeNull();
    (firstLink as HTMLElement | null)?.focus();
    await user.keyboard('{Tab}');
    expect(document.activeElement?.tagName).toBe('A');
  });

  it('toggles dark theme through the design tokens', () => {
    renderAdmin();
    expect(document.documentElement.getAttribute('data-clsc-theme')).toBe(
      'light',
    );
  });

  it('isolates errors with the error boundary', () => {
    function Thrower(): ReactElement {
      throw new Error('boom');
    }
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    render(
      <ErrorBoundary>
        <Thrower />
      </ErrorBoundary>,
    );
    expect(screen.getByRole('alert')).toBeDefined();
    spy.mockRestore();
  });

  it('adapts to narrow viewports', async () => {
    setupMatchMedia(true);
    window.innerWidth = 375;
    const user = userEvent.setup();
    renderAdmin();
    const toggle = screen.getByLabelText('Toggle navigation');
    expect(toggle).toBeDefined();
    await user.click(toggle);
    await waitFor(() => {
      expect(screen.getByLabelText('主导航')).toBeDefined();
    });
  });

  it('passes axe accessibility smoke test', async () => {
    const { container } = renderAdmin('/login');
    const results = await axe.run(container);
    expect(results.violations.length).toBe(0);
  });
});
