import {
  useMutation,
  useQueryClient,
  queryOptions,
  type QueryKey,
} from '@tanstack/react-query';
import { apiClient } from './client.ts';
import { tenantQueryKey } from './tenantQuery.ts';
import type { components } from '@clsc/api-client';

type ConfigValueDto = components['schemas']['ConfigValueDto'];
type ConfigDefinitionDto = components['schemas']['ConfigDefinitionDto'];
type UpdateConfigValueRequest =
  components['schemas']['UpdateConfigValueRequest'];

function configValuesPrefix(tenant: string | undefined): readonly unknown[] {
  return tenantQueryKey(tenant, ['config-values']);
}

export function configValuesQueryOptions(
  tenant: string | undefined,
  filters: { module?: string; search?: string } = {},
) {
  return queryOptions({
    queryKey: tenantQueryKey(tenant, ['config-values', filters]),
    queryFn: async ({ signal }) => {
      const { data, error } = await apiClient.GET('/config-values', {
        params: { query: filters as never },
        signal,
      });
      if (error) throw error;
      return (data ?? []) as ConfigValueDto[];
    },
    enabled: Boolean(tenant),
  });
}

export function configDefinitionsQueryOptions(
  tenant: string | undefined,
  filters: { module?: string; search?: string } = {},
) {
  return queryOptions({
    queryKey: tenantQueryKey(tenant, ['config-definitions', filters]),
    queryFn: async ({ signal }) => {
      const { data, error } = await apiClient.GET('/config-definitions', {
        params: { query: filters as never },
        signal,
      });
      if (error) throw error;
      return (data ?? []) as ConfigDefinitionDto[];
    },
    enabled: Boolean(tenant),
  });
}

function etagHeader(revision: number): string {
  return `"${revision}"`;
}

export function useUpdateConfigValue(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: {
      id: string;
      value?: string;
      clearSecret: boolean;
      expectedRevision: number;
    }) => {
      const body: UpdateConfigValueRequest = {
        value: payload.value,
        clearSecret: payload.clearSecret,
        expectedRevision: payload.expectedRevision,
      };
      const { data, error } = await apiClient.PATCH('/config-values/{id}', {
        params: { path: { id: payload.id } },
        body: body as never,
        headers: { 'If-Match': etagHeader(payload.expectedRevision) },
      });
      if (error) throw error;
      return data as ConfigValueDto;
    },
    onMutate: async (payload) => {
      if (!tenant)
        return { previous: [] as [QueryKey, ConfigValueDto[] | undefined][] };
      await queryClient.cancelQueries({ queryKey: configValuesPrefix(tenant) });
      const previous = queryClient.getQueriesData<ConfigValueDto[]>({
        queryKey: configValuesPrefix(tenant),
      });
      queryClient.setQueriesData<ConfigValueDto[]>(
        { queryKey: configValuesPrefix(tenant) },
        (old) =>
          old?.map((v) =>
            v.id === payload.id
              ? {
                  ...v,
                  value: payload.value ?? v.value,
                  secretRef: payload.clearSecret ? undefined : v.secretRef,
                }
              : v,
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
      queryClient.invalidateQueries({ queryKey: configValuesPrefix(tenant) });
    },
  });
}
