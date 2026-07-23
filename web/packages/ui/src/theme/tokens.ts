export type ColorMode = 'light' | 'dark';
export type Density = 'compact' | 'default' | 'comfortable';

export interface ClscColors {
  primary: string;
  primaryHover: string;
  background: string;
  surface: string;
  surfaceElevated: string;
  text: string;
  textMuted: string;
  border: string;
  error: string;
  warning: string;
  success: string;
  info: string;
  focusRing: string;
}

export interface ClscSpacing {
  xsmall: string;
  small: string;
  medium: string;
  large: string;
  xlarge: string;
}

export interface ClscShape {
  borderRadius: string;
  borderRadiusLarge: string;
}

export interface ClscTypography {
  fontFamily: string;
  fontSizeSmall: string;
  fontSizeBase: string;
  fontSizeLarge: string;
  lineHeight: number;
}

export interface ClscTokens {
  colors: ClscColors;
  spacing: ClscSpacing;
  shape: ClscShape;
  typography: ClscTypography;
}

export interface ClscTheme {
  mode: ColorMode;
  density: Density;
  highContrast: boolean;
  reducedMotion: boolean;
  tokens: ClscTokens;
}

const lightColors: ClscColors = {
  primary: '#0066ff',
  primaryHover: '#0052cc',
  background: '#f4f6f8',
  surface: '#ffffff',
  surfaceElevated: '#ffffff',
  text: '#1a1a2e',
  textMuted: '#5c5c6d',
  border: '#d9dce0',
  error: '#d92b2b',
  warning: '#f5a623',
  success: '#1fbf6a',
  info: '#0066ff',
  focusRing: '#0066ff',
};

const darkColors: ClscColors = {
  primary: '#4d94ff',
  primaryHover: '#80b3ff',
  background: '#0b0c15',
  surface: '#141626',
  surfaceElevated: '#1c1e33',
  text: '#f0f2f5',
  textMuted: '#a1a4b0',
  border: '#2c2f45',
  error: '#ff6b6b',
  warning: '#ffbe45',
  success: '#4cd97b',
  info: '#4d94ff',
  focusRing: '#4d94ff',
};

const spacing: ClscSpacing = {
  xsmall: '0.25rem',
  small: '0.5rem',
  medium: '1rem',
  large: '1.5rem',
  xlarge: '2rem',
};

const shape: ClscShape = {
  borderRadius: '0.375rem',
  borderRadiusLarge: '0.75rem',
};

const typography: ClscTypography = {
  fontFamily:
    '-apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", sans-serif',
  fontSizeSmall: '0.75rem',
  fontSizeBase: '0.875rem',
  fontSizeLarge: '1rem',
  lineHeight: 1.5,
};

export function buildTokens(mode: ColorMode): ClscTokens {
  return {
    colors: mode === 'dark' ? darkColors : lightColors,
    spacing,
    shape,
    typography,
  };
}

export function createTheme(
  mode: ColorMode = 'light',
  density: Density = 'default',
  highContrast = false,
  reducedMotion = false,
): ClscTheme {
  const tokens = buildTokens(mode);
  return {
    mode,
    density,
    highContrast,
    reducedMotion,
    tokens,
  };
}

export function cssVariables(theme: ClscTheme): Record<string, string> {
  const { colors, spacing, shape, typography } = theme.tokens;
  return {
    '--clsc-color-primary': colors.primary,
    '--clsc-color-primary-hover': colors.primaryHover,
    '--clsc-color-background': colors.background,
    '--clsc-color-surface': colors.surface,
    '--clsc-color-surface-elevated': colors.surfaceElevated,
    '--clsc-color-text': colors.text,
    '--clsc-color-text-muted': colors.textMuted,
    '--clsc-color-border': colors.border,
    '--clsc-color-error': colors.error,
    '--clsc-color-warning': colors.warning,
    '--clsc-color-success': colors.success,
    '--clsc-color-info': colors.info,
    '--clsc-color-focus-ring': colors.focusRing,
    '--clsc-spacing-xsmall': spacing.xsmall,
    '--clsc-spacing-small': spacing.small,
    '--clsc-spacing-medium': spacing.medium,
    '--clsc-spacing-large': spacing.large,
    '--clsc-spacing-xlarge': spacing.xlarge,
    '--clsc-shape-radius': shape.borderRadius,
    '--clsc-shape-radius-large': shape.borderRadiusLarge,
    '--clsc-typography-font-family': typography.fontFamily,
    '--clsc-typography-font-size-small': typography.fontSizeSmall,
    '--clsc-typography-font-size-base': typography.fontSizeBase,
    '--clsc-typography-font-size-large': typography.fontSizeLarge,
    '--clsc-typography-line-height': String(typography.lineHeight),
    '--clsc-density-padding':
      theme.density === 'compact'
        ? '0.25rem 0.5rem'
        : theme.density === 'comfortable'
          ? '0.75rem 1.25rem'
          : '0.5rem 1rem',
    '--clsc-min-tap-size': '2.75rem',
  };
}
