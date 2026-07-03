"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "@/hooks/use-auth";
import { useProjects } from "@/hooks/use-projects";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import Link from "next/link";
import { Database, Plus, Users, LogOut, Server, HardDrive } from "lucide-react";

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

  return (
    <div className="min-h-screen bg-background">
      {/* Header */}
      <header className="border-b bg-card/50 backdrop-blur sticky top-0 z-10">
        <div className="container mx-auto px-6 h-14 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className="size-7 rounded-md bg-primary flex items-center justify-center">
              <Database className="size-4 text-primary-foreground" />
            </div>
            <h1 className="text-base font-semibold">DB Console</h1>
          </div>
          <div className="flex items-center gap-3">
            <div className="size-7 rounded-full bg-accent flex items-center justify-center text-xs font-semibold text-accent-foreground">
              {user?.email?.[0]?.toUpperCase() ?? "?"}
            </div>
            <span className="text-sm text-muted-foreground hidden sm:inline">{user?.email}</span>
            <Button variant="ghost" size="sm" onClick={logout}>
              <LogOut className="size-4" />
            </Button>
          </div>
        </div>
      </header>

      <main className="container mx-auto px-6 py-8">
        {/* Tabs */}
        <div className="flex gap-6 border-b mb-8">
          <Link href="/dashboard" className="text-sm font-medium border-b-2 border-primary pb-3 -mb-px text-foreground">
            プロジェクト
          </Link>
          <Link href="/organizations" className="text-sm font-medium text-muted-foreground hover:text-foreground pb-3 flex items-center gap-1.5">
            <Users className="size-4" />
            組織 / チーム
          </Link>
        </div>

        <div className="flex items-center justify-between mb-6">
          <div>
            <h2 className="text-xl font-bold">プロジェクト</h2>
            <p className="text-sm text-muted-foreground mt-0.5">
              {projects?.length ?? 0} 個のプロジェクト
            </p>
          </div>
          <Link href="/projects/new">
            <Button className="gap-1.5">
              <Plus className="size-4" />
              新規プロジェクト
            </Button>
          </Link>
        </div>

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
                <Card className="group hover:border-primary/40 hover:shadow-md transition-all cursor-pointer h-full">
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
                          <p className="text-xs text-muted-foreground truncate">{project.slug}</p>
                        </div>
                      </div>
                      <div className="flex items-center gap-1.5 shrink-0">
                        <span className={`size-2 rounded-full ${statusDot[project.status] ?? "bg-gray-400"}`} />
                        <span className="text-xs text-muted-foreground capitalize">{project.status}</span>
                      </div>
                    </div>
                    <div className="flex items-center gap-4 text-xs text-muted-foreground">
                      <span className="flex items-center gap-1">
                        <Server className="size-3.5" />
                        :{project.port}
                      </span>
                      <span className="flex items-center gap-1">
                        <HardDrive className="size-3.5" />
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
