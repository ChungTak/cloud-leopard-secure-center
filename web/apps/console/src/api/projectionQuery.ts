import { queryOptions } from '@tanstack/react-query';
import { apiClient } from './client.ts';
import { tenantQueryKey } from './tenantQuery.ts';
import type { components } from '@clsc/api-client';

type ProjectionStateDto = components['schemas']['ProjectionStateDto'];

export function projectionQueryKey(
  tenant: string | undefined,
  filters: { deviceId?: string; isStale?: boolean } = {},
) {
  return tenantQueryKey(tenant, ['projections', filters]);
}

export function projectionsQueryOptions(
  tenant: string | undefined,
  filters: { deviceId?: string; isStale?: boolean } = {},
) {
  return queryOptions({
    queryKey: projectionQueryKey(tenant, filters),
    queryFn: async ({ signal }) => {
      const { data, error } = await apiClient.GET('/projections', {
        params: { query: filters as never },
        signal,
      });
      if (error) throw error;
      return (data ?? []) as ProjectionStateDto[];
    },
    enabled: Boolean(tenant),
  });
}
