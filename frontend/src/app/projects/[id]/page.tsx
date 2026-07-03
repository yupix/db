"use client";

import { useState } from "react";
import { useParams, useRouter } from "next/navigation";
import { useProject } from "@/hooks/use-projects";
import { useProjectMutations } from "@/hooks/use-project-mutations";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { MetricsCard } from "@/components/charts/metrics-card";
import { AlertsCard } from "@/components/charts/alerts-card";
import Link from "next/link";

export default function ProjectOverviewPage() {
  const { id } = useParams<{ id: string }>();
  const router = useRouter();
  const { data: project, isLoading } = useProject(id);
  const { start, stop, remove } = useProjectMutations(id);
  const [copied, setCopied] = useState<string | null>(null);
  const [connModal, setConnModal] = useState<{ label: string; value: string } | null>(null);

  const isPending = start.isPending || stop.isPending || remove.isPending;

  const statusDot: Record<string, string> = {
    running: "bg-emerald-500",
    stopped: "bg-amber-500",
    creating: "bg-blue-500 animate-pulse",
    resetting: "bg-purple-500 animate-pulse",
    error: "bg-red-500",
  };

  if (isLoading) return <div className="p-8 text-muted-foreground">読み込み中...</div>;
  if (!project) return <div className="p-8">プロジェクトが見つかりません</div>;

  const copyToClipboard = (text: string, key: string) => {
    if (navigator.clipboard) {
      navigator.clipboard.writeText(text).catch(() => fallbackCopy(text));
    } else {
      fallbackCopy(text);
    }
    setCopied(key);
    setTimeout(() => setCopied(null), 2000);
  };

  const fallbackCopy = (text: string) => {
    const el = document.createElement("textarea");
    el.value = text;
    el.style.position = "fixed";
    el.style.opacity = "0";
    document.body.appendChild(el);
    el.select();
    document.execCommand("copy");
    document.body.removeChild(el);
  };

  const handleAction = (action: "start" | "stop" | "delete") => {
    if (action === "delete") {
      if (!confirm("本当に削除しますか？")) return;
      remove.mutate(undefined, { onSuccess: () => router.push("/dashboard") });
      return;
    }
    if (action === "start") start.mutate();
    if (action === "stop") stop.mutate();
  };

  return (
    <div>
      <Dialog open={!!connModal} onOpenChange={(o) => !o && setConnModal(null)}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>{connModal?.label}</DialogTitle>
          </DialogHeader>
          <div className="space-y-3">
            <code className="block w-full p-3 bg-muted rounded-lg text-sm break-all select-all font-mono">
              {connModal?.value}
            </code>
            <Button
              className="w-full"
              onClick={() => connModal && copyToClipboard(connModal.value, "modal")}
            >
              {copied === "modal" ? "コピー済み!" : "クリップボードにコピー"}
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      {/* Page header */}
      <div className="sticky top-0 z-10 bg-background/80 backdrop-blur border-b">
        <div className="px-6 h-16 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <h1 className="text-lg font-bold">Overview</h1>
            <span className="inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs">
              <span className={`size-1.5 rounded-full ${statusDot[project.status] ?? "bg-gray-400"}`} />
              <span className="capitalize text-muted-foreground">{project.status}</span>
            </span>
          </div>
          <div className="flex gap-2">
            {project.status === "stopped" && (
              <Button size="sm" onClick={() => handleAction("start")} disabled={isPending}>起動</Button>
            )}
            {project.status === "running" && (
              <Button size="sm" variant="outline" onClick={() => handleAction("stop")} disabled={isPending}>停止</Button>
            )}
            <Link href={`/projects/${id}/editor`}>
              <Button size="sm" variant="outline" disabled={project.status !== "running"}>SQL エディタ</Button>
            </Link>
            <Button size="sm" variant="destructive" onClick={() => handleAction("delete")} disabled={isPending}>
              削除
            </Button>
          </div>
        </div>
      </div>

      <div className="p-6 space-y-6 max-w-3xl">
        {/* Quick facts */}
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
          {[
            { label: "ポート", value: `:${project.port}` },
            { label: "プールポート", value: project.pgbouncer_port ? `:${project.pgbouncer_port}` : "—" },
            { label: "データベース", value: project.db_name },
            { label: "ユーザー", value: project.db_user },
          ].map((f) => (
            <Card key={f.label}>
              <CardContent className="p-4">
                <p className="text-xs text-muted-foreground">{f.label}</p>
                <p className="font-mono text-sm mt-1 truncate">{f.value}</p>
              </CardContent>
            </Card>
          ))}
        </div>

        {/* Connection */}
        <Card>
          <CardHeader>
            <CardTitle>接続情報</CardTitle>
            <CardDescription>アプリケーションから接続するための情報</CardDescription>
          </CardHeader>
          <CardContent className="space-y-5">
            <div>
              <Label className="text-xs text-muted-foreground">直接接続（Postgres）</Label>
              <div className="flex items-center gap-2 mt-1.5">
                <code className="flex-1 p-2.5 bg-muted rounded-lg text-sm break-all font-mono">
                  {project.connection_string}
                </code>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setConnModal({ label: "直接接続（Postgres）", value: project.connection_string })}
                >
                  表示
                </Button>
              </div>
            </div>

            {project.pooled_connection_string && (
              <div>
                <Label className="text-xs text-muted-foreground flex items-center gap-1.5">
                  プール接続（PgBouncer）
                  <span className="rounded bg-primary/10 text-primary px-1.5 py-0.5 text-[10px] font-medium">推奨</span>
                </Label>
                <div className="flex items-center gap-2 mt-1.5">
                  <code className="flex-1 p-2.5 bg-muted rounded-lg text-sm break-all font-mono">
                    {project.pooled_connection_string}
                  </code>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => setConnModal({ label: "プール接続（PgBouncer）", value: project.pooled_connection_string! })}
                  >
                    表示
                  </Button>
                </div>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Monitoring */}
        <MetricsCard projectId={id} running={project.status === "running"} />
        <AlertsCard projectId={id} />
      </div>
    </div>
  );
}
