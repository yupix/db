"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "@/hooks/use-auth";
import { useProjects } from "@/hooks/use-projects";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { AppShell } from "@/components/app-shell";
import Link from "next/link";
import { Database, Plus, Server, HardDrive, Activity, CircleDot, ChevronRight } from "lucide-react";

const statusDot: Record<string, string> = {
  running: "bg-emerald-500",
  stopped: "bg-amber-500",
  creating: "bg-blue-500 animate-pulse",
  error: "bg-red-500",
};

export default function DashboardPage() {
  const { loadUser, isAuthenticated, isLoading: authLoading } = useAuth();
  const router = useRouter();
  const { data: projects, isLoading } = useProjects();

  useEffect(() => {
    if (!isAuthenticated && !authLoading) {
      loadUser().catch(() => router.push("/login"));
    }
  }, [isAuthenticated, authLoading, loadUser, router]);

  if (authLoading || !isAuthenticated) return null;

  const total = projects?.length ?? 0;
  const running = projects?.filter((p) => p.status === "running").length ?? 0;
  const stopped = projects?.filter((p) => p.status === "stopped").length ?? 0;

  const stats = [
    { label: "プロジェクト", value: total, Icon: Database, tint: "text-foreground" },
    { label: "稼働中", value: running, Icon: Activity, tint: "text-emerald-600" },
    { label: "停止中", value: stopped, Icon: CircleDot, tint: "text-amber-600" },
  ];

  return (
    <AppShell>
      {/* Page header */}
      <div className="sticky top-0 z-10 bg-background/80 backdrop-blur border-b">
        <div className="px-6 h-16 flex items-center justify-between">
          <div>
            <h1 className="text-lg font-bold">プロジェクト</h1>
            <p className="text-xs text-muted-foreground mt-0.5">
              PostgreSQL インスタンスの作成と管理
            </p>
          </div>
          <Link href="/projects/new">
            <Button className="gap-1.5">
              <Plus className="size-4" />
              新規プロジェクト
            </Button>
          </Link>
        </div>
      </div>

      <div className="p-6 space-y-6">
        {/* Stats */}
        <div className="grid grid-cols-3 gap-4">
          {stats.map(({ label, value, Icon, tint }) => (
            <Card key={label}>
              <CardContent className="p-4 flex items-center justify-between">
                <div>
                  <p className="text-xs text-muted-foreground">{label}</p>
                  <p className={`text-2xl font-bold tabular-nums mt-0.5 ${tint}`}>{value}</p>
                </div>
                <div className="size-9 rounded-lg bg-muted flex items-center justify-center">
                  <Icon className={`size-4 ${tint}`} />
                </div>
              </CardContent>
            </Card>
          ))}
        </div>

        {/* Project table */}
        {isLoading ? (
          <div className="rounded-xl border divide-y">
            {[...Array(4)].map((_, i) => (
              <div key={i} className="h-14 bg-card animate-pulse" />
            ))}
          </div>
        ) : projects && projects.length > 0 ? (
          <div className="rounded-xl border overflow-hidden bg-card">
            {/* header row */}
            <div className="grid grid-cols-[1fr_120px_100px_1fr_32px] gap-4 px-4 py-2.5 border-b bg-muted/40 text-xs font-medium text-muted-foreground">
              <span>名前</span>
              <span>ステータス</span>
              <span>ポート</span>
              <span>データベース</span>
              <span />
            </div>
            <div className="divide-y">
              {projects.map((project) => (
                <Link
                  key={project.id}
                  href={`/projects/${project.id}`}
                  className="group grid grid-cols-[1fr_120px_100px_1fr_32px] gap-4 px-4 py-3 items-center hover:bg-muted/40 transition-colors"
                >
                  <div className="flex items-center gap-2.5 min-w-0">
                    <div className="size-8 rounded-lg bg-accent flex items-center justify-center shrink-0">
                      <Database className="size-4 text-accent-foreground" />
                    </div>
                    <div className="min-w-0">
                      <p className="font-medium truncate group-hover:text-primary transition-colors">
                        {project.name}
                      </p>
                      <p className="text-xs text-muted-foreground truncate font-mono">{project.slug}</p>
                    </div>
                  </div>
                  <div className="flex items-center gap-1.5">
                    <span className={`size-1.5 rounded-full ${statusDot[project.status] ?? "bg-gray-400"}`} />
                    <span className="text-xs text-muted-foreground capitalize">{project.status}</span>
                  </div>
                  <span className="text-sm font-mono text-muted-foreground flex items-center gap-1.5">
                    <Server className="size-3.5" />:{project.port}
                  </span>
                  <span className="text-sm font-mono text-muted-foreground truncate flex items-center gap-1.5">
                    <HardDrive className="size-3.5 shrink-0" />
                    {project.db_name}
                  </span>
                  <ChevronRight className="size-4 text-muted-foreground/40 group-hover:text-primary group-hover:translate-x-0.5 transition-all" />
                </Link>
              ))}
            </div>
          </div>
        ) : (
          <Card className="border-dashed">
            <CardContent className="py-16 text-center">
              <div className="size-12 rounded-xl bg-accent flex items-center justify-center mx-auto mb-4">
                <Database className="size-6 text-accent-foreground" />
              </div>
              <p className="font-medium mb-1">プロジェクトがありません</p>
              <p className="text-sm text-muted-foreground mb-5">
                最初の PostgreSQL プロジェクトを作成しましょう
              </p>
              <Link href="/projects/new">
                <Button className="gap-1.5">
                  <Plus className="size-4" />
                  プロジェクトを作成
                </Button>
              </Link>
            </CardContent>
          </Card>
        )}
      </div>
    </AppShell>
  );
}
