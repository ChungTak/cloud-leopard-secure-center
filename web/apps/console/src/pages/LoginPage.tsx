import { useState, type FormEvent } from 'react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@clsc/ui';

export default function LoginPage(): ReactNode {
  const { t } = useTranslation('common');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');

  function handleSubmit(event: FormEvent): void {
    event.preventDefault();
    // eslint-disable-next-line no-console
    console.log('login submitted', { username });
  }

  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        minHeight: '100vh',
        backgroundColor: 'var(--clsc-color-background)',
      }}
    >
      <form
        onSubmit={handleSubmit}
        style={{
          width: 'min(100%, 22rem)',
          padding: 'var(--clsc-spacing-xlarge)',
          backgroundColor: 'var(--clsc-color-surface)',
          borderRadius: 'var(--clsc-shape-radius-large)',
          boxShadow: '0 4px 12px rgba(0,0,0,0.08)',
        }}
      >
        <h1>{t('appTitle')}</h1>
        <label htmlFor="username">{t('username')}</label>
        <input
          id="username"
          type="text"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          autoComplete="username"
          style={{
            display: 'block',
            width: '100%',
            marginBottom: 'var(--clsc-spacing-medium)',
          }}
        />
        <label htmlFor="password">{t('password')}</label>
        <input
          id="password"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="current-password"
          style={{
            display: 'block',
            width: '100%',
            marginBottom: 'var(--clsc-spacing-medium)',
          }}
        />
        <Button type="submit" label={t('login')} />
      </form>
    </div>
  );
}
