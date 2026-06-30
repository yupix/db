"use client";

import { useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { organizationsApi, ApiError } from "@/lib/api";
import { useAuth, apiWithRefresh } from "@/hooks/use-auth";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import Link from "next/link";

export default function OrganizationDetailPage() {
  const { id } = useParams<{ id: string }>();
  const router = useRouter();
  const qc = useQueryClient();
  const { isAuthenticated, isLoading: authLoading, loadUser } = useAuth();
  const [teamName, setTeamName] = useState("");
  const [teamError, setTeamError] = useState("");
  const [openDialog, setOpenDialog] = useState(false);

  useEffect(() => {
    if (!isAuthenticated && !authLoading) {
      loadUser().catch(() => router.push("/login"));
    }
  }, [isAuthenticated, authLoading, loadUser, router]);

  const {
    data: org,
    isLoading: orgLoading,
    error: orgError,
  } = useQuery({
    queryKey: ["organization", id],
    queryFn: () => apiWithRefresh(() => organizationsApi.get(id)),
    enabled: isAuthenticated,
  });

  const { data: teams, isLoading: teamsLoading } = useQuery({
    queryKey: ["teams", id],
    queryFn: () => apiWithRefresh(() => organizationsApi.listTeams(id)),
    enabled: isAuthenticated,
  });

  const createTeam = useMutation({
    mutationFn: () => organizationsApi.createTeam(id, { name: teamName }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["teams", id] });
      setTeamName("");
      setOpenDialog(false);
    },
    onError: (e) => {
      setTeamError(e instanceof ApiError ? e.message : "作成に失敗しました");
    },
  });

  const deleteOrg = useMutation({
    mutationFn: () => organizationsApi.delete(id),
    onSuccess: () => router.push("/organizations"),
  });

  if (authLoading || !isAuthenticated) return null;
  if (orgLoading) return <p className="p-8 text-muted-foreground">読み込み中...</p>;
  if (orgError) {
    const status = orgError instanceof ApiError ? orgError.status : 0;
    const message =
      status === 403
        ? "この組織へのアクセス権限がありません"
        : status === 404
          ? "組織が見つかりません"
          : "組織の読み込みに失敗しました";
    return (
      <div className="p-8 space-y-4">
        <p className="text-destructive">{message}</p>
        <Link href="/organizations" className="text-sm text-muted-foreground hover:text-foreground">
          ← 組織一覧へ戻る
        </Link>
      </div>
    );
  }
  if (!org) return <p className="p-8 text-destructive">組織が見つかりません</p>;

  return (
    <div className="min-h-screen bg-background">
      <header className="border-b">
        <div className="container mx-auto px-4 py-4 flex items-center justify-between">
          <div className="flex items-center gap-4">
            <Link href="/organizations" className="text-muted-foreground hover:text-foreground text-sm">
              ← 組織一覧
            </Link>
            <h1 className="text-2xl font-bold">{org.name}</h1>
            <span className="text-sm text-muted-foreground">{org.slug}</span>
          </div>
          <div className="flex gap-2">
            <Dialog open={openDialog} onOpenChange={setOpenDialog}>
              <DialogTrigger>
                <Button>チームを作成</Button>
              </DialogTrigger>
              <DialogContent>
                <DialogHeader>
                  <DialogTitle>新規チームを作成</DialogTitle>
                </DialogHeader>
                <form
                  onSubmit={(e) => {
                    e.preventDefault();
                    setTeamError("");
                    createTeam.mutate();
                  }}
                  className="space-y-4"
                >
                  <div className="space-y-2">
                    <Label htmlFor="team-name">チーム名</Label>
                    <Input
                      id="team-name"
                      value={teamName}
                      onChange={(e) => setTeamName(e.target.value)}
                      placeholder="Engineering"
                      required
                    />
                  </div>
                  {teamError && <p className="text-sm text-destructive">{teamError}</p>}
                  <Button type="submit" disabled={createTeam.isPending} className="w-full">
                    {createTeam.isPending ? "作成中..." : "作成"}
                  </Button>
                </form>
              </DialogContent>
            </Dialog>

            <Button
              variant="destructive"
              size="sm"
              onClick={() => {
                if (confirm("この組織を削除しますか？")) deleteOrg.mutate();
              }}
              disabled={deleteOrg.isPending}
            >
              削除
            </Button>
          </div>
        </div>
      </header>

      <main className="container mx-auto px-4 py-8">
        <h2 className="text-xl font-semibold mb-4">チーム</h2>

        {teamsLoading ? (
          <p className="text-muted-foreground">読み込み中...</p>
        ) : teams && teams.length > 0 ? (
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
            {teams.map((team) => (
              <Link key={team.id} href={`/organizations/${id}/teams/${team.id}`}>
                <Card className="hover:shadow-md transition-shadow cursor-pointer">
                  <CardHeader>
                    <CardTitle className="text-lg">{team.name}</CardTitle>
                  </CardHeader>
                  <CardContent>
                    <p className="text-sm text-muted-foreground">
                      作成: {new Date(team.created_at).toLocaleDateString("ja-JP")}
                    </p>
                  </CardContent>
                </Card>
              </Link>
            ))}
          </div>
        ) : (
          <Card>
            <CardContent className="py-12 text-center">
              <p className="text-muted-foreground">チームがありません</p>
            </CardContent>
          </Card>
        )}
      </main>
    </div>
  );
}
