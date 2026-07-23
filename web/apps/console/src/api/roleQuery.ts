import {
  useMutation,
  useQueryClient,
  queryOptions,
} from '@tanstack/react-query';
import { apiClient } from './client.ts';
import { tenantQueryKey } from './tenantQuery.ts';
import type { components } from '@clsc/api-client';

type RoleDto = components['schemas']['RoleDto'];

type CreateRoleRequest = components['schemas']['CreateRoleRequest'];

type UpdateRoleRequest = components['schemas']['UpdateRoleRequest'];

export function roleQueryKey(
  tenant: string | undefined,
  filters: { search?: string } = {},
) {
  return tenantQueryKey(tenant, ['roles', filters]);
}

export function rolesQueryOptions(
  tenant: string | undefined,
  filters: { search?: string } = {},
) {
  return queryOptions({
    queryKey: roleQueryKey(tenant, filters),
    queryFn: async ({ signal }) => {
      const { data, error } = await apiClient.GET('/roles', {
        params: { query: filters as never },
        signal,
      });
      if (error) throw error;
      return (data ?? []) as RoleDto[];
    },
    enabled: Boolean(tenant),
  });
}

function etagHeader(revision: number): string {
  return `"${revision}"`;
}

export function useCreateRole(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (body: CreateRoleRequest) => {
      const { data, error } = await apiClient.POST('/roles', {
        body: body as never,
      });
      if (error) throw error;
      return data as RoleDto;
    },
    onSuccess: () => {
      if (tenant) {
        queryClient.invalidateQueries({ queryKey: roleQueryKey(tenant) });
      }
    },
  });
}

export function useUpdateRole(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: {
      id: string;
      name: string;
      permissions: string[];
      expectedRevision: number;
    }) => {
      const body: UpdateRoleRequest = {
        name: payload.name,
        permissions: payload.permissions,
        expectedRevision: payload.expectedRevision,
      };
      const { data, error } = await apiClient.PATCH('/roles/{id}', {
        params: { path: { id: payload.id } },
        body: body as never,
        headers: { 'If-Match': etagHeader(payload.expectedRevision) },
      });
      if (error) throw error;
      return data as RoleDto;
    },
    onMutate: async (payload) => {
      if (!tenant) return { previous: [] as RoleDto[] };
      await queryClient.cancelQueries({ queryKey: roleQueryKey(tenant) });
      const previous =
        queryClient.getQueryData<RoleDto[]>(roleQueryKey(tenant)) ?? [];
      const next = previous.map((r) =>
        r.id === payload.id
          ? { ...r, name: payload.name, permissions: payload.permissions }
          : r,
      );
      queryClient.setQueryData(roleQueryKey(tenant), next);
      return { previous };
    },
    onError: (_err, _payload, context) => {
      if (tenant && context?.previous) {
        queryClient.setQueryData(roleQueryKey(tenant), context.previous);
      }
    },
    onSettled: () => {
      if (tenant) {
        queryClient.invalidateQueries({ queryKey: roleQueryKey(tenant) });
      }
    },
  });
}

export function useDeleteRole(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const { error } = await apiClient.DELETE('/roles/{id}', {
        params: { path: { id } },
      });
      if (error) throw error;
    },
    onSuccess: () => {
      if (tenant) {
        queryClient.invalidateQueries({ queryKey: roleQueryKey(tenant) });
      }
    },
  });
}
