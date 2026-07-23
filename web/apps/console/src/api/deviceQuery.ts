import {
  useMutation,
  useQueryClient,
  queryOptions,
  type QueryKey,
} from '@tanstack/react-query';
import { apiClient } from './client.ts';
import { tenantQueryKey } from './tenantQuery.ts';
import type { components } from '@clsc/api-client';

type DeviceDto = components['schemas']['DeviceDto'];
type CreateDeviceRequest = components['schemas']['CreateDeviceRequest'];
type UpdateDeviceRequest = components['schemas']['UpdateDeviceRequest'];
type ChangeDeviceLifecycleRequest =
  components['schemas']['ChangeDeviceLifecycleRequest'];

function devicesPrefix(tenant: string | undefined): readonly unknown[] {
  return tenantQueryKey(tenant, ['devices']);
}

export function deviceQueryKey(
  tenant: string | undefined,
  filters: {
    search?: string;
    organizationId?: string;
    areaId?: string;
    lifecycle?: string;
  } = {},
) {
  return tenantQueryKey(tenant, ['devices', filters]);
}

export function devicesQueryOptions(
  tenant: string | undefined,
  filters: {
    search?: string;
    organizationId?: string;
    areaId?: string;
    lifecycle?: string;
  } = {},
) {
  return queryOptions({
    queryKey: deviceQueryKey(tenant, filters),
    queryFn: async ({ signal }) => {
      const { data, error } = await apiClient.GET('/devices', {
        params: { query: filters as never },
        signal,
      });
      if (error) throw error;
      return (data ?? []) as DeviceDto[];
    },
    enabled: Boolean(tenant),
  });
}

function etagHeader(revision: number): string {
  return `"${revision}"`;
}

export function useCreateDevice(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (body: CreateDeviceRequest) => {
      const { data, error } = await apiClient.POST('/devices', {
        body: body as never,
      });
      if (error) throw error;
      return data as DeviceDto;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: devicesPrefix(tenant) });
    },
  });
}

export function useUpdateDevice(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: {
      id: string;
      organizationId?: string;
      areaId?: string;
      name: string;
      serial?: string;
      expectedRevision: number;
    }) => {
      const body: UpdateDeviceRequest = {
        organizationId: payload.organizationId,
        areaId: payload.areaId,
        name: payload.name,
        serial: payload.serial,
        expectedRevision: payload.expectedRevision,
      };
      const { data, error } = await apiClient.PATCH('/devices/{id}', {
        params: { path: { id: payload.id } },
        body: body as never,
        headers: { 'If-Match': etagHeader(payload.expectedRevision) },
      });
      if (error) throw error;
      return data as DeviceDto;
    },
    onMutate: async (payload) => {
      if (!tenant)
        return { previous: [] as [QueryKey, DeviceDto[] | undefined][] };
      await queryClient.cancelQueries({ queryKey: devicesPrefix(tenant) });
      const previous = queryClient.getQueriesData<DeviceDto[]>({
        queryKey: devicesPrefix(tenant),
      });
      queryClient.setQueriesData<DeviceDto[]>(
        { queryKey: devicesPrefix(tenant) },
        (old) =>
          old?.map((d) =>
            d.id === payload.id
              ? {
                  ...d,
                  organizationId: payload.organizationId ?? d.organizationId,
                  areaId: payload.areaId ?? d.areaId,
                  name: payload.name,
                  serial: payload.serial ?? d.serial,
                }
              : d,
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
      queryClient.invalidateQueries({ queryKey: devicesPrefix(tenant) });
    },
  });
}

export function useChangeDeviceLifecycle(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: {
      id: string;
      lifecycle: string;
      expectedRevision: number;
    }) => {
      const body: ChangeDeviceLifecycleRequest = {
        lifecycle: payload.lifecycle,
        expectedRevision: payload.expectedRevision,
      };
      const { data, error } = await apiClient.POST('/devices/{id}/lifecycle', {
        params: { path: { id: payload.id } },
        body: body as never,
        headers: { 'If-Match': etagHeader(payload.expectedRevision) },
      });
      if (error) throw error;
      return data as DeviceDto;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: devicesPrefix(tenant) });
    },
  });
}

export function useDeleteDevice(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const { error } = await apiClient.DELETE('/devices/{id}', {
        params: { path: { id } },
      });
      if (error) throw error;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: devicesPrefix(tenant) });
    },
  });
}
