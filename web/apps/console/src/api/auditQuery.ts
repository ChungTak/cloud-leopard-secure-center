import { queryOptions } from '@tanstack/react-query';
import { apiClient } from './client.ts';
import { tenantQueryKey } from './tenantQuery.ts';
import type { components } from '@clsc/api-client';

type AuditRecordDto = components['schemas']['AuditRecordDto'];

export function auditQueryKey(
  tenant: string | undefined,
  filters: {
    targetType?: string;
    targetId?: string;
    action?: string;
    from?: string;
    to?: string;
    search?: string;
  } = {},
) {
  return tenantQueryKey(tenant, ['audit-records', filters]);
}

export function auditRecordsQueryOptions(
  tenant: string | undefined,
  filters: {
    targetType?: string;
    targetId?: string;
    action?: string;
    from?: string;
    to?: string;
    search?: string;
  } = {},
) {
  return queryOptions({
    queryKey: auditQueryKey(tenant, filters),
    queryFn: async ({ signal }) => {
      const { data, error } = await apiClient.GET('/audit-records', {
        params: { query: filters as never },
        signal,
      });
      if (error) throw error;
      return (data ?? []) as AuditRecordDto[];
    },
    enabled: Boolean(tenant),
  });
}

export function auditRecordQueryOptions(
  tenant: string | undefined,
  id: string | undefined,
) {
  return queryOptions({
    queryKey: tenantQueryKey(tenant, ['audit-records', id]),
    queryFn: async ({ signal }) => {
      if (!id) return null as unknown as AuditRecordDto;
      const { data, error } = await apiClient.GET('/audit-records/{id}', {
        params: { path: { id } },
        signal,
      });
      if (error) throw error;
      return data as AuditRecordDto;
    },
    enabled: Boolean(tenant) && Boolean(id),
  });
}
