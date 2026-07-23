import {
  useMutation,
  useQueryClient,
  queryOptions,
} from '@tanstack/react-query';
import { apiClient } from './client.ts';
import { tenantQueryKey } from './tenantQuery.ts';
import type { components } from '@clsc/api-client';

type RoleBindingDto = components['schemas']['RoleBindingDto'];

type CreateRoleBindingRequest =
  components['schemas']['CreateRoleBindingRequest'];

type UpdateRoleBindingRequest =
  components['schemas']['UpdateRoleBindingRequest'];

export function roleBindingQueryKey(
  tenant: string | undefined,
  filters: { search?: string; principalId?: string; roleId?: string } = {},
) {
  return tenantQueryKey(tenant, ['role-bindings', filters]);
}

export function roleBindingsQueryOptions(
  tenant: string | undefined,
  filters: { search?: string; principalId?: string; roleId?: string } = {},
) {
  return queryOptions({
    queryKey: roleBindingQueryKey(tenant, filters),
    queryFn: async ({ signal }) => {
      const { data, error } = await apiClient.GET('/role-bindings', {
        params: { query: filters as never },
        signal,
      });
      if (error) throw error;
      return (data ?? []) as RoleBindingDto[];
    },
    enabled: Boolean(tenant),
  });
}

function etagHeader(revision: number): string {
  return `"${revision}"`;
}

export function useCreateRoleBinding(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (body: CreateRoleBindingRequest) => {
      const { data, error } = await apiClient.POST('/role-bindings', {
        body: body as never,
      });
      if (error) throw error;
      return data as RoleBindingDto;
    },
    onSuccess: () => {
      if (tenant) {
        queryClient.invalidateQueries({
          queryKey: roleBindingQueryKey(tenant),
        });
      }
    },
  });
}

export function useUpdateRoleBinding(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: {
      id: string;
      roleId: string;
      scope: CreateRoleBindingRequest['scope'];
      validFrom: string;
      validUntil: string | null;
      expectedRevision: number;
    }) => {
      const body: UpdateRoleBindingRequest = {
        roleId: payload.roleId,
        scope: payload.scope,
        validFrom: payload.validFrom,
        validUntil: payload.validUntil,
        expectedRevision: payload.expectedRevision,
      };
      const { data, error } = await apiClient.PATCH('/role-bindings/{id}', {
        params: { path: { id: payload.id } },
        body: body as never,
        headers: { 'If-Match': etagHeader(payload.expectedRevision) },
      });
      if (error) throw error;
      return data as RoleBindingDto;
    },
    onMutate: async (payload) => {
      if (!tenant) return { previous: [] as RoleBindingDto[] };
      await queryClient.cancelQueries({
        queryKey: roleBindingQueryKey(tenant),
      });
      const previous =
        queryClient.getQueryData<RoleBindingDto[]>(
          roleBindingQueryKey(tenant),
        ) ?? [];
      const next = previous.map((b) =>
        b.id === payload.id
          ? {
              ...b,
              roleId: payload.roleId,
              scope: payload.scope,
              validFrom: payload.validFrom,
              validUntil: payload.validUntil,
            }
          : b,
      );
      queryClient.setQueryData(roleBindingQueryKey(tenant), next);
      return { previous };
    },
    onError: (_err, _payload, context) => {
      if (tenant && context?.previous) {
        queryClient.setQueryData(roleBindingQueryKey(tenant), context.previous);
      }
    },
    onSettled: () => {
      if (tenant) {
        queryClient.invalidateQueries({
          queryKey: roleBindingQueryKey(tenant),
        });
      }
    },
  });
}

export function useDeleteRoleBinding(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const { error } = await apiClient.DELETE('/role-bindings/{id}', {
        params: { path: { id } },
      });
      if (error) throw error;
    },
    onSuccess: () => {
      if (tenant) {
        queryClient.invalidateQueries({
          queryKey: roleBindingQueryKey(tenant),
        });
      }
    },
  });
}

export function useExplainAuth() {
  return useMutation({
    mutationFn: async (body: components['schemas']['AuthExplainRequest']) => {
      const { data, error } = await apiClient.POST('/auth/explain', {
        body: body as never,
      });
      if (error) throw error;
      return data as components['schemas']['AuthExplainResponse'];
    },
  });
}
