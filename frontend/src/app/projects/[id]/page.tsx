"use client";

import { useState } from "react";
import { useParams, useRouter } from "next/navigation";
import { useProject } from "@/hooks/use-projects";
import { useProjectMutations } from "@/hooks/use-project-mutations";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import Link from "next/link";

export default function ProjectOverviewPage() {
  const { id } = useParams<{ id: string }>();
  const router = useRouter();
  const { data: project, isLoading } = useProject(id);
  const { start, stop, remove } = useProjectMutations(id);
  const [copied, setCopied] = useState<string | null>(null);
  const [connModal, setConnModal] = useState<{ label: string; value: string } | null>(null);

  const isPending = start.isPending || stop.isPending || remove.isPending;

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
    <div className="p-6 space-y-6 max-w-3xl">
      <Dialog open={!!connModal} onOpenChange={(o) => !o && setConnModal(null)}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>{connModal?.label}</DialogTitle>
          </DialogHeader>
          <div className="space-y-3">
            <code className="block w-full p-3 bg-muted rounded text-sm break-all select-all">
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

      <div>
        <h2 className="text-xl font-bold">Overview</h2>
        <p className="text-sm text-muted-foreground mt-1">{project.name}</p>
      </div>

      {/* Actions */}
      <Card>
        <CardHeader>
          <CardTitle>コンテナ操作</CardTitle>
        </CardHeader>
        <CardContent className="flex gap-2">
          {project.status === "stopped" && (
            <Button onClick={() => handleAction("start")} disabled={isPending}>起動</Button>
          )}
          {project.status === "running" && (
            <Button variant="outline" onClick={() => handleAction("stop")} disabled={isPending}>停止</Button>
          )}
          <Button variant="destructive" onClick={() => handleAction("delete")} disabled={isPending}>
            削除
          </Button>
          <Link href={`/projects/${id}/editor`}>
            <Button variant="outline" disabled={project.status !== "running"}>SQL エディタ</Button>
          </Link>
        </CardContent>
      </Card>

      {/* Connection */}
      <Card>
        <CardHeader>
          <CardTitle>接続情報</CardTitle>
          <CardDescription>アプリケーションから接続するための情報</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div>
            <Label className="text-xs text-muted-foreground">直接接続（Postgres）</Label>
            <div className="flex items-center gap-2 mt-1">
              <code className="flex-1 p-2 bg-muted rounded text-sm break-all">
                {project.connection_string}
              </code>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setConnModal({ label: "直接接続（Postgres）", value: project.connection_string })}
              >
                接続文字列
              </Button>
            </div>
            <p className="text-xs text-muted-foreground mt-1">Port: {project.port}</p>
          </div>

          {project.pooled_connection_string && (
            <div>
              <Label className="text-xs text-muted-foreground">プール接続（PgBouncer）推奨</Label>
              <div className="flex items-center gap-2 mt-1">
                <code className="flex-1 p-2 bg-muted rounded text-sm break-all">
                  {project.pooled_connection_string}
                </code>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setConnModal({ label: "プール接続（PgBouncer）", value: project.pooled_connection_string! })}
                >
                  接続文字列
                </Button>
              </div>
              <p className="text-xs text-muted-foreground mt-1">Port: {project.pgbouncer_port}</p>
            </div>
          )}

          <div className="grid grid-cols-2 gap-4 text-sm">
            <div>
              <Label className="text-xs text-muted-foreground">データベース</Label>
              <p className="font-mono">{project.db_name}</p>
            </div>
            <div>
              <Label className="text-xs text-muted-foreground">ユーザー</Label>
              <p className="font-mono">{project.db_user}</p>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
