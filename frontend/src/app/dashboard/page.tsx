"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "@/hooks/use-auth";
import { useProjects } from "@/hooks/use-projects";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import Link from "next/link";
import { Database, Plus, Users, LogOut, Server, HardDrive, Activity, CircleDot } from "lucide-react";

const statusDot: Record<string, string> = {
  running: "bg-emerald-500",
  stopped: "bg-amber-500",
  creating: "bg-blue-500 animate-pulse",
  error: "bg-red-500",
};

export default function DashboardPage() {
  const { user, loadUser, logout, isAuthenticated, isLoading: authLoading } = useAuth();
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
    <div className="min-h-screen bg-background">
      {/* Header */}
      <header className="border-b bg-card/60 backdrop-blur sticky top-0 z-10">
        <div className="container mx-auto px-6 h-14 flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <div className="size-7 rounded-md bg-primary flex items-center justify-center">
              <Database className="size-4 text-primary-foreground" />
            </div>
            <span className="font-semibold tracking-tight">DB Console</span>
            <nav className="ml-6 hidden md:flex items-center gap-1 text-sm">
              <Link href="/dashboard" className="px-3 py-1.5 rounded-md bg-accent text-accent-foreground font-medium">
                プロジェクト
              </Link>
              <Link href="/organizations" className="px-3 py-1.5 rounded-md text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors">
                組織 / チーム
              </Link>
            </nav>
          </div>
          <div className="flex items-center gap-3">
            <span className="text-sm text-muted-foreground hidden sm:inline">{user?.email}</span>
            <div className="size-8 rounded-full bg-gradient-to-br from-primary to-emerald-700 flex items-center justify-center text-xs font-semibold text-primary-foreground ring-2 ring-background">
              {user?.email?.[0]?.toUpperCase() ?? "?"}
            </div>
            <Button variant="ghost" size="sm" onClick={logout} aria-label="ログアウト">
              <LogOut className="size-4" />
            </Button>
          </div>
        </div>
      </header>

      <main className="container mx-auto px-6 py-8 space-y-8">
        {/* Title + CTA */}
        <div className="flex items-end justify-between">
          <div>
            <h1 className="text-2xl font-bold tracking-tight">プロジェクト</h1>
            <p className="text-sm text-muted-foreground mt-1">
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

        {/* Project grid */}
        {isLoading ? (
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
            {[...Array(3)].map((_, i) => (
              <div key={i} className="h-36 rounded-xl border bg-card animate-pulse" />
            ))}
          </div>
        ) : projects && projects.length > 0 ? (
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
            {projects.map((project) => (
              <Link key={project.id} href={`/projects/${project.id}`}>
                <Card className="group relative overflow-hidden hover:border-primary/40 hover:shadow-md transition-all cursor-pointer h-full">
                  {/* top accent on hover */}
                  <div className="absolute inset-x-0 top-0 h-0.5 bg-primary scale-x-0 group-hover:scale-x-100 origin-left transition-transform" />
                  <CardContent className="p-5">
                    <div className="flex items-start justify-between mb-4">
                      <div className="flex items-center gap-2.5 min-w-0">
                        <div className="size-9 rounded-lg bg-accent flex items-center justify-center shrink-0">
                          <Database className="size-4 text-accent-foreground" />
                        </div>
                        <div className="min-w-0">
                          <p className="font-semibold truncate group-hover:text-primary transition-colors">
                            {project.name}
                          </p>
                          <p className="text-xs text-muted-foreground truncate font-mono">{project.slug}</p>
                        </div>
                      </div>
                      <div className="flex items-center gap-1.5 shrink-0 rounded-full border px-2 py-0.5">
                        <span className={`size-1.5 rounded-full ${statusDot[project.status] ?? "bg-gray-400"}`} />
                        <span className="text-xs text-muted-foreground capitalize">{project.status}</span>
                      </div>
                    </div>
                    <div className="flex items-center gap-4 text-xs text-muted-foreground border-t pt-3">
                      <span className="flex items-center gap-1.5 font-mono">
                        <Server className="size-3.5" />
                        :{project.port}
                      </span>
                      <span className="flex items-center gap-1.5 font-mono truncate">
                        <HardDrive className="size-3.5 shrink-0" />
                        {project.db_name}
                      </span>
                    </div>
                  </CardContent>
                </Card>
              </Link>
            ))}
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
      </main>
    </div>
  );
}
