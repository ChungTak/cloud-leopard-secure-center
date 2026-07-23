import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import {
  organizationQueryKey,
  organizationUnitsQueryOptions,
  useUpdateOrganizationUnit,
  useMoveOrganizationUnit,
} from './organizationQuery.ts';
import type { components } from '@clsc/api-client';

type OrganizationUnitDto = components['schemas']['OrganizationUnitDto'];

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

describe('organization query', () => {
  let client: QueryClient;

  beforeEach(() => {
    client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('prefixes query key with tenant and filters', () => {
    expect(
      organizationQueryKey('t1', { parentId: 'p1', search: 'acme' }),
    ).toEqual([
      'tenant',
      't1',
      'organization-units',
      { parentId: 'p1', search: 'acme' },
    ]);
  });

  it('disables query when tenant is missing', () => {
    const options = organizationUnitsQueryOptions(undefined);
    expect(options.enabled).toBe(false);
  });

  it('rolls back optimistic update on 412', async () => {
    const initial: OrganizationUnitDto[] = [
      {
        id: 'ou-1',
        tenantId: 't1',
        code: 'acme',
        name: 'Acme',
        parentId: null,
        revision: 1,
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T00:00:00Z',
        actor: null,
      },
    ];
    client.setQueryData(organizationQueryKey('t1'), initial);
    vi.stubGlobal(
      'fetch',
      mockFetch(412, { status: 412, title: 'Precondition Failed' }),
    );

    const { result } = renderHook(() => useUpdateOrganizationUnit('t1'), {
      wrapper: wrapper(client),
    });

    result.current.mutate({ id: 'ou-1', name: 'Updated', expectedRevision: 1 });

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(
      client.getQueryData<OrganizationUnitDto[]>(
        organizationQueryKey('t1'),
      )?.[0].name,
    ).toBe('Acme');
  });

  it('optimistically moves a node and rolls back on 409', async () => {
    const initial: OrganizationUnitDto[] = [
      {
        id: 'ou-1',
        tenantId: 't1',
        code: 'acme',
        name: 'Acme',
        parentId: 'ou-2',
        revision: 2,
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T00:00:00Z',
        actor: null,
      },
      {
        id: 'ou-2',
        tenantId: 't1',
        code: 'parent',
        name: 'Parent',
        parentId: null,
        revision: 1,
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T00:00:00Z',
        actor: null,
      },
    ];
    client.setQueryData(organizationQueryKey('t1'), initial);
    vi.stubGlobal('fetch', mockFetch(409, { status: 409, title: 'Conflict' }));

    const { result } = renderHook(() => useMoveOrganizationUnit('t1'), {
      wrapper: wrapper(client),
    });

    result.current.mutate({ id: 'ou-1', parentId: null, expectedRevision: 2 });

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(
      client.getQueryData<OrganizationUnitDto[]>(
        organizationQueryKey('t1'),
      )?.[0].parentId,
    ).toBe('ou-2');
  });
});
