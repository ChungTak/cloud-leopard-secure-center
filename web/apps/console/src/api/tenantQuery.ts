import { queryOptions } from '@tanstack/react-query';
import { apiClient } from './client.ts';

export function tenantQueryKey(
  tenant: string | undefined,
  key: readonly unknown[],
): readonly unknown[] {
  return tenant ? (['tenant', tenant, ...key] as const) : key;
}

export function deviceQueryOptions(tenant: string | undefined, id: string) {
  return queryOptions({
    queryKey: tenantQueryKey(tenant, ['devices', id]),
    queryFn: async ({ signal }) => {
      const { data } = await apiClient.GET('/devices/{id}', {
        params: { path: { id } },
        signal,
      });
      return data;
    },
    enabled: Boolean(tenant),
  });
}
