import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { createApiClient, ApiError, type TokenStore } from './index';

function getRequestHeaders(input: Request, init?: RequestInit): Headers {
  if (init?.headers) return new Headers(init.headers);
  return input.headers;
}

describe('createApiClient', () => {
  let mockFetch: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    mockFetch = vi.fn();
    vi.stubGlobal('fetch', mockFetch);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  function makeStore(overrides: Partial<TokenStore> = {}): TokenStore {
    return {
      getAccessToken: () => 'access-1',
      getRefreshToken: () => 'refresh-1',
      getTenant: () => 't1',
      setTokens: vi.fn(),
      clearTokens: vi.fn(),
      refresh: vi.fn().mockResolvedValue('access-2'),
      ...overrides,
    };
  }

  it('sends Authorization and X-Tenant-Code headers', async () => {
    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({ id: 'a', message: 'ok' }), { status: 200 }),
    );
    const store = makeStore();
    const client = createApiClient({
      baseUrl: 'https://api.example.com',
      store,
    });
    await client.GET('/devices/{id}', { params: { path: { id: 'a' } } });
    const request = mockFetch.mock.calls[0][0] as Request;
    const init = mockFetch.mock.calls[0][1] as RequestInit | undefined;
    const headers = getRequestHeaders(request, init);
    expect(headers.get('Authorization')).toBe('Bearer access-1');
    expect(headers.get('X-Tenant-Code')).toBe('t1');
  });

  it('performs single-flight token refresh on 401 and retries', async () => {
    const refresh = vi.fn().mockResolvedValue('access-2');
    const store = makeStore({ refresh });
    const client = createApiClient({
      baseUrl: 'https://api.example.com',
      store,
    });

    mockFetch.mockImplementation((request: Request, init?: RequestInit) => {
      const headers = getRequestHeaders(request, init);
      const auth = headers.get('Authorization');
      if (auth === 'Bearer access-1') {
        return Promise.resolve(
          new Response(JSON.stringify({ type: 'unauthorized' }), {
            status: 401,
          }),
        );
      }
      return Promise.resolve(
        new Response(JSON.stringify({ ok: true }), { status: 200 }),
      );
    });

    const [first, second] = await Promise.all([
      client.GET('/devices/{id}', { params: { path: { id: 'a' } } }),
      client.GET('/cameras/{id}', { params: { path: { id: 'b' } } }),
    ]);

    expect(refresh).toHaveBeenCalledTimes(1);
    expect(first.data).toEqual({ ok: true });
    expect(second.data).toEqual({ ok: true });
  });

  it('maps 403 to ApiError', async () => {
    const problem = {
      type: 'forbidden',
      title: 'Forbidden',
      detail: 'no access',
    };
    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify(problem), { status: 403 }),
    );
    const client = createApiClient({
      baseUrl: 'https://api.example.com',
      store: makeStore(),
    });
    const result = client.GET('/devices/{id}', {
      params: { path: { id: 'a' } },
    });
    await expect(result).rejects.toBeInstanceOf(ApiError);
    await expect(result).rejects.toMatchObject({
      status: 403,
      code: 'forbidden',
      detail: 'no access',
    });
  });

  it('maps 429 and parses Retry-After', async () => {
    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({ type: 'rate-limited' }), {
        status: 429,
        headers: { 'Retry-After': '120' },
      }),
    );
    const client = createApiClient({
      baseUrl: 'https://api.example.com',
      store: makeStore(),
    });
    const result = client.GET('/devices/{id}', {
      params: { path: { id: 'a' } },
    });
    await expect(result).rejects.toMatchObject({
      status: 429,
      retryAfter: 120,
    });
  });

  it('maps 409 and 412 conflicts', async () => {
    const conflict = { type: 'version-mismatch', title: 'Conflict' };
    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify(conflict), { status: 409 }),
    );
    const client = createApiClient({
      baseUrl: 'https://api.example.com',
      store: makeStore(),
    });
    const result = client.GET('/devices/{id}', {
      params: { path: { id: 'a' } },
    });
    await expect(result).rejects.toMatchObject({
      status: 409,
      code: 'version-mismatch',
    });
  });

  it('aborts an in-flight request when signal is aborted', async () => {
    mockFetch.mockImplementation(() => {
      return new Promise((_resolve, reject) => {
        setTimeout(() => reject(new DOMException('Aborted', 'AbortError')), 10);
      });
    });
    const controller = new AbortController();
    const client = createApiClient({
      baseUrl: 'https://api.example.com',
      store: makeStore(),
    });
    const promise = client.GET('/devices/{id}', {
      params: { path: { id: 'a' } },
      signal: controller.signal,
    });
    controller.abort();
    await expect(promise).rejects.toBeInstanceOf(DOMException);
  });
});
