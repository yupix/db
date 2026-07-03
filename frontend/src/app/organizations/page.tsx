"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { useAuth } from "@/hooks/use-auth";
import { organizationsApi } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { AppShell } from "@/components/app-shell";
import { Users } from "lucide-react";
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
    <AppShell>
      <div className="sticky top-0 z-10 bg-background/80 backdrop-blur border-b">
        <div className="px-6 h-16 flex items-center justify-between">
          <div>
            <h1 className="text-lg font-bold">組織 / チーム</h1>
            <p className="text-xs text-muted-foreground mt-0.5">組織とチームメンバーの管理</p>
          </div>
          <Link href="/organizations/new">
            <Button className="gap-1.5">
              <Users className="size-4" />
              新規組織
            </Button>
          </Link>
        </div>
      </div>

      <div className="p-6">
        {isLoading ? (
          <p className="text-muted-foreground">読み込み中...</p>
        ) : orgs && orgs.length > 0 ? (
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
            {orgs.map((org) => (
              <Link key={org.id} href={`/organizations/${org.id}`}>
                <Card className="group hover:border-primary/40 hover:shadow-md transition-all cursor-pointer">
                  <CardContent className="p-5">
                    <div className="flex items-center gap-3">
                      <div className="size-9 rounded-lg bg-accent flex items-center justify-center shrink-0">
                        <Users className="size-4 text-accent-foreground" />
                      </div>
                      <div className="min-w-0">
                        <p className="font-semibold truncate group-hover:text-primary transition-colors">{org.name}</p>
                        <p className="text-xs text-muted-foreground truncate font-mono">{org.slug}</p>
                      </div>
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
                <Users className="size-6 text-accent-foreground" />
              </div>
              <p className="font-medium mb-1">組織がありません</p>
              <p className="text-sm text-muted-foreground mb-5">組織を作成してチームで DB を管理しましょう</p>
              <Link href="/organizations/new">
                <Button className="gap-1.5">
                  <Users className="size-4" />
                  組織を作成
                </Button>
              </Link>
            </CardContent>
          </Card>
        )}
      </div>
    </AppShell>
  );
}
