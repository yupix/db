"use client";

import { useQuery } from "@tanstack/react-query";
import { projectsApi, type Project } from "@/lib/api";
import { useAuth } from "@/hooks/use-auth";

export function useProjects() {
  const token = useAuth((s) => s.token);

  return useQuery<Project[]>({
    queryKey: ["projects"],
    queryFn: () => projectsApi.list(token!),
    enabled: !!token,
  });
}

export function useProject(id: string) {
  const token = useAuth((s) => s.token);

  return useQuery<Project>({
    queryKey: ["project", id],
    queryFn: () => projectsApi.get(id, token!),
    enabled: !!token && !!id,
  });
}
