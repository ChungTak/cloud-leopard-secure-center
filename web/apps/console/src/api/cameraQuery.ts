import {
  useMutation,
  useQueryClient,
  queryOptions,
  type QueryKey,
} from '@tanstack/react-query';
import { apiClient } from './client.ts';
import { tenantQueryKey } from './tenantQuery.ts';
import type { components } from '@clsc/api-client';

type CameraDto = components['schemas']['CameraDto'];
type CreateCameraRequest = components['schemas']['CreateCameraRequest'];
type UpdateCameraRequest = components['schemas']['UpdateCameraRequest'];

function camerasPrefix(tenant: string | undefined): readonly unknown[] {
  return tenantQueryKey(tenant, ['cameras']);
}

export function cameraQueryKey(
  tenant: string | undefined,
  filters: {
    search?: string;
    deviceId?: string;
    areaId?: string;
    sensitivity?: string;
  } = {},
) {
  return tenantQueryKey(tenant, ['cameras', filters]);
}

export function camerasQueryOptions(
  tenant: string | undefined,
  filters: {
    search?: string;
    deviceId?: string;
    areaId?: string;
    sensitivity?: string;
  } = {},
) {
  return queryOptions({
    queryKey: cameraQueryKey(tenant, filters),
    queryFn: async ({ signal }) => {
      const { data, error } = await apiClient.GET('/cameras', {
        params: { query: filters as never },
        signal,
      });
      if (error) throw error;
      return (data ?? []) as CameraDto[];
    },
    enabled: Boolean(tenant),
  });
}

function etagHeader(revision: number): string {
  return `"${revision}"`;
}

export function useCreateCamera(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (body: CreateCameraRequest) => {
      const { data, error } = await apiClient.POST('/cameras', {
        body: body as never,
      });
      if (error) throw error;
      return data as CameraDto;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: camerasPrefix(tenant) });
    },
  });
}

export function useUpdateCamera(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: {
      id: string;
      areaId?: string;
      name: string;
      sensitivity: string;
      isEnabled: boolean;
      expectedRevision: number;
    }) => {
      const body: UpdateCameraRequest = {
        areaId: payload.areaId,
        name: payload.name,
        sensitivity: payload.sensitivity,
        isEnabled: payload.isEnabled,
        expectedRevision: payload.expectedRevision,
      };
      const { data, error } = await apiClient.PATCH('/cameras/{id}', {
        params: { path: { id: payload.id } },
        body: body as never,
        headers: { 'If-Match': etagHeader(payload.expectedRevision) },
      });
      if (error) throw error;
      return data as CameraDto;
    },
    onMutate: async (payload) => {
      if (!tenant)
        return { previous: [] as [QueryKey, CameraDto[] | undefined][] };
      await queryClient.cancelQueries({ queryKey: camerasPrefix(tenant) });
      const previous = queryClient.getQueriesData<CameraDto[]>({
        queryKey: camerasPrefix(tenant),
      });
      queryClient.setQueriesData<CameraDto[]>(
        { queryKey: camerasPrefix(tenant) },
        (old) =>
          old?.map((c) =>
            c.id === payload.id
              ? {
                  ...c,
                  areaId: payload.areaId ?? c.areaId,
                  name: payload.name,
                  sensitivity: payload.sensitivity,
                  isEnabled: payload.isEnabled,
                }
              : c,
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
      queryClient.invalidateQueries({ queryKey: camerasPrefix(tenant) });
    },
  });
}

export function useDeleteCamera(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const { error } = await apiClient.DELETE('/cameras/{id}', {
        params: { path: { id } },
      });
      if (error) throw error;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: camerasPrefix(tenant) });
    },
  });
}
