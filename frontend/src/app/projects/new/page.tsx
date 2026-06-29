"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "@/hooks/use-auth";
import { projectsApi } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import Link from "next/link";

export default function NewProjectPage() {
  const [name, setName] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { isAuthenticated, loadUser, isLoading: authLoading } = useAuth();
  const router = useRouter();

  useEffect(() => {
    if (!isAuthenticated && !authLoading) {
      loadUser().catch(() => router.push("/login"));
    }
  }, [isAuthenticated, authLoading, loadUser, router]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsLoading(true);
    setError(null);

    try {
      const project = await projectsApi.create({ name });
      router.push(`/projects/${project.id}`);
    } catch (e) {
      setError(e instanceof Error ? e.message : "作成に失敗しました");
    } finally {
      setIsLoading(false);
    }
  };

  if (authLoading || !isAuthenticated) return null;

  return (
    <div className="min-h-screen bg-background">
      <header className="border-b">
        <div className="container mx-auto px-4 py-4">
          <Link href="/dashboard" className="text-sm text-muted-foreground hover:underline">
            &larr; ダッシュボードに戻る
          </Link>
        </div>
      </header>

      <main className="container mx-auto px-4 py-8 max-w-lg">
        <Card>
          <CardHeader>
            <CardTitle>新規プロジェクト</CardTitle>
            <CardDescription>PostgreSQLデータベースを作成</CardDescription>
          </CardHeader>
          <CardContent>
            <form onSubmit={handleSubmit} className="space-y-4">
              {error && (
                <div className="text-sm text-red-500 bg-red-50 p-2 rounded">{error}</div>
              )}
              <div className="space-y-2">
                <Label htmlFor="name">プロジェクト名</Label>
                <Input
                  id="name"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="my-database"
                  required
                />
              </div>
              <Button type="submit" className="w-full" disabled={isLoading}>
                {isLoading ? "作成中..." : "プロジェクトを作成"}
              </Button>
            </form>
          </CardContent>
        </Card>
      </main>
    </div>
  );
}
