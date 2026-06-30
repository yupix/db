"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { useAuth } from "@/hooks/use-auth";
import { organizationsApi } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import Link from "next/link";

export default function OrganizationsPage() {
  const { isAuthenticated, isLoading: authLoading, loadUser } = useAuth();
  const router = useRouter();

  useEffect(() => {
    if (!isAuthenticated && !authLoading) {
      loadUser().catch(() => router.push("/login"));
    }
  }, [isAuthenticated, authLoading, loadUser, router]);

  const { data: orgs, isLoading } = useQuery({
    queryKey: ["organizations"],
    queryFn: () => organizationsApi.list(),
    enabled: isAuthenticated,
  });

  if (authLoading || !isAuthenticated) return null;

  return (
    <div className="min-h-screen bg-background">
      <header className="border-b">
        <div className="container mx-auto px-4 py-4 flex items-center justify-between">
          <div className="flex items-center gap-4">
            <Link href="/dashboard" className="text-muted-foreground hover:text-foreground text-sm">
              ← ダッシュボード
            </Link>
            <h1 className="text-2xl font-bold">組織</h1>
          </div>
          <Link href="/organizations/new">
            <Button>新規組織を作成</Button>
          </Link>
        </div>
      </header>

      <main className="container mx-auto px-4 py-8">
        {isLoading ? (
          <p className="text-muted-foreground">読み込み中...</p>
        ) : orgs && orgs.length > 0 ? (
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
            {orgs.map((org) => (
              <Link key={org.id} href={`/organizations/${org.id}`}>
                <Card className="hover:shadow-md transition-shadow cursor-pointer">
                  <CardHeader>
                    <CardTitle className="text-lg">{org.name}</CardTitle>
                  </CardHeader>
                  <CardContent>
                    <p className="text-sm text-muted-foreground">{org.slug}</p>
                  </CardContent>
                </Card>
              </Link>
            ))}
          </div>
        ) : (
          <Card>
            <CardContent className="py-12 text-center">
              <p className="text-muted-foreground mb-4">組織がありません</p>
              <Link href="/organizations/new">
                <Button>最初の組織を作成</Button>
              </Link>
            </CardContent>
          </Card>
        )}
      </main>
    </div>
  );
}
