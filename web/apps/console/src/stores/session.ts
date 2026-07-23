import { create } from 'zustand';
import { queryClient } from '../api/queryClient.ts';

export interface SessionState {
  tenant: string | undefined;
  accessToken: string | undefined;
  refreshToken: string | undefined;
  setTenant: (tenant: string) => void;
  setTokens: (accessToken: string, refreshToken?: string) => void;
  clearSession: () => void;
}

export const useSessionStore = create<SessionState>((set, get) => ({
  tenant: undefined,
  accessToken: undefined,
  refreshToken: undefined,
  setTenant: (tenant) => {
    const previous = get().tenant;
    set({ tenant });
    if (previous) {
      queryClient.removeQueries({ queryKey: ['tenant', previous] });
    }
  },
  setTokens: (accessToken, refreshToken) => set({ accessToken, refreshToken }),
  clearSession: () => set({ accessToken: undefined, refreshToken: undefined }),
}));
