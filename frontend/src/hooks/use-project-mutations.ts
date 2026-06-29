"use client";

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { projectsApi, type Project } from "@/lib/api";

export function useProjectMutations(projectId: string) {
  const queryClient = useQueryClient();

  const updateCache = (updater: (old: Project | undefined) => Project | undefined) => {
    queryClient.setQueryData<Project>(["project", projectId], updater);
    queryClient.setQueriesData<Project[]>({ queryKey: ["projects"] }, (old) =>
      old?.map((p) => (p.id === projectId ? (updater(p) ?? p) : p))
    );
  };

  const start = useMutation({
    mutationFn: () => projectsApi.start(projectId),
    onMutate: async () => {
      await queryClient.cancelQueries({ queryKey: ["project", projectId] });
      const previous = queryClient.getQueryData<Project>(["project", projectId]);
      updateCache((old) => old ? { ...old, status: "running" } : undefined);
      return { previous };
    },
    onError: (_err, _vars, context) => {
      if (context?.previous) {
        updateCache(() => context.previous);
      }
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ["project", projectId] });
      queryClient.invalidateQueries({ queryKey: ["projects"] });
    },
  });

  const stop = useMutation({
    mutationFn: () => projectsApi.stop(projectId),
    onMutate: async () => {
      await queryClient.cancelQueries({ queryKey: ["project", projectId] });
      const previous = queryClient.getQueryData<Project>(["project", projectId]);
      updateCache((old) => old ? { ...old, status: "stopped" } : undefined);
      return { previous };
    },
    onError: (_err, _vars, context) => {
      if (context?.previous) {
        updateCache(() => context.previous);
      }
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ["project", projectId] });
      queryClient.invalidateQueries({ queryKey: ["projects"] });
    },
  });

  const remove = useMutation({
    mutationFn: () => projectsApi.delete(projectId),
  });

  return { start, stop, remove };
}
