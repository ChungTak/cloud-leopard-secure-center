import { test, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Button } from './index';

test('Button renders label and handles click', () => {
  let clicked = false;
  render(
    <Button
      label="Click me"
      onClick={() => {
        clicked = true;
      }}
    />,
  );
  const button = screen.getByText('Click me');
  fireEvent.click(button);
  expect(clicked).toBe(true);
});
