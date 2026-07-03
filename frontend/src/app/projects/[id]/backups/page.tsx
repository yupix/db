"use client";

import { useParams } from "next/navigation";
import { useProject } from "@/hooks/use-projects";
import { BackupsCard } from "@/components/backups/backups-card";
import { ProjectPageHeader } from "@/components/project-page-header";

export default function BackupsPage() {
  const { id } = useParams<{ id: string }>();
  const { data: project } = useProject(id);

  return (
    <div>
      <ProjectPageHeader title="Backups" description="スケジュールバックアップの管理と復元" />
      <div className="p-6 space-y-6">
        {project && <BackupsCard projectId={id} running={project.status === "running"} />}
      </div>
    </div>
  );
}
