import * as React from 'react';

export interface ButtonProps {
  label: string;
  onClick?: () => void;
}

export function Button({ label, onClick }: ButtonProps): React.ReactElement {
  return <button onClick={onClick}>{label}</button>;
}
