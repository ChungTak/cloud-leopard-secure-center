import { test, expect, afterEach } from 'vitest';
import { render, screen, cleanup } from '@testing-library/react';
import { MultiPlayerLayout } from './MultiPlayerLayout';

afterEach(() => {
  cleanup();
});

test.each([
  [1, 1],
  [4, 2],
  [9, 3],
  [16, 4],
] as const)('renders %i slots in a %i by %i grid', (layout, columns) => {
  render(<MultiPlayerLayout layout={layout} />);
  expect(screen.getAllByTestId('player-slot').length).toBe(layout);
  const grid = screen.getByTestId('multi-player-layout');
  expect(grid.style.gridTemplateColumns).toBe(`repeat(${columns}, 1fr)`);
});

test('uses custom slot renderer', () => {
  render(
    <MultiPlayerLayout
      layout={1}
      renderSlot={(i) => <span data-testid={`custom-${i}`}>ok</span>}
    />,
  );
  expect(screen.getByTestId('custom-0').textContent).toBe('ok');
});
