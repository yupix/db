"use client";

import { useParams } from "next/navigation";
import { useProject } from "@/hooks/use-projects";
import { MetricsCard } from "@/components/charts/metrics-card";
import { QueryStatsCard } from "@/components/charts/query-stats-card";
import { AlertsCard } from "@/components/charts/alerts-card";

export default function MonitoringPage() {
  const { id } = useParams<{ id: string }>();
  const { data: project } = useProject(id);
  const running = project?.status === "running";

  return (
    <div className="p-6 space-y-6">
      <div>
        <h2 className="text-xl font-bold">Monitoring</h2>
        <p className="text-sm text-muted-foreground mt-1">
          リソース使用状況とクエリ統計
        </p>
      </div>

      <MetricsCard projectId={id} running={running} />
      <AlertsCard projectId={id} />
      <QueryStatsCard projectId={id} running={running} />
    </div>
  );
}
