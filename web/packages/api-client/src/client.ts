import createClient from 'openapi-fetch';
import type { paths } from './openapi';
import { ApiError } from './errors';

export type { paths };
export type { components } from './openapi';

export interface TokenStore {
  getAccessToken(): string | undefined;
  getRefreshToken(): string | undefined;
  getTenant(): string | undefined;
  setTokens(accessToken: string, refreshToken?: string): void;
  clearTokens(): void;
  refresh?(): Promise<string>;
}

export interface CreateApiClientOptions {
  baseUrl: string;
  store: TokenStore;
}

function resolveBaseUrl(baseUrl: string): string {
  if (baseUrl.startsWith('http://') || baseUrl.startsWith('https://')) {
    return baseUrl;
  }
  if (typeof window !== 'undefined' && window.location?.href) {
    return new URL(baseUrl, window.location.href).toString();
  }
  return baseUrl;
}

export function createApiClient({ baseUrl, store }: CreateApiClientOptions) {
  const resolvedBaseUrl = resolveBaseUrl(baseUrl);
  let refreshTask: Promise<string | undefined> | null = null;

  async function refreshAccessToken(): Promise<string | undefined> {
    if (refreshTask) return refreshTask;
    if (!store.refresh) return undefined;

    refreshTask = (async () => {
      try {
        const token = await store.refresh!();
        return token;
      } finally {
        refreshTask = null;
      }
    })();

    return refreshTask;
  }

  async function clscFetch(
    input: RequestInfo | URL,
    init?: RequestInit,
  ): Promise<Response> {
    const requestInit: RequestInit = { ...init };
    const headers = new Headers(requestInit.headers);

    const token = store.getAccessToken();
    if (token) headers.set('Authorization', `Bearer ${token}`);
    const tenant = store.getTenant();
    if (tenant) headers.set('X-Tenant-Code', tenant);

    requestInit.headers = headers;

    try {
      let response = await fetch(input, requestInit);

      if (response.status === 401 && store.refresh) {
        try {
          const newToken = await refreshAccessToken();
          if (newToken) {
            headers.set('Authorization', `Bearer ${newToken}`);
            requestInit.headers = headers;
            response = await fetch(input, requestInit);
          }
        } catch {
          store.clearTokens();
        }
      }

      if (!response.ok) {
        throw await ApiError.fromResponse(response);
      }

      return response;
    } catch (error) {
      if (error instanceof ApiError) throw error;
      if (error instanceof Error && error.name === 'AbortError') throw error;
      throw new ApiError(
        0,
        'network-error',
        error instanceof Error ? error.message : 'Network error',
      );
    }
  }

  return createClient<paths>({ baseUrl: resolvedBaseUrl, fetch: clscFetch });
}
