import {
  useMutation,
  useQueryClient,
  queryOptions,
} from '@tanstack/react-query';
import { apiClient } from './client.ts';
import { tenantQueryKey } from './tenantQuery.ts';
import type { components } from '@clsc/api-client';

type SpatialNodeDto = components['schemas']['SpatialNodeDto'];
type CreateSpatialNodeRequest =
  components['schemas']['CreateSpatialNodeRequest'];
type UpdateSpatialNodeRequest =
  components['schemas']['UpdateSpatialNodeRequest'];
type MoveSpatialNodeRequest = components['schemas']['MoveSpatialNodeRequest'];

export type SpatialNodeType = SpatialNodeDto['nodeType'];

export function spatialQueryKey(
  tenant: string | undefined,
  filters: {
    parentId?: string;
    search?: string;
    nodeType?: SpatialNodeType;
  } = {},
) {
  return tenantQueryKey(tenant, ['spatial-nodes', filters]);
}

export function spatialNodesQueryOptions(
  tenant: string | undefined,
  filters: {
    parentId?: string;
    search?: string;
    nodeType?: SpatialNodeType;
  } = {},
) {
  return queryOptions({
    queryKey: spatialQueryKey(tenant, filters),
    queryFn: async ({ signal }) => {
      const { data, error } = await apiClient.GET('/spatial-nodes', {
        params: { query: filters as never },
        signal,
      });
      if (error) throw error;
      return (data ?? []) as SpatialNodeDto[];
    },
    enabled: Boolean(tenant),
  });
}

function etagHeader(revision: number): string {
  return `"${revision}"`;
}

export function useCreateSpatialNode(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (body: CreateSpatialNodeRequest) => {
      const { data, error } = await apiClient.POST('/spatial-nodes', {
        body: body as never,
      });
      if (error) throw error;
      return data as SpatialNodeDto;
    },
    onSuccess: () => {
      if (tenant) {
        queryClient.invalidateQueries({ queryKey: spatialQueryKey(tenant) });
      }
    },
  });
}

export function useUpdateSpatialNode(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: {
      id: string;
      name: string;
      expectedRevision: number;
    }) => {
      const body: UpdateSpatialNodeRequest = {
        name: payload.name,
        expectedRevision: payload.expectedRevision,
      };
      const { data, error } = await apiClient.PATCH('/spatial-nodes/{id}', {
        params: { path: { id: payload.id } },
        body: body as never,
        headers: { 'If-Match': etagHeader(payload.expectedRevision) },
      });
      if (error) throw error;
      return data as SpatialNodeDto;
    },
    onMutate: async (payload) => {
      if (!tenant) return { previous: [] as SpatialNodeDto[] };
      await queryClient.cancelQueries({ queryKey: spatialQueryKey(tenant) });
      const previous =
        queryClient.getQueryData<SpatialNodeDto[]>(spatialQueryKey(tenant)) ??
        [];
      const next = previous.map((n) =>
        n.id === payload.id ? { ...n, name: payload.name } : n,
      );
      queryClient.setQueryData(spatialQueryKey(tenant), next);
      return { previous };
    },
    onError: (_err, _payload, context) => {
      if (tenant && context?.previous) {
        queryClient.setQueryData(spatialQueryKey(tenant), context.previous);
      }
    },
    onSettled: () => {
      if (tenant) {
        queryClient.invalidateQueries({ queryKey: spatialQueryKey(tenant) });
      }
    },
  });
}

export function useMoveSpatialNode(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: {
      id: string;
      parentId: string | null;
      expectedRevision: number;
    }) => {
      const body: MoveSpatialNodeRequest = {
        parentId: payload.parentId,
        expectedRevision: payload.expectedRevision,
      };
      const { data, error } = await apiClient.POST('/spatial-nodes/{id}/move', {
        params: { path: { id: payload.id } },
        body: body as never,
        headers: { 'If-Match': etagHeader(payload.expectedRevision) },
      });
      if (error) throw error;
      return data as SpatialNodeDto;
    },
    onMutate: async (payload) => {
      if (!tenant) return { previous: [] as SpatialNodeDto[] };
      await queryClient.cancelQueries({ queryKey: spatialQueryKey(tenant) });
      const previous =
        queryClient.getQueryData<SpatialNodeDto[]>(spatialQueryKey(tenant)) ??
        [];
      const next = previous.map((n) =>
        n.id === payload.id ? { ...n, parentId: payload.parentId } : n,
      );
      queryClient.setQueryData(spatialQueryKey(tenant), next);
      return { previous };
    },
    onError: (_err, _payload, context) => {
      if (tenant && context?.previous) {
        queryClient.setQueryData(spatialQueryKey(tenant), context.previous);
      }
    },
    onSettled: () => {
      if (tenant) {
        queryClient.invalidateQueries({ queryKey: spatialQueryKey(tenant) });
      }
    },
  });
}

export function useDeleteSpatialNode(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const { error } = await apiClient.DELETE('/spatial-nodes/{id}', {
        params: { path: { id } },
      });
      if (error) throw error;
    },
    onSuccess: () => {
      if (tenant) {
        queryClient.invalidateQueries({ queryKey: spatialQueryKey(tenant) });
      }
    },
  });
}
