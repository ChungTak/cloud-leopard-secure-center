/**
 * Security player Worker/Wasm loader.
 *
 * Phase 1: self-hosted workers and Wasm codecs are not implemented; the loader
 * returns an unsupported status so the component can fall back to the native
 * `<video>` element or display an unsupported message.
 */

export interface WorkerLoadResult {
  ok: boolean;
  error?: 'unsupported' | 'unavailable' | 'load_failed';
}

/**
 * Attempt to load a dedicated worker and optional Wasm module for decoding.
 */
export async function loadSecurityPlayerWorker(
  _workerUrl?: string,
  _wasmUrl?: string,
): Promise<WorkerLoadResult> {
  // Phase 1: self-hosted decoding and Wasm runtime are deferred.
  return { ok: false, error: 'unsupported' };
}
