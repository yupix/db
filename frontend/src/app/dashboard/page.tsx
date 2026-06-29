"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "@/hooks/use-auth";
import { useProjects } from "@/hooks/use-projects";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import Link from "next/link";

const statusColors: Record<string, string> = {
  running: "bg-green-500",
  stopped: "bg-yellow-500",
  creating: "bg-blue-500",
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
      <header className="border-b">
        <div className="container mx-auto px-4 py-4 flex items-center justify-between">
          <h1 className="text-2xl font-bold">DB Console</h1>
          <div className="flex items-center gap-4">
            <span className="text-sm text-muted-foreground">{user?.email}</span>
            <Button variant="outline" onClick={logout}>
              ログアウト
            </Button>
          </div>
        </div>
      </header>

      <main className="container mx-auto px-4 py-8">
        <div className="flex items-center justify-between mb-8">
          <h2 className="text-xl font-semibold">プロジェクト</h2>
          <Link href="/projects/new">
            <Button>新規プロジェクト</Button>
          </Link>
        </div>

        {isLoading ? (
          <p className="text-muted-foreground">読み込み中...</p>
        ) : projects && projects.length > 0 ? (
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
            {projects.map((project) => (
              <Link key={project.id} href={`/projects/${project.id}`}>
                <Card className="hover:shadow-md transition-shadow cursor-pointer">
                  <CardHeader>
                    <div className="flex items-center justify-between">
                      <CardTitle className="text-lg">{project.name}</CardTitle>
                      <Badge className={statusColors[project.status] || "bg-gray-500"}>
                        {project.status}
                      </Badge>
                    </div>
                    <CardDescription>{project.slug}</CardDescription>
                  </CardHeader>
                  <CardContent>
                    <div className="text-sm text-muted-foreground space-y-1">
                      <p>Port: {project.port}</p>
                      <p>DB: {project.db_name}</p>
                    </div>
                  </CardContent>
                </Card>
              </Link>
            ))}
          </div>
        ) : (
          <Card>
            <CardContent className="py-12 text-center">
              <p className="text-muted-foreground mb-4">プロジェクトがありません</p>
              <Link href="/projects/new">
                <Button>最初のプロジェクトを作成</Button>
              </Link>
            </CardContent>
          </Card>
        )}
      </main>
    </div>
  );
}
