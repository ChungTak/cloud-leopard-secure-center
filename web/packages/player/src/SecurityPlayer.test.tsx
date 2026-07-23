import { test, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/react';
import { SecurityPlayer } from './SecurityPlayer';

afterEach(() => {
  cleanup();
});

test('SecurityPlayer reports unsupported and does not leak token', () => {
  const onError = vi.fn();
  render(
    <SecurityPlayer
      source={{ mainUrl: 'wss://example.com/stream?token=secret' }}
      token="bearer-secret-token"
      onError={onError}
    />,
  );

  expect(screen.getByTestId('player-error').textContent).toContain(
    'unsupported',
  );
  expect(onError).toHaveBeenCalledOnce();
  const error = onError.mock.calls[0][0];
  expect(error.sanitized).toBe(true);

  // Token must never appear in the DOM.
  expect(screen.getByTestId('security-player').textContent).not.toContain(
    'bearer-secret-token',
  );
});

test('SecurityPlayer can switch between main and sub stream', () => {
  const onDiagnostics = vi.fn();
  render(
    <SecurityPlayer
      source={{
        mainUrl: 'wss://example.com/main',
        subUrl: 'wss://example.com/sub',
      }}
      token="token"
      startWithSubStream
      onDiagnostics={onDiagnostics}
    />,
  );

  const video = screen.getByTestId('player-video') as HTMLVideoElement;
  expect(video.src).toContain('/sub');

  fireEvent.click(screen.getByTestId('player-switch-stream'));

  expect(video.src).toContain('/main');
});
