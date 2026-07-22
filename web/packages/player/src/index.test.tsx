import { test, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { Player } from './index';

test('Player renders stream url', () => {
  render(<Player streamUrl="wss://example.com/stream" />);
  expect(screen.getByTestId('player').textContent).toContain(
    'wss://example.com/stream',
  );
});
