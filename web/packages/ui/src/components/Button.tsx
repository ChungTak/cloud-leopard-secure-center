import type { ReactNode, MouseEventHandler } from 'react';

export interface ButtonProps {
  label: ReactNode;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  type?: 'button' | 'submit' | 'reset';
  disabled?: boolean;
  ariaLabel?: string;
}

export function Button({
  label,
  onClick,
  type = 'button',
  disabled = false,
  ariaLabel,
}: ButtonProps): ReactNode {
  return (
    <button
      type={type}
      onClick={onClick}
      disabled={disabled}
      aria-label={ariaLabel}
      style={{
        fontFamily: 'var(--clsc-typography-font-family)',
        fontSize: 'var(--clsc-typography-font-size-base)',
        padding: 'var(--clsc-density-padding)',
        borderRadius: 'var(--clsc-shape-radius)',
        minHeight: 'var(--clsc-min-tap-size)',
        minWidth: 'var(--clsc-min-tap-size)',
        backgroundColor: 'var(--clsc-color-primary)',
        color: '#ffffff',
        border: 'none',
        cursor: disabled ? 'not-allowed' : 'pointer',
        opacity: disabled ? 0.6 : 1,
      }}
    >
      {label}
    </button>
  );
}
