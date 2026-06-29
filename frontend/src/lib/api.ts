const API_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080";

interface ApiOptions {
  method?: string;
  body?: unknown;
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
  const { method = "GET", body } = options;

  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };

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
  access_token: string;
  refresh_token: string;
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

  me: () =>
    api<User>("/api/auth/me"),

  refresh: (refresh_token: string) =>
    api<AuthResponse>("/api/auth/refresh", { method: "POST", body: { refresh_token } }),
};

export const projectsApi = {
  list: () =>
    api<Project[]>("/api/projects"),

  get: (id: string) =>
    api<Project>(`/api/projects/${id}`),

  create: (data: { name: string }) =>
    api<Project>("/api/projects", { method: "POST", body: data }),

  delete: (id: string) =>
    api<{ deleted: boolean }>(`/api/projects/${id}`, { method: "DELETE" }),

  start: (id: string) =>
    api<Project>(`/api/projects/${id}/start`, { method: "POST" }),

  stop: (id: string) =>
    api<Project>(`/api/projects/${id}/stop`, { method: "POST" }),

  update: (id: string, data: { name?: string }) =>
    api<Project>(`/api/projects/${id}`, { method: "PATCH", body: data }),
};
