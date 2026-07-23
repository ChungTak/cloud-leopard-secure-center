import { describe, it, expect, beforeEach } from 'vitest';
import { useSessionStore } from './session.ts';
import { queryClient } from '../api/queryClient.ts';

describe('session store', () => {
  beforeEach(() => {
    queryClient.clear();
    useSessionStore.setState({
      tenant: undefined,
      accessToken: undefined,
      refreshToken: undefined,
      capabilities: [],
    });
    window.localStorage.clear();
  });

  it('clears query cache and session on logout', () => {
    useSessionStore.setState({ tenant: 't1' });
    const key = ['tenant', 't1', 'devices', 'a'];
    queryClient.setQueryData(key, { id: 'a' });
    window.localStorage.setItem('clsc.session', 'secret');

    useSessionStore.getState().logout();

    expect(useSessionStore.getState().tenant).toBeUndefined();
    expect(useSessionStore.getState().accessToken).toBeUndefined();
    expect(useSessionStore.getState().capabilities).toEqual([]);
    expect(queryClient.getQueryData(key)).toBeUndefined();
    expect(window.localStorage.getItem('clsc.session')).toBeNull();
  });
});
