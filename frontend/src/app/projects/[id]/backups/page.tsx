"use client";

import { useParams } from "next/navigation";
import { useProject } from "@/hooks/use-projects";
import { BackupsCard } from "@/components/backups/backups-card";

export default function BackupsPage() {
  const { id } = useParams<{ id: string }>();
  const { data: project } = useProject(id);

  return (
    <div className="p-6 space-y-6">
      <div>
        <h2 className="text-xl font-bold">Backups</h2>
        <p className="text-sm text-muted-foreground mt-1">
          スケジュールバックアップの管理と復元
        </p>
      </div>

      {project && <BackupsCard projectId={id} running={project.status === "running"} />}
    </div>
  );
}
