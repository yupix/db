const API_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080";

interface ApiOptions {
  method?: string;
  body?: unknown;
  token?: string;
}

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string
  ) {
    super(message);
    this.name = "ApiError";
  }
}

export async function api<T>(path: string, options: ApiOptions = {}): Promise<T> {
  const { method = "GET", body, token } = options;

  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };

  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }

  const res = await fetch(`${API_URL}${path}`, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
    credentials: "include",
  });

  if (!res.ok) {
    const errorBody = await res.json().catch(() => ({ error: "Unknown error" }));
    throw new ApiError(res.status, errorBody.error || `HTTP ${res.status}`);
  }

  return res.json();
}

export interface User {
  id: string;
  email: string;
  name: string;
}

export interface AuthResponse {
  token: string;
  user: User;
}

export interface Project {
  id: string;
  name: string;
  slug: string;
  status: string;
  port: number;
  db_name: string;
  db_user: string;
  connection_string: string;
  created_at: string;
}

export const authApi = {
  register: (data: { email: string; password: string; name: string }) =>
    api<AuthResponse>("/api/auth/register", { method: "POST", body: data }),

  login: (data: { email: string; password: string }) =>
    api<AuthResponse>("/api/auth/login", { method: "POST", body: data }),

  me: (token: string) =>
    api<User>("/api/auth/me", { token }),
};

export const projectsApi = {
  list: (token: string) =>
    api<Project[]>("/api/projects", { token }),

  get: (id: string, token: string) =>
    api<Project>(`/api/projects/${id}`, { token }),

  create: (data: { name: string }, token: string) =>
    api<Project>("/api/projects", { method: "POST", body: data, token }),

  delete: (id: string, token: string) =>
    api<{ deleted: boolean }>(`/api/projects/${id}`, { method: "DELETE", token }),

  start: (id: string, token: string) =>
    api<Project>(`/api/projects/${id}/start`, { method: "POST", token }),

  stop: (id: string, token: string) =>
    api<Project>(`/api/projects/${id}/stop`, { method: "POST", token }),

  update: (id: string, data: { name?: string }, token: string) =>
    api<Project>(`/api/projects/${id}`, { method: "PATCH", body: data, token }),
};
