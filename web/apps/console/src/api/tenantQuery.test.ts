import { describe, it, expect, beforeEach } from 'vitest';
import { tenantQueryKey, deviceQueryOptions } from './tenantQuery.ts';
import { useSessionStore } from '../stores/session.ts';
import { queryClient } from './queryClient.ts';

describe('tenant query integration', () => {
  beforeEach(() => {
    queryClient.clear();
    useSessionStore.setState({
      tenant: undefined,
      accessToken: undefined,
      refreshToken: undefined,
    });
  });

  it('prefixes query keys with tenant', () => {
    expect(tenantQueryKey('t1', ['devices', 'a'])).toEqual([
      'tenant',
      't1',
      'devices',
      'a',
    ]);
    expect(tenantQueryKey(undefined, ['devices', 'a'])).toEqual([
      'devices',
      'a',
    ]);
  });

  it('clears the previous tenant cache when tenant changes', () => {
    useSessionStore.getState().setTenant('t1');

    const key = tenantQueryKey('t1', ['devices', 'a']);
    queryClient.setQueryData(key, { id: 'a' });
    expect(queryClient.getQueryData(key)).toEqual({ id: 'a' });

    useSessionStore.getState().setTenant('t2');

    expect(queryClient.getQueryData(key)).toBeUndefined();
  });

  it('includes tenant in device query options', () => {
    const options = deviceQueryOptions('t1', 'a');
    expect(options.queryKey).toEqual(['tenant', 't1', 'devices', 'a']);
    expect(options.enabled).toBe(true);
  });

  it('disables device query when tenant is missing', () => {
    const options = deviceQueryOptions(undefined, 'a');
    expect(options.enabled).toBe(false);
  });
});
