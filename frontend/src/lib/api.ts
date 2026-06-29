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
  user: User;
}

export interface Project {
  id: string;
  name: string;
  slug: string;
  status: string;
  port: number;
  pgbouncer_port: number | null;
  db_name: string;
  db_user: string;
  connection_string: string;
  pooled_connection_string: string | null;
  pool_mode: string;
  max_client_conn: number;
  default_pool_size: number;
  created_at: string;
}

export interface PoolSettings {
  pool_mode: string;
  max_client_conn: number;
  default_pool_size: number;
  pgbouncer_port: number | null;
}

export interface Environment {
  id: string;
  project_id: string;
  name: string;
  endpoint_type: string;
  connection_string: string;
  is_default: boolean;
}

export interface Branch {
  id: string;
  project_id: string;
  parent_branch_id: string | null;
  name: string;
  status: string;
  port: number;
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

  refresh: () =>
    api<AuthResponse>("/api/auth/refresh", { method: "POST", body: {} }),
};

export const projectsApi = {
  list: () =>
    api<Project[]>("/api/projects"),

  get: (id: string) =>
    api<Project>(`/api/projects/${id}`),

  create: (data: {
    name: string;
    pool_mode?: string;
    max_client_conn?: number;
    default_pool_size?: number;
  }) =>
    api<Project>("/api/projects", { method: "POST", body: data }),

  delete: (id: string) =>
    api<{ deleted: boolean }>(`/api/projects/${id}`, { method: "DELETE" }),

  start: (id: string) =>
    api<Project>(`/api/projects/${id}/start`, { method: "POST" }),

  stop: (id: string) =>
    api<Project>(`/api/projects/${id}/stop`, { method: "POST" }),

  update: (id: string, data: { name?: string }) =>
    api<Project>(`/api/projects/${id}`, { method: "PATCH", body: data }),

  getPoolSettings: (id: string) =>
    api<PoolSettings>(`/api/projects/${id}/pool`),

  updatePoolSettings: (
    id: string,
    data: { pool_mode?: string; max_client_conn?: number; default_pool_size?: number }
  ) =>
    api<PoolSettings>(`/api/projects/${id}/pool`, { method: "PATCH", body: data }),

  listEnvironments: (id: string) =>
    api<Environment[]>(`/api/projects/${id}/environments`),

  createEnvironment: (
    id: string,
    data: { name: string; endpoint_type?: string; is_default?: boolean }
  ) =>
    api<Environment>(`/api/projects/${id}/environments`, { method: "POST", body: data }),

  deleteEnvironment: (id: string, envId: string) =>
    api<{ deleted: boolean }>(`/api/projects/${id}/environments/${envId}`, { method: "DELETE" }),

  // Branch operations
  listBranches: (id: string) =>
    api<Branch[]>(`/api/projects/${id}/branches`),

  createBranch: (
    id: string,
    data: { name: string; parent_branch_id?: string }
  ) =>
    api<Branch>(`/api/projects/${id}/branches`, { method: "POST", body: data }),

  deleteBranch: (id: string, branchId: string) =>
    api<{ deleted: boolean }>(`/api/projects/${id}/branches/${branchId}`, { method: "DELETE" }),

  renameBranch: (id: string, branchId: string, data: { name: string }) =>
    api<Branch>(`/api/projects/${id}/branches/${branchId}`, { method: "PATCH", body: data }),

  resetBranch: (id: string, branchId: string) =>
    api<Branch>(`/api/projects/${id}/branches/${branchId}/reset`, { method: "POST" }),
};
