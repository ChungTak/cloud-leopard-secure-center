import {
  useMutation,
  useQueryClient,
  queryOptions,
  type QueryKey,
} from '@tanstack/react-query';
import { apiClient } from './client.ts';
import { tenantQueryKey } from './tenantQuery.ts';
import type { components } from '@clsc/api-client';

type ExternalBindingDto = components['schemas']['ExternalBindingDto'];
type CreateExternalBindingRequest =
  components['schemas']['CreateExternalBindingRequest'];
type ResolveExternalBindingConflictRequest =
  components['schemas']['ResolveExternalBindingConflictRequest'];

function externalBindingsPrefix(
  tenant: string | undefined,
): readonly unknown[] {
  return tenantQueryKey(tenant, ['external-bindings']);
}

export function externalBindingQueryKey(
  tenant: string | undefined,
  filters: {
    resourceType?: string;
    resourceId?: string;
    search?: string;
    state?: string;
  } = {},
) {
  return tenantQueryKey(tenant, ['external-bindings', filters]);
}

export function externalBindingsQueryOptions(
  tenant: string | undefined,
  filters: {
    resourceType?: string;
    resourceId?: string;
    search?: string;
    state?: string;
  } = {},
) {
  return queryOptions({
    queryKey: externalBindingQueryKey(tenant, filters),
    queryFn: async ({ signal }) => {
      const { data, error } = await apiClient.GET('/external-bindings', {
        params: { query: filters as never },
        signal,
      });
      if (error) throw error;
      return (data ?? []) as ExternalBindingDto[];
    },
    enabled: Boolean(tenant),
  });
}

function etagHeader(revision: number): string {
  return `"${revision}"`;
}

export function useCreateExternalBinding(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (body: CreateExternalBindingRequest) => {
      const { data, error } = await apiClient.POST('/external-bindings', {
        body: body as never,
      });
      if (error) throw error;
      return data as ExternalBindingDto;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: externalBindingsPrefix(tenant),
      });
    },
  });
}

export function useResolveExternalBindingConflict(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: {
      id: string;
      action: string;
      expectedRevision: number;
    }) => {
      const body: ResolveExternalBindingConflictRequest = {
        action: payload.action,
        expectedRevision: payload.expectedRevision,
      };
      const { data, error } = await apiClient.POST(
        '/external-bindings/{id}/resolve',
        {
          params: { path: { id: payload.id } },
          body: body as never,
          headers: { 'If-Match': etagHeader(payload.expectedRevision) },
        },
      );
      if (error) throw error;
      return data as ExternalBindingDto;
    },
    onMutate: async (payload) => {
      if (!tenant)
        return {
          previous: [] as [QueryKey, ExternalBindingDto[] | undefined][],
        };
      await queryClient.cancelQueries({
        queryKey: externalBindingsPrefix(tenant),
      });
      const previous = queryClient.getQueriesData<ExternalBindingDto[]>({
        queryKey: externalBindingsPrefix(tenant),
      });
      queryClient.setQueriesData<ExternalBindingDto[]>(
        { queryKey: externalBindingsPrefix(tenant) },
        (old) =>
          old?.map((b) =>
            b.id === payload.id ? { ...b, state: payload.action } : b,
          ) ?? old,
      );
      return { previous };
    },
    onError: (_err, _payload, context) => {
      if (!tenant || !context?.previous) return;
      for (const [key, data] of context.previous) {
        queryClient.setQueryData(key, data);
      }
    },
    onSettled: () => {
      queryClient.invalidateQueries({
        queryKey: externalBindingsPrefix(tenant),
      });
    },
  });
}

export function useDeleteExternalBinding(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const { error } = await apiClient.DELETE('/external-bindings/{id}', {
        params: { path: { id } },
      });
      if (error) throw error;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: externalBindingsPrefix(tenant),
      });
    },
  });
}
