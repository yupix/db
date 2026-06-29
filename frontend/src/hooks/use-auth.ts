"use client";

import { create } from "zustand";
import { authApi, type User } from "@/lib/api";

interface AuthState {
  user: User | null;
  token: string | null;
  isLoading: boolean;
  error: string | null;
  login: (email: string, password: string) => Promise<void>;
  register: (email: string, password: string, name: string) => Promise<void>;
  logout: () => void;
  loadUser: () => Promise<void>;
  setToken: (token: string) => void;
}

export const useAuth = create<AuthState>((set, get) => ({
  user: null,
  token: typeof window !== "undefined" ? localStorage.getItem("token") : null,
  isLoading: false,
  error: null,

  login: async (email, password) => {
    set({ isLoading: true, error: null });
    try {
      const res = await authApi.login({ email, password });
      localStorage.setItem("token", res.token);
      set({ user: res.user, token: res.token, isLoading: false });
    } catch (e) {
      set({ error: e instanceof Error ? e.message : "Login failed", isLoading: false });
      throw e;
    }
  },

  register: async (email, password, name) => {
    set({ isLoading: true, error: null });
    try {
      const res = await authApi.register({ email, password, name });
      localStorage.setItem("token", res.token);
      set({ user: res.user, token: res.token, isLoading: false });
    } catch (e) {
      set({ error: e instanceof Error ? e.message : "Registration failed", isLoading: false });
      throw e;
    }
  },

  logout: () => {
    localStorage.removeItem("token");
    set({ user: null, token: null });
  },

  loadUser: async () => {
    const token = get().token;
    if (!token) return;
    set({ isLoading: true });
    try {
      const user = await authApi.me(token);
      set({ user, isLoading: false });
    } catch {
      localStorage.removeItem("token");
      set({ user: null, token: null, isLoading: false });
    }
  },

  setToken: (token) => {
    localStorage.setItem("token", token);
    set({ token });
  },
}));
