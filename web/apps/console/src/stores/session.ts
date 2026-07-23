import { create } from 'zustand';
import { queryClient } from '../api/queryClient.ts';

export interface SessionState {
  tenant: string | undefined;
  accessToken: string | undefined;
  refreshToken: string | undefined;
  capabilities: readonly string[];
  setTenant: (tenant: string) => void;
  setTokens: (accessToken: string, refreshToken?: string) => void;
  setCapabilities: (capabilities: readonly string[]) => void;
  clearSession: () => void;
  logout: () => void;
}

export const useSessionStore = create<SessionState>((set, get) => ({
  tenant: undefined,
  accessToken: undefined,
  refreshToken: undefined,
  capabilities: [],
  setTenant: (tenant) => {
    const previous = get().tenant;
    set({ tenant });
    if (previous) {
      queryClient.removeQueries({ queryKey: ['tenant', previous] });
    }
  },
  setTokens: (accessToken, refreshToken) => set({ accessToken, refreshToken }),
  setCapabilities: (capabilities) => set({ capabilities }),
  clearSession: () =>
    set({ accessToken: undefined, refreshToken: undefined, capabilities: [] }),
  logout: () => {
    const previous = get().tenant;
    set({
      tenant: undefined,
      accessToken: undefined,
      refreshToken: undefined,
      capabilities: [],
    });
    queryClient.clear();
    if (previous) {
      queryClient.removeQueries({ queryKey: ['tenant', previous] });
    }
    if (typeof window !== 'undefined') {
      window.localStorage.removeItem('clsc.session');
    }
  },
}));
