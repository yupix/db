"use client";

import { useParams } from "next/navigation";
import { useProject } from "@/hooks/use-projects";
import { MetricsCard } from "@/components/charts/metrics-card";
import { QueryStatsCard } from "@/components/charts/query-stats-card";
import { AlertsCard } from "@/components/charts/alerts-card";
import { ProjectPageHeader } from "@/components/project-page-header";

export default function MonitoringPage() {
  const { id } = useParams<{ id: string }>();
  const { data: project } = useProject(id);
  const running = project?.status === "running";

  return (
    <div>
      <ProjectPageHeader title="Monitoring" description="リソース使用状況とクエリ統計" />
      <div className="p-6 space-y-6">
        <MetricsCard projectId={id} running={running} />
        <AlertsCard projectId={id} />
        <QueryStatsCard projectId={id} running={running} />
      </div>
    </div>
  );
}
