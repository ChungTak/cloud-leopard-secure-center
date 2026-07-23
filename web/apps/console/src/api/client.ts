import { createApiClient } from '@clsc/api-client';
import { useSessionStore } from '../stores/session.ts';

export const apiClient = createApiClient({
  baseUrl: import.meta.env.VITE_API_BASE_URL ?? '/api/v1',
  store: {
    getAccessToken: () => useSessionStore.getState().accessToken,
    getRefreshToken: () => useSessionStore.getState().refreshToken,
    getTenant: () => useSessionStore.getState().tenant,
    setTokens: (accessToken, refreshToken) =>
      useSessionStore.getState().setTokens(accessToken, refreshToken),
    clearTokens: () => useSessionStore.getState().clearSession(),
  },
});
