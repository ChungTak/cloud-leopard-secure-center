import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor, cleanup } from '@testing-library/react';
import { useSessionStore } from '../stores/session.ts';
import Can from './Can.tsx';

describe('Can', () => {
  beforeEach(() => {
    useSessionStore.setState({
      capabilities: [],
      tenant: undefined,
      accessToken: undefined,
      refreshToken: undefined,
    });
  });

  afterEach(() => cleanup());

  it('renders children when capability is present', () => {
    useSessionStore.setState({ capabilities: ['tenant:user:read'] });
    render(<Can permission="tenant:user:read">visible</Can>);
    expect(screen.getByText('visible')).toBeDefined();
  });

  it('renders fallback when capability is missing', () => {
    render(
      <Can permission="tenant:user:read" fallback={<span>hidden</span>}>
        visible
      </Can>,
    );
    expect(screen.getByText('hidden')).toBeDefined();
    expect(screen.queryByText('visible')).toBeNull();
  });

  it('reacts to capability changes', async () => {
    render(<Can permission="tenant:user:read">visible</Can>);
    expect(screen.queryByText('visible')).toBeNull();
    useSessionStore.setState({ capabilities: ['tenant:user:read'] });
    await waitFor(() => {
      expect(screen.getByText('visible')).toBeDefined();
    });
  });
});
