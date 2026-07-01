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

export interface Organization {
  id: string;
  name: string;
  slug: string;
  owner_id: string;
  created_at: string;
}

export interface Team {
  id: string;
  org_id: string;
  name: string;
  created_at: string;
}

export interface TeamMember {
  id: string;
  team_id: string;
  user_id: string;
  role: string;
  email: string;
  name: string;
  created_at: string;
}

export interface Invitation {
  id: string;
  team_id: string;
  email: string;
  role: string;
  token: string;
  status: string;
  expires_at: string;
  created_at: string;
}

export interface TeamProject {
  project_id: string;
  team_id: string;
  name: string;
  slug: string;
  status: string;
}

export const organizationsApi = {
  list: () =>
    api<Organization[]>("/api/organizations"),

  get: (id: string) =>
    api<Organization>(`/api/organizations/${id}`),

  create: (data: { name: string }) =>
    api<Organization>("/api/organizations", { method: "POST", body: data }),

  update: (id: string, data: { name?: string }) =>
    api<Organization>(`/api/organizations/${id}`, { method: "PATCH", body: data }),

  delete: (id: string) =>
    api<{ deleted: boolean }>(`/api/organizations/${id}`, { method: "DELETE" }),

  listTeams: (orgId: string) =>
    api<Team[]>(`/api/organizations/${orgId}/teams`),

  createTeam: (orgId: string, data: { name: string }) =>
    api<Team>(`/api/organizations/${orgId}/teams`, { method: "POST", body: data }),

  getTeam: (orgId: string, teamId: string) =>
    api<Team>(`/api/organizations/${orgId}/teams/${teamId}`),

  updateTeam: (orgId: string, teamId: string, data: { name?: string }) =>
    api<Team>(`/api/organizations/${orgId}/teams/${teamId}`, { method: "PATCH", body: data }),

  deleteTeam: (orgId: string, teamId: string) =>
    api<{ deleted: boolean }>(`/api/organizations/${orgId}/teams/${teamId}`, { method: "DELETE" }),

  listMembers: (orgId: string, teamId: string) =>
    api<TeamMember[]>(`/api/organizations/${orgId}/teams/${teamId}/members`),

  addMember: (orgId: string, teamId: string, data: { email: string; role?: string }) =>
    api<TeamMember>(`/api/organizations/${orgId}/teams/${teamId}/members`, { method: "POST", body: data }),

  updateMemberRole: (orgId: string, teamId: string, userId: string, data: { role: string }) =>
    api<{ updated: boolean }>(`/api/organizations/${orgId}/teams/${teamId}/members/${userId}`, { method: "PATCH", body: data }),

  removeMember: (orgId: string, teamId: string, userId: string) =>
    api<{ deleted: boolean }>(`/api/organizations/${orgId}/teams/${teamId}/members/${userId}`, { method: "DELETE" }),

  listInvitations: (orgId: string, teamId: string) =>
    api<Invitation[]>(`/api/organizations/${orgId}/teams/${teamId}/invitations`),

  createInvitation: (orgId: string, teamId: string, data: { email: string; role?: string }) =>
    api<Invitation>(`/api/organizations/${orgId}/teams/${teamId}/invitations`, { method: "POST", body: data }),

  cancelInvitation: (orgId: string, teamId: string, invId: string) =>
    api<{ deleted: boolean }>(`/api/organizations/${orgId}/teams/${teamId}/invitations/${invId}`, { method: "DELETE" }),

  acceptInvitation: (token: string) =>
    api<{ accepted: boolean; team_id: string }>(`/api/organizations/invitations/${token}/accept`, { method: "POST" }),

  listTeamProjects: (orgId: string, teamId: string) =>
    api<TeamProject[]>(`/api/organizations/${orgId}/teams/${teamId}/projects`),

  assignProject: (orgId: string, teamId: string, projectId: string) =>
    api<{ assigned: boolean }>(`/api/organizations/${orgId}/teams/${teamId}/projects`, { method: "POST", body: { project_id: projectId } }),

  unassignProject: (orgId: string, teamId: string, projectId: string) =>
    api<{ deleted: boolean }>(`/api/organizations/${orgId}/teams/${teamId}/projects/${projectId}`, { method: "DELETE" }),
};

