"use client";

import { useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import { useAuth } from "@/hooks/use-auth";
import { useProject } from "@/hooks/use-projects";
import { projectsApi } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Label } from "@/components/ui/label";
import Link from "next/link";

const statusColors: Record<string, string> = {
  running: "bg-green-500",
  stopped: "bg-yellow-500",
  creating: "bg-blue-500",
  error: "bg-red-500",
};

export default function ProjectDetailPage() {
  const { id } = useParams<{ id: string }>();
  const { isAuthenticated, loadUser, isLoading: authLoading } = useAuth();
  const router = useRouter();
  const { data: project, isLoading, refetch } = useProject(id);
  const [actionLoading, setActionLoading] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!isAuthenticated && !authLoading) {
      loadUser().catch(() => router.push("/login"));
    }
  }, [isAuthenticated, authLoading, loadUser, router]);

  const handleAction = async (action: "start" | "stop" | "delete") => {
    setActionLoading(true);
    try {
      if (action === "delete") {
        if (!confirm("本当に削除しますか？")) return;
        await projectsApi.delete(id);
        router.push("/dashboard");
        return;
      }
      if (action === "start") await projectsApi.start(id);
      if (action === "stop") await projectsApi.stop(id);
      refetch();
    } catch (e) {
      alert(e instanceof Error ? e.message : "操作に失敗しました");
    } finally {
      setActionLoading(false);
    }
  };

  const copyConnectionString = () => {
    if (project) {
      navigator.clipboard.writeText(project.connection_string);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  if (authLoading || !isAuthenticated) return null;
  if (isLoading) return <div className="p-8">読み込み中...</div>;
  if (!project) return <div className="p-8">プロジェクトが見つかりません</div>;

  return (
    <div className="min-h-screen bg-background">
      <header className="border-b">
        <div className="container mx-auto px-4 py-4">
          <Link href="/dashboard" className="text-sm text-muted-foreground hover:underline">
            &larr; ダッシュボードに戻る
          </Link>
        </div>
      </header>

      <main className="container mx-auto px-4 py-8 max-w-2xl">
        <div className="flex items-center justify-between mb-6">
          <div>
            <h1 className="text-2xl font-bold">{project.name}</h1>
            <p className="text-sm text-muted-foreground">{project.slug}</p>
          </div>
          <Badge className={statusColors[project.status] || "bg-gray-500"}>
            {project.status}
          </Badge>
        </div>

        <div className="grid gap-4">
          <Card>
            <CardHeader>
              <CardTitle>接続情報</CardTitle>
              <CardDescription>アプリケーションから接続するための情報</CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              <div>
                <Label className="text-xs text-muted-foreground">接続文字列</Label>
                <div className="flex items-center gap-2 mt-1">
                  <code className="flex-1 p-2 bg-muted rounded text-sm break-all">
                    {project.connection_string}
                  </code>
                  <Button variant="outline" size="sm" onClick={copyConnectionString}>
                    {copied ? "コピー済み!" : "コピー"}
                  </Button>
                </div>
              </div>
              <div className="grid grid-cols-3 gap-4 text-sm">
                <div>
                  <Label className="text-xs text-muted-foreground">ポート</Label>
                  <p className="font-mono">{project.port}</p>
                </div>
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

          <Card>
            <CardHeader>
              <CardTitle>操作</CardTitle>
            </CardHeader>
            <CardContent className="flex gap-2">
              {project.status === "stopped" && (
                <Button onClick={() => handleAction("start")} disabled={actionLoading}>
                  起動
                </Button>
              )}
              {project.status === "running" && (
                <Button variant="outline" onClick={() => handleAction("stop")} disabled={actionLoading}>
                  停止
                </Button>
              )}
              <Button
                variant="destructive"
                onClick={() => handleAction("delete")}
                disabled={actionLoading}
              >
                削除
              </Button>
            </CardContent>
          </Card>
        </div>
      </main>
    </div>
  );
}
