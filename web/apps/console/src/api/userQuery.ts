import {
  useMutation,
  useQueryClient,
  queryOptions,
} from '@tanstack/react-query';
import { apiClient } from './client.ts';
import { tenantQueryKey } from './tenantQuery.ts';
import type { components } from '@clsc/api-client';

type UserDto = components['schemas']['UserDto'];

type CreateUserRequest = components['schemas']['CreateUserRequest'];

type UpdateUserRequest = components['schemas']['UpdateUserRequest'];

type ChangeUserStatusRequest = components['schemas']['ChangeUserStatusRequest'];

type SetPasswordRequest = components['schemas']['SetPasswordRequest'];

type ManageMfaRequest = components['schemas']['ManageMfaRequest'];

export function userQueryKey(
  tenant: string | undefined,
  filters: { search?: string; status?: string } = {},
) {
  return tenantQueryKey(tenant, ['users', filters]);
}

export function usersQueryOptions(
  tenant: string | undefined,
  filters: { search?: string; status?: string } = {},
) {
  return queryOptions({
    queryKey: userQueryKey(tenant, filters),
    queryFn: async ({ signal }) => {
      const { data, error } = await apiClient.GET('/users', {
        params: { query: filters as never },
        signal,
      });
      if (error) throw error;
      return (data ?? []) as UserDto[];
    },
    enabled: Boolean(tenant),
  });
}

function etagHeader(revision: number): string {
  return `"${revision}"`;
}

export function useCreateUser(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (body: CreateUserRequest) => {
      const { data, error } = await apiClient.POST('/users', {
        body: body as never,
      });
      if (error) throw error;
      return data as UserDto;
    },
    onSuccess: () => {
      if (tenant) {
        queryClient.invalidateQueries({ queryKey: userQueryKey(tenant) });
      }
    },
  });
}

export function useUpdateUser(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: {
      id: string;
      displayName: string;
      expectedRevision: number;
    }) => {
      const body: UpdateUserRequest = {
        displayName: payload.displayName,
        expectedRevision: payload.expectedRevision,
      };
      const { data, error } = await apiClient.PATCH('/users/{id}', {
        params: { path: { id: payload.id } },
        body: body as never,
        headers: { 'If-Match': etagHeader(payload.expectedRevision) },
      });
      if (error) throw error;
      return data as UserDto;
    },
    onMutate: async (payload) => {
      if (!tenant) return { previous: [] as UserDto[] };
      await queryClient.cancelQueries({ queryKey: userQueryKey(tenant) });
      const previous =
        queryClient.getQueryData<UserDto[]>(userQueryKey(tenant)) ?? [];
      const next = previous.map((u) =>
        u.id === payload.id ? { ...u, displayName: payload.displayName } : u,
      );
      queryClient.setQueryData(userQueryKey(tenant), next);
      return { previous };
    },
    onError: (_err, _payload, context) => {
      if (tenant && context?.previous) {
        queryClient.setQueryData(userQueryKey(tenant), context.previous);
      }
    },
    onSettled: () => {
      if (tenant) {
        queryClient.invalidateQueries({ queryKey: userQueryKey(tenant) });
      }
    },
  });
}

export function useChangeUserStatus(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: {
      id: string;
      status: string;
      expectedRevision: number;
    }) => {
      const body: ChangeUserStatusRequest = {
        status: payload.status,
        expectedRevision: payload.expectedRevision,
      };
      const { data, error } = await apiClient.POST('/users/{id}/status', {
        params: { path: { id: payload.id } },
        body: body as never,
        headers: { 'If-Match': etagHeader(payload.expectedRevision) },
      });
      if (error) throw error;
      return data as UserDto;
    },
    onSuccess: () => {
      if (tenant) {
        queryClient.invalidateQueries({ queryKey: userQueryKey(tenant) });
      }
    },
  });
}

export function useSetUserPassword(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: {
      id: string;
      password: string;
      expectedRevision: number;
    }) => {
      const body: SetPasswordRequest = {
        password: payload.password,
        expectedRevision: payload.expectedRevision,
      };
      const { error } = await apiClient.POST('/users/{id}/password', {
        params: { path: { id: payload.id } },
        body: body as never,
        headers: { 'If-Match': etagHeader(payload.expectedRevision) },
      });
      if (error) throw error;
    },
    onSuccess: () => {
      if (tenant) {
        queryClient.invalidateQueries({ queryKey: userQueryKey(tenant) });
      }
    },
  });
}

export function useManageUserMfa(tenant: string | undefined) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: {
      id: string;
      enabled: boolean;
      expectedRevision: number;
    }) => {
      const body: ManageMfaRequest = {
        enabled: payload.enabled,
        expectedRevision: payload.expectedRevision,
      };
      const { data, error } = await apiClient.POST('/users/{id}/mfa', {
        params: { path: { id: payload.id } },
        body: body as never,
        headers: { 'If-Match': etagHeader(payload.expectedRevision) },
      });
      if (error) throw error;
      return data as UserDto;
    },
    onSuccess: () => {
      if (tenant) {
        queryClient.invalidateQueries({ queryKey: userQueryKey(tenant) });
      }
    },
  });
}

export function useCreateServiceAccount(_tenant: string | undefined) {
  return useMutation({
    mutationFn: async (name: string) => {
      const { data, error } = await apiClient.POST('/service-accounts', {
        body: { name } as never,
      });
      if (error) throw error;
      return data as components['schemas']['ApiKeyCreatedDto'];
    },
  });
}
