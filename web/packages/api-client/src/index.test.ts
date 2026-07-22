import { test, expect } from 'vitest';
import { ApiClient } from './index';

test('health returns ok', async () => {
  const client = new ApiClient('/');
  expect(await client.health()).toEqual({ status: 'ok' });
});
