import { test, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import App from './App.tsx';

test('App renders title and button', () => {
  render(<App />);
  expect(screen.getByText('Cloud Leopard Secure Center')).toBeDefined();
  const button = screen.getByText('Clicked 0 times');
  fireEvent.click(button);
  expect(screen.getByText('Clicked 1 times')).toBeDefined();
});
