import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import {
  configValuesQueryOptions,
  useUpdateConfigValue,
} from './configQuery.ts';
import type { components } from '@clsc/api-client';

type ConfigValueDto = components['schemas']['ConfigValueDto'];

function wrapper(client: QueryClient) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
  };
}

function mockFetch(status: number, body: unknown) {
  return vi.fn().mockResolvedValue(
    new Response(JSON.stringify(body), {
      status,
      headers: { 'content-type': 'application/json' },
    }),
  );
}

describe('config query', () => {
  let client: QueryClient;

  beforeEach(() => {
    client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('prefixes list query with tenant and filters', () => {
    expect(configValuesQueryOptions('t1', { module: 'core' }).queryKey).toEqual(
      ['tenant', 't1', 'config-values', { module: 'core' }],
    );
  });

  it('optimistically removes secretRef when clearing a secret', async () => {
    const initial: ConfigValueDto[] = [
      {
        id: 'cv1',
        scope: { kind: 'tenant', tenantId: 't1' },
        configKey: 'smtp.password',
        value: '***',
        secretRef: 'ref-1',
        revision: 1,
      },
    ];
    client.setQueryData(['tenant', 't1', 'config-values', {}], initial);
    vi.stubGlobal(
      'fetch',
      mockFetch(200, {
        id: 'cv1',
        scope: { kind: 'tenant', tenantId: 't1' },
        configKey: 'smtp.password',
        value: '',
        secretRef: null,
        revision: 2,
      }),
    );

    const { result } = renderHook(() => useUpdateConfigValue('t1'), {
      wrapper: wrapper(client),
    });
    result.current.mutate({
      id: 'cv1',
      clearSecret: true,
      expectedRevision: 1,
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    const data = client.getQueryData<ConfigValueDto[]>([
      'tenant',
      't1',
      'config-values',
      {},
    ]);
    expect(data?.[0].secretRef).toBeUndefined();
  });
});
