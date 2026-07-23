import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import type { ReactNode } from 'react';
import { ThemeProvider, useTheme, createTheme } from './index';

function TestConsumer(): ReactNode {
  const { mode, toggleMode } = useTheme();
  return (
    <button onClick={toggleMode} data-testid="mode-toggle">
      {mode}
    </button>
  );
}

describe('ThemeProvider', () => {
  it('applies default light tokens and toggles dark mode', () => {
    const before = document.documentElement.style.getPropertyValue(
      '--clsc-color-primary',
    );
    expect(before).toBe('');

    render(
      <ThemeProvider>
        <TestConsumer />
      </ThemeProvider>,
    );

    expect(document.documentElement.getAttribute('data-clsc-theme')).toBe(
      'light',
    );
    expect(
      document.documentElement.style.getPropertyValue('--clsc-color-primary'),
    ).toBe('#0066ff');

    fireEvent.click(screen.getByTestId('mode-toggle'));
    expect(document.documentElement.getAttribute('data-clsc-theme')).toBe(
      'dark',
    );
    expect(
      document.documentElement.style.getPropertyValue('--clsc-color-primary'),
    ).toBe('#4d94ff');
  });

  it('throws when useTheme is called outside provider', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() => render(<TestConsumer />)).toThrow(
      'useTheme must be used within ThemeProvider',
    );
    spy.mockRestore();
  });
});

describe('createTheme', () => {
  it('produces light and dark color palettes', () => {
    const light = createTheme('light');
    const dark = createTheme('dark');
    expect(light.tokens.colors.background).toBe('#f4f6f8');
    expect(dark.tokens.colors.background).toBe('#0b0c15');
  });
});
