import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { usersQueryOptions, useUpdateUser } from './userQuery.ts';
import type { components } from '@clsc/api-client';

type UserDto = components['schemas']['UserDto'];

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

describe('user query', () => {
  let client: QueryClient;

  beforeEach(() => {
    client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('prefixes query key with tenant and filters', () => {
    expect(usersQueryOptions('t1', { search: 'alice' }).queryKey).toEqual([
      'tenant',
      't1',
      'users',
      { search: 'alice' },
    ]);
  });

  it('rolls back optimistic user update on 412', async () => {
    const initial: UserDto[] = [
      {
        id: 'u1',
        tenantId: 't1',
        username: 'alice',
        displayName: 'Alice',
        status: 'active',
        sessionVersion: 1,
        revision: 1,
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T00:00:00Z',
        actor: null,
        deletedAt: null,
      },
    ];
    client.setQueryData(['tenant', 't1', 'users', {}], initial);
    vi.stubGlobal(
      'fetch',
      mockFetch(412, { status: 412, title: 'Precondition Failed' }),
    );

    const { result } = renderHook(() => useUpdateUser('t1'), {
      wrapper: wrapper(client),
    });

    result.current.mutate({
      id: 'u1',
      displayName: 'Alicia',
      expectedRevision: 1,
    });

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(
      client.getQueryData<UserDto[]>(['tenant', 't1', 'users', {}])?.[0]
        .displayName,
    ).toBe('Alice');
  });
});
