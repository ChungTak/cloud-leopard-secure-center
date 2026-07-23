/**
 * Player security policy configuration.
 *
 * Phase 1: CSP/SRI/COOP/COEP headers are managed by the hosting application;
 * this file documents the required directives and provides a default policy
 * object. The actual headers must be served by the CDN/reverse proxy.
 */

export interface PlayerSecurityPolicy {
  /** Content-Security-Policy for the player frame. */
  contentSecurityPolicy: string;
  /** Cross-Origin-Opener-Policy. */
  coop: string;
  /** Cross-Origin-Embedder-Policy. */
  coep: string;
  /** Subresource Integrity hashes for the player bundle and worker. */
  subresourceIntegrity: Record<string, string>;
}

/** Default policy compatible with a self-hosted worker + Wasm build. */
export const defaultPlayerSecurityPolicy: PlayerSecurityPolicy = {
  contentSecurityPolicy:
    "default-src 'none'; script-src 'self'; worker-src 'self'; media-src 'self' blob:; connect-src 'self'; img-src 'self' data:;",
  coop: 'same-origin',
  coep: 'require-corp',
  subresourceIntegrity: {},
};

/** Browser compatibility matrix for self-hosted decoding. */
export const securePlayerBrowserMatrix: Record<
  string,
  { worker: boolean; wasm: boolean; fallback: 'native' | 'none' }
> = {
  chromium: { worker: true, wasm: true, fallback: 'native' },
  firefox: { worker: true, wasm: true, fallback: 'native' },
  webkit: { worker: true, wasm: false, fallback: 'native' },
  edge: { worker: true, wasm: true, fallback: 'native' },
};
