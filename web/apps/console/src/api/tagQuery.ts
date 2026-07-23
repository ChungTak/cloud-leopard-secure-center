import {
  useMutation,
  useQueryClient,
  queryOptions,
  type QueryKey,
} from '@tanstack/react-query';
import { apiClient } from './client.ts';
import { tenantQueryKey } from './tenantQuery.ts';
import type { components } from '@clsc/api-client';

type TagDto = components['schemas']['TagDto'];
type CreateTagRequest = components['schemas']['CreateTagRequest'];
type UpdateTagRequest = components['schemas']['UpdateTagRequest'];

function tagsPrefix(tenant: string | undefined): readonly unknown[] {
  return tenantQueryKey(tenant, ['tags']);
}

export function tagQueryKey(
  tenant: string | undefined,
  filters: { resourceType?: string; resourceId?: string; search?: string } = {},
) {
  return tenantQueryKey(tenant, ['tags', filters]);
}

export function tagsQueryOptions(
  tenant: string | undefined,
  filters: { resourceType?: string; resourceId?: string; search?: string } = {},
) {
  return queryOptions({
    queryKey: tagQueryKey(tenant, filters),
    queryFn: async ({ signal }) => {
      const { data, error } = await apiClient.GET('/tags', {
        params: { query: filters as never },
        signal,
      });
      if (error) throw error;
      return (data ?? []) as TagDto[];
    },
    enabled: Boolean(tenant),
  });
}

function etagHeader(revision: number): string {
  return `"${revision}"`;
}

export function useCreateTag(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (body: CreateTagRequest) => {
      const { data, error } = await apiClient.POST('/tags', {
        body: body as never,
      });
      if (error) throw error;
      return data as TagDto;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: tagsPrefix(tenant) });
    },
  });
}

export function useUpdateTag(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: {
      id: string;
      value: string;
      expectedRevision: number;
    }) => {
      const body: UpdateTagRequest = {
        value: payload.value,
        expectedRevision: payload.expectedRevision,
      };
      const { data, error } = await apiClient.PATCH('/tags/{id}', {
        params: { path: { id: payload.id } },
        body: body as never,
        headers: { 'If-Match': etagHeader(payload.expectedRevision) },
      });
      if (error) throw error;
      return data as TagDto;
    },
    onMutate: async (payload) => {
      if (!tenant)
        return { previous: [] as [QueryKey, TagDto[] | undefined][] };
      await queryClient.cancelQueries({ queryKey: tagsPrefix(tenant) });
      const previous = queryClient.getQueriesData<TagDto[]>({
        queryKey: tagsPrefix(tenant),
      });
      queryClient.setQueriesData<TagDto[]>(
        { queryKey: tagsPrefix(tenant) },
        (old) =>
          old?.map((t) =>
            t.id === payload.id ? { ...t, value: payload.value } : t,
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
      queryClient.invalidateQueries({ queryKey: tagsPrefix(tenant) });
    },
  });
}

export function useDeleteTag(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const { error } = await apiClient.DELETE('/tags/{id}', {
        params: { path: { id } },
      });
      if (error) throw error;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: tagsPrefix(tenant) });
    },
  });
}
