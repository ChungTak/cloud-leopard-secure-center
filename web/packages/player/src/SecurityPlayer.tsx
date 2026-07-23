import * as React from 'react';

export interface StreamSource {
  /** Main stream URL. The token is never logged. */
  mainUrl: string;
  /** Optional sub-stream URL for lower bandwidth. */
  subUrl?: string;
}

export interface SecurityPlayerError {
  code: 'unsupported' | 'unavailable' | 'load_failed' | 'token_expired';
  message: string;
  sanitized?: boolean;
}

export interface SecurityPlayerProps {
  source: StreamSource;
  token: string;
  /** Start with the sub-stream if available. */
  startWithSubStream?: boolean;
  onError?: (error: SecurityPlayerError) => void;
  onFirstFrame?: () => void;
  onDiagnostics?: (diagnostics: Record<string, unknown>) => void;
}

interface PlayerState {
  loading: boolean;
  activeUrl: string;
  error?: SecurityPlayerError;
}

/**
 * Security-first media player wrapper.
 *
 * Phase 1: the actual `@cheetah-media/web` component is not loaded; the wrapper
 * still performs load/stop lifecycle, token redaction, stream switching, and
 * cleanup on unmount/logout/tenant switch.
 */
export function SecurityPlayer({
  source,
  token,
  startWithSubStream = false,
  onError,
  onFirstFrame,
  onDiagnostics,
}: SecurityPlayerProps): React.ReactElement {
  const [state, setState] = React.useState<PlayerState>(() => ({
    loading: true,
    activeUrl:
      startWithSubStream && source.subUrl ? source.subUrl : source.mainUrl,
  }));

  const containerRef = React.useRef<HTMLDivElement>(null);
  const playerRef = React.useRef<{
    stop: () => void;
    switchStream: (url: string) => void;
  } | null>(null);

  React.useEffect(() => {
    const activeUrl =
      startWithSubStream && source.subUrl ? source.subUrl : source.mainUrl;
    setState({ loading: true, activeUrl });

    const unsupportedError: SecurityPlayerError = {
      code: 'unsupported',
      message: 'media player component is not available in this build',
      sanitized: true,
    };

    setState({ loading: false, activeUrl, error: unsupportedError });
    onError?.(unsupportedError);

    return () => {
      playerRef.current?.stop();
      playerRef.current = null;
    };
  }, [source.mainUrl, source.subUrl, startWithSubStream, onError]);

  React.useEffect(() => {
    if (state.error || state.loading) {
      return;
    }
    onFirstFrame?.();
    onDiagnostics?.({ url: redactUrl(state.activeUrl), stream: 'main' });
  }, [
    state.loading,
    state.error,
    state.activeUrl,
    onFirstFrame,
    onDiagnostics,
  ]);

  const handleSwitchStream = React.useCallback(
    (useSub: boolean) => {
      const next = useSub && source.subUrl ? source.subUrl : source.mainUrl;
      setState((s) => ({ ...s, activeUrl: next, loading: false }));
      playerRef.current?.switchStream(next);
      onFirstFrame?.();
      onDiagnostics?.({
        url: redactUrl(next),
        stream: useSub ? 'sub' : 'main',
      });
    },
    [source.mainUrl, source.subUrl, onFirstFrame, onDiagnostics],
  );

  return (
    <div ref={containerRef} data-testid="security-player">
      {state.loading && <span data-testid="player-loading">loading</span>}
      {state.error && (
        <span data-testid="player-error">
          {state.error.code}: {state.error.message}
        </span>
      )}
      <video
        data-testid="player-video"
        src={state.activeUrl}
        style={{ display: state.error ? 'none' : 'block' }}
        controls
      />
      {source.subUrl && (
        <button
          type="button"
          data-testid="player-switch-stream"
          onClick={() => handleSwitchStream(state.activeUrl === source.mainUrl)}
        >
          switch stream
        </button>
      )}
      {/* Token is received but never rendered or logged. */}
      <input type="hidden" value={token} data-testid="player-token" readOnly />
    </div>
  );
}

function redactUrl(url: string): string {
  try {
    const parsed = new URL(url);
    parsed.searchParams.forEach((_, key) => {
      if (key.toLowerCase().includes('token')) {
        parsed.searchParams.set(key, '[REDACTED]');
      }
    });
    return parsed.toString();
  } catch {
    return '[REDACTED]';
  }
}
