import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import {
  externalBindingsQueryOptions,
  useResolveExternalBindingConflict,
} from './externalBindingQuery.ts';
import type { components } from '@clsc/api-client';

type ExternalBindingDto = components['schemas']['ExternalBindingDto'];

function wrapper(client: QueryClient) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
  };
}

function mockFetch(status: number, body: unknown, options?: ResponseInit) {
  return vi.fn().mockResolvedValue(
    new Response(JSON.stringify(body), {
      status,
      headers: { 'content-type': 'application/json' },
      ...options,
    }),
  );
}

describe('external binding query', () => {
  let client: QueryClient;

  beforeEach(() => {
    client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('prefixes query key with tenant and filters', () => {
    expect(
      externalBindingsQueryOptions('t1', { state: 'conflict' }).queryKey,
    ).toEqual(['tenant', 't1', 'external-bindings', { state: 'conflict' }]);
  });

  it('rolls back optimistic conflict resolve on 409', async () => {
    const initial: ExternalBindingDto[] = [
      {
        id: 'b1',
        tenantId: 't1',
        resourceType: 'device',
        resourceId: 'd1',
        externalRef: 'ext-1',
        externalKind: 'nvr',
        state: 'conflict',
        activatedAt: null,
        revision: 2,
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T00:00:00Z',
        actor: null,
      },
    ];
    client.setQueryData(['tenant', 't1', 'external-bindings', {}], initial);
    vi.stubGlobal('fetch', mockFetch(409, { status: 409, title: 'Conflict' }));

    const { result } = renderHook(
      () => useResolveExternalBindingConflict('t1'),
      {
        wrapper: wrapper(client),
      },
    );

    result.current.mutate({ id: 'b1', action: 'active', expectedRevision: 2 });

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(
      client.getQueryData<ExternalBindingDto[]>([
        'tenant',
        't1',
        'external-bindings',
        {},
      ])?.[0].state,
    ).toBe('conflict');
  });
});