export interface MetricPoint {
  ts: string;
  cpu_pct: number;
  mem_used_bytes: number;
  mem_limit_bytes: number;
  net_rx_bytes: number;
  net_tx_bytes: number;
  block_read_bytes: number;
  block_write_bytes: number;
}

export interface MetricsResponse {
  range: string;
  resolution: string;
  points: MetricPoint[];
}

export type MetricsRange = "1h" | "6h" | "24h" | "7d" | "30d";

export interface QueryStat {
  query: string;
  calls: number;
  total_exec_time_ms: number;
  mean_exec_time_ms: number;
  rows: number;
}

export interface QueryStatsResponse {
  available: boolean;
  stats: QueryStat[];
}

export interface MetricAlert {
  id: string;
  project_id: string;
  metric: "cpu_pct" | "mem_pct";
  comparison: "gt" | "lt";
  threshold: number;
  enabled: boolean;
  triggered: boolean;
  last_triggered_at: string | null;
  created_at: string;
}

export interface Backup {
  id: string;
  project_id: string;
  file_path: string;
  size_bytes: number | null;
  status: string;
  kind: string;
  error: string | null;
  created_at: string;
  completed_at: string | null;
}

export interface BackupPolicy {
  enabled: boolean;
  schedule_hour: number;
  daily_keep: number;
  weekly_keep: number;
}

export const backupsApi = {
  list: (projectId: string) => api<Backup[]>(`/api/projects/${projectId}/backups`),

  create: (projectId: string) =>
    api<Backup>(`/api/projects/${projectId}/backups`, { method: "POST" }),

  delete: (projectId: string, backupId: string) =>
    api<{ deleted: boolean }>(`/api/projects/${projectId}/backups/${backupId}`, {
      method: "DELETE",
    }),

  restore: (projectId: string, backupId: string) =>
    api<{ restored: boolean }>(`/api/projects/${projectId}/backups/${backupId}/restore`, {
      method: "POST",
    }),

  restoreAsBranch: (projectId: string, backupId: string, name: string) =>
    api<Branch>(`/api/projects/${projectId}/backups/${backupId}/restore-as-branch`, {
      method: "POST",
      body: { name },
    }),

  getPolicy: (projectId: string) =>
    api<BackupPolicy>(`/api/projects/${projectId}/backup-policy`),

  updatePolicy: (projectId: string, data: Partial<BackupPolicy>) =>
    api<BackupPolicy>(`/api/projects/${projectId}/backup-policy`, {
      method: "PATCH",
      body: data,
    }),
};

export const metricsApi = {
  get: (projectId: string, range: MetricsRange) =>
    api<MetricsResponse>(`/api/projects/${projectId}/metrics?range=${range}`),

  queryStats: (projectId: string) =>
    api<QueryStatsResponse>(`/api/projects/${projectId}/query-stats`),

  listAlerts: (projectId: string) =>
    api<MetricAlert[]>(`/api/projects/${projectId}/alerts`),

  createAlert: (
    projectId: string,
    data: { metric: string; comparison?: string; threshold: number }
  ) => api<MetricAlert>(`/api/projects/${projectId}/alerts`, { method: "POST", body: data }),

  updateAlert: (
    projectId: string,
    alertId: string,
    data: { threshold?: number; comparison?: string; enabled?: boolean }
  ) =>
    api<MetricAlert>(`/api/projects/${projectId}/alerts/${alertId}`, {
      method: "PATCH",
      body: data,
    }),

  deleteAlert: (projectId: string, alertId: string) =>
    api<{ deleted: boolean }>(`/api/projects/${projectId}/alerts/${alertId}`, {
      method: "DELETE",
    }),
};

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
