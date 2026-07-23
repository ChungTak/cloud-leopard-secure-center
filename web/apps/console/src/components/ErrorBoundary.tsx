import { Component, type ErrorInfo, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

interface ErrorBoundaryProps {
  children: ReactNode;
  fallback?: ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
}

class ErrorBoundaryClass extends Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(): ErrorBoundaryState {
    return { hasError: true };
  }

  override componentDidCatch(error: Error, info: ErrorInfo): void {
    // eslint-disable-next-line no-console
    console.error('ErrorBoundary caught an error:', error, info.componentStack);
  }

  override render(): ReactNode {
    if (this.state.hasError) {
      return this.props.fallback ?? <DefaultErrorFallback />;
    }
    return this.props.children;
  }
}

function DefaultErrorFallback(): ReactNode {
  const { t } = useTranslation('common');
  return (
    <div
      role="alert"
      style={{
        padding: 'var(--clsc-spacing-large)',
        backgroundColor: 'var(--clsc-color-surface)',
        color: 'var(--clsc-color-error)',
        borderRadius: 'var(--clsc-shape-radius)',
      }}
    >
      <h2>{t('serverError')}</h2>
      <p>{t('retry')}</p>
    </div>
  );
}

export default ErrorBoundaryClass;
