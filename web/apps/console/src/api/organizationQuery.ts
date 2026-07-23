import {
  useMutation,
  useQueryClient,
  queryOptions,
} from '@tanstack/react-query';
import { apiClient } from './client.ts';
import { tenantQueryKey } from './tenantQuery.ts';
import type { components } from '@clsc/api-client';

type OrganizationUnitDto = components['schemas']['OrganizationUnitDto'];
type CreateOrganizationUnitRequest =
  components['schemas']['CreateOrganizationUnitRequest'];
type UpdateOrganizationUnitRequest =
  components['schemas']['UpdateOrganizationUnitRequest'];
type MoveOrganizationUnitRequest =
  components['schemas']['MoveOrganizationUnitRequest'];

export function organizationQueryKey(
  tenant: string | undefined,
  filters: { parentId?: string; search?: string } = {},
) {
  return tenantQueryKey(tenant, ['organization-units', filters]);
}

export function organizationUnitsQueryOptions(
  tenant: string | undefined,
  filters: { parentId?: string; search?: string } = {},
) {
  return queryOptions({
    queryKey: organizationQueryKey(tenant, filters),
    queryFn: async ({ signal }) => {
      const { data, error } = await apiClient.GET('/organization-units', {
        params: { query: filters },
        signal,
      });
      if (error) throw error;
      return (data ?? []) as OrganizationUnitDto[];
    },
    enabled: Boolean(tenant),
  });
}

function etagHeader(revision: number): string {
  return `"${revision}"`;
}

export function useCreateOrganizationUnit(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (body: CreateOrganizationUnitRequest) => {
      const { data, error } = await apiClient.POST('/organization-units', {
        body: body as never,
      });
      if (error) throw error;
      return data as OrganizationUnitDto;
    },
    onSuccess: () => {
      if (tenant) {
        queryClient.invalidateQueries({
          queryKey: organizationQueryKey(tenant),
        });
      }
    },
  });
}

export function useUpdateOrganizationUnit(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: {
      id: string;
      name: string;
      expectedRevision: number;
    }) => {
      const body: UpdateOrganizationUnitRequest = {
        name: payload.name,
        expectedRevision: payload.expectedRevision,
      };
      const { data, error } = await apiClient.PATCH(
        '/organization-units/{id}',
        {
          params: { path: { id: payload.id } },
          body: body as never,
          headers: { 'If-Match': etagHeader(payload.expectedRevision) },
        },
      );
      if (error) throw error;
      return data as OrganizationUnitDto;
    },
    onMutate: async (payload) => {
      if (!tenant) return { previous: [] as OrganizationUnitDto[] };
      await queryClient.cancelQueries({
        queryKey: organizationQueryKey(tenant),
      });
      const previous =
        queryClient.getQueryData<OrganizationUnitDto[]>(
          organizationQueryKey(tenant),
        ) ?? [];
      const next = previous.map((u) =>
        u.id === payload.id ? { ...u, name: payload.name } : u,
      );
      queryClient.setQueryData(organizationQueryKey(tenant), next);
      return { previous };
    },
    onError: (_err, _payload, context) => {
      if (tenant && context?.previous) {
        queryClient.setQueryData(
          organizationQueryKey(tenant),
          context.previous,
        );
      }
    },
    onSettled: () => {
      if (tenant) {
        queryClient.invalidateQueries({
          queryKey: organizationQueryKey(tenant),
        });
      }
    },
  });
}

export function useMoveOrganizationUnit(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: {
      id: string;
      parentId: string | null;
      expectedRevision: number;
    }) => {
      const body: MoveOrganizationUnitRequest = {
        parentId: payload.parentId,
        expectedRevision: payload.expectedRevision,
      };
      const { data, error } = await apiClient.POST(
        '/organization-units/{id}/move',
        {
          params: { path: { id: payload.id } },
          body: body as never,
          headers: { 'If-Match': etagHeader(payload.expectedRevision) },
        },
      );
      if (error) throw error;
      return data as OrganizationUnitDto;
    },
    onMutate: async (payload) => {
      if (!tenant) return { previous: [] as OrganizationUnitDto[] };
      await queryClient.cancelQueries({
        queryKey: organizationQueryKey(tenant),
      });
      const previous =
        queryClient.getQueryData<OrganizationUnitDto[]>(
          organizationQueryKey(tenant),
        ) ?? [];
      const next = previous.map((u) =>
        u.id === payload.id ? { ...u, parentId: payload.parentId } : u,
      );
      queryClient.setQueryData(organizationQueryKey(tenant), next);
      return { previous };
    },
    onError: (_err, _payload, context) => {
      if (tenant && context?.previous) {
        queryClient.setQueryData(
          organizationQueryKey(tenant),
          context.previous,
        );
      }
    },
    onSettled: () => {
      if (tenant) {
        queryClient.invalidateQueries({
          queryKey: organizationQueryKey(tenant),
        });
      }
    },
  });
}

export function useDeleteOrganizationUnit(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const { error } = await apiClient.DELETE('/organization-units/{id}', {
        params: { path: { id } },
      });
      if (error) throw error;
    },
    onSuccess: () => {
      if (tenant) {
        queryClient.invalidateQueries({
          queryKey: organizationQueryKey(tenant),
        });
      }
    },
  });
}
