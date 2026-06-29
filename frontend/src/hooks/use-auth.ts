"use client";

import { create } from "zustand";
import { authApi, type User, type ApiError } from "@/lib/api";

interface AuthState {
  user: User | null;
  isLoading: boolean;
  isAuthenticated: boolean;
  error: string | null;
  login: (email: string, password: string) => Promise<void>;
  register: (email: string, password: string, name: string) => Promise<void>;
  logout: () => Promise<void>;
  loadUser: () => Promise<boolean>;
  refresh: () => Promise<boolean>;
}

export const useAuth = create<AuthState>((set) => ({
  user: null,
  isLoading: false,
  isAuthenticated: false,
  error: null,

  login: async (email, password) => {
    set({ isLoading: true, error: null });
    try {
      const res = await authApi.login({ email, password });
      set({ user: res.user, isAuthenticated: true, isLoading: false });
    } catch (e) {
      set({ error: e instanceof Error ? e.message : "Login failed", isLoading: false });
      throw e;
    }
  },

  register: async (email, password, name) => {
    set({ isLoading: true, error: null });
    try {
      const res = await authApi.register({ email, password, name });
      set({ user: res.user, isAuthenticated: true, isLoading: false });
    } catch (e) {
      set({ error: e instanceof Error ? e.message : "Registration failed", isLoading: false });
      throw e;
    }
  },

  logout: async () => {
    try {
      await fetch(`${process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080"}/api/auth/logout`, {
        method: "POST",
        credentials: "include",
      });
    } catch {}
    set({ user: null, isAuthenticated: false });
  },

  loadUser: async () => {
    set({ isLoading: true });
    try {
      const user = await authApi.me();
      set({ user, isAuthenticated: true, isLoading: false });
      return true;
    } catch {
      set({ user: null, isAuthenticated: false, isLoading: false });
      return false;
    }
  },

  refresh: async () => {
    try {
      const res = await authApi.refresh();
      set({ user: res.user, isAuthenticated: true });
      return true;
    } catch {
      set({ user: null, isAuthenticated: false });
      return false;
    }
  },
}));

export async function apiWithRefresh<T>(fn: () => Promise<T>): Promise<T> {
  try {
    return await fn();
  } catch (e) {
    if (e instanceof Error && (e as ApiError).status === 401) {
      const refreshed = await useAuth.getState().refresh();
      if (refreshed) {
        return await fn();
      }
    }
    throw e;
  }
}
