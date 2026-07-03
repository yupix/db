"use client";

import { useParams } from "next/navigation";
import { useProject } from "@/hooks/use-projects";
import { QueryStatsCard } from "@/components/charts/query-stats-card";
import { ProjectPageHeader } from "@/components/project-page-header";

export default function MonitoringPage() {
  const { id } = useParams<{ id: string }>();
  const { data: project } = useProject(id);
  const running = project?.status === "running";

  return (
    <div>
      <ProjectPageHeader title="クエリ統計" description="pg_stat_statements による実行時間の統計" />
      <div className="p-6 space-y-6">
        <QueryStatsCard projectId={id} running={running} />
      </div>
    </div>
  );
}
