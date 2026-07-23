import {
  createContext,
  useContext,
  useState,
  useCallback,
  useMemo,
  useEffect,
} from 'react';
import type { ReactNode } from 'react';
import {
  type ClscTheme,
  type ColorMode,
  type Density,
  createTheme,
  cssVariables,
} from './tokens';

export interface ThemeContextValue {
  theme: ClscTheme;
  mode: ColorMode;
  density: Density;
  highContrast: boolean;
  reducedMotion: boolean;
  setMode: (mode: ColorMode) => void;
  setDensity: (density: Density) => void;
  setHighContrast: (value: boolean) => void;
  setReducedMotion: (value: boolean) => void;
  toggleMode: () => void;
}

const ThemeContext = createContext<ThemeContextValue | undefined>(undefined);

export function useTheme(): ThemeContextValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) {
    throw new Error('useTheme must be used within ThemeProvider');
  }
  return ctx;
}

export interface ThemeProviderProps {
  children: ReactNode;
  defaultMode?: ColorMode;
  defaultDensity?: Density;
  defaultHighContrast?: boolean;
  defaultReducedMotion?: boolean;
}

export function ThemeProvider({
  children,
  defaultMode = 'light',
  defaultDensity = 'default',
  defaultHighContrast = false,
  defaultReducedMotion = false,
}: ThemeProviderProps): ReactNode {
  const [mode, setMode] = useState<ColorMode>(defaultMode);
  const [density, setDensity] = useState<Density>(defaultDensity);
  const [highContrast, setHighContrast] = useState(defaultHighContrast);
  const [reducedMotion, setReducedMotion] = useState(defaultReducedMotion);

  const toggleMode = useCallback(() => {
    setMode((prev) => (prev === 'light' ? 'dark' : 'light'));
  }, []);

  const theme = useMemo(
    () => createTheme(mode, density, highContrast, reducedMotion),
    [mode, density, highContrast, reducedMotion],
  );

  useEffect(() => {
    const root = document.documentElement;
    root.setAttribute('data-clsc-theme', mode);
    root.setAttribute('data-clsc-density', density);
    if (highContrast) root.setAttribute('data-clsc-high-contrast', 'true');
    else root.removeAttribute('data-clsc-high-contrast');
    if (reducedMotion) root.setAttribute('data-clsc-reduced-motion', 'true');
    else root.removeAttribute('data-clsc-reduced-motion');

    const vars = cssVariables(theme);
    for (const [key, value] of Object.entries(vars)) {
      root.style.setProperty(key, value);
    }
    return () => {
      for (const key of Object.keys(vars)) {
        root.style.removeProperty(key);
      }
    };
  }, [theme, mode, density, highContrast, reducedMotion]);

  const value = useMemo(
    () => ({
      theme,
      mode,
      density,
      highContrast,
      reducedMotion,
      setMode,
      setDensity,
      setHighContrast,
      setReducedMotion,
      toggleMode,
    }),
    [theme, mode, density, highContrast, reducedMotion, toggleMode],
  );

  return (
    <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>
  );
}
