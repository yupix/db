"use client";

import { useQuery } from "@tanstack/react-query";
import { projectsApi, type Project } from "@/lib/api";
import { useAuth } from "@/hooks/use-auth";

export function useProjects() {
  const isAuthenticated = useAuth((s) => s.isAuthenticated);

  return useQuery<Project[]>({
    queryKey: ["projects"],
    queryFn: () => projectsApi.list(),
    enabled: isAuthenticated,
  });
}

export function useProject(id: string) {
  const isAuthenticated = useAuth((s) => s.isAuthenticated);

  return useQuery<Project>({
    queryKey: ["project", id],
    queryFn: () => projectsApi.get(id),
    enabled: isAuthenticated && !!id,
  });
}
