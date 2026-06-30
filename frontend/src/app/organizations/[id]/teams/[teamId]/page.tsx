"use client";

import { useState } from "react";
import { useParams } from "next/navigation";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { organizationsApi, ApiError } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";
import Link from "next/link";

const ROLES = ["owner", "admin", "developer", "viewer"] as const;

const roleColors: Record<string, string> = {
  owner: "bg-purple-500",
  admin: "bg-blue-500",
  developer: "bg-green-500",
  viewer: "bg-gray-500",
};

export default function TeamDetailPage() {
  const { id: orgId, teamId } = useParams<{ id: string; teamId: string }>();
  const qc = useQueryClient();

  // Members tab state
  const [memberEmail, setMemberEmail] = useState("");
  const [memberRole, setMemberRole] = useState("developer");
  const [memberError, setMemberError] = useState("");

  // Invitation tab state
  const [invEmail, setInvEmail] = useState("");
  const [invRole, setInvRole] = useState("developer");
  const [invError, setInvError] = useState("");

  // Project assign state
  const [projectId, setProjectId] = useState("");
  const [projectError, setProjectError] = useState("");

  const { data: team } = useQuery({
    queryKey: ["team", orgId, teamId],
    queryFn: () => organizationsApi.getTeam(orgId, teamId),
  });

  const { data: members, isLoading: membersLoading } = useQuery({
    queryKey: ["members", orgId, teamId],
    queryFn: () => organizationsApi.listMembers(orgId, teamId),
  });

  const { data: invitations, isLoading: invLoading } = useQuery({
    queryKey: ["invitations", orgId, teamId],
    queryFn: () => organizationsApi.listInvitations(orgId, teamId),
  });

  const { data: teamProjects, isLoading: projLoading } = useQuery({
    queryKey: ["teamProjects", orgId, teamId],
    queryFn: () => organizationsApi.listTeamProjects(orgId, teamId),
  });

  const addMember = useMutation({
    mutationFn: () =>
      organizationsApi.addMember(orgId, teamId, { email: memberEmail, role: memberRole }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["members", orgId, teamId] });
      setMemberEmail("");
      setMemberError("");
    },
    onError: (e) => {
      setMemberError(e instanceof ApiError ? e.message : "追加に失敗しました");
    },
  });

  const removeMember = useMutation({
    mutationFn: (userId: string) => organizationsApi.removeMember(orgId, teamId, userId),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["members", orgId, teamId] }),
  });

  const updateRole = useMutation({
    mutationFn: ({ userId, role }: { userId: string; role: string }) =>
      organizationsApi.updateMemberRole(orgId, teamId, userId, { role }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["members", orgId, teamId] }),
  });

  const createInvitation = useMutation({
    mutationFn: () =>
      organizationsApi.createInvitation(orgId, teamId, { email: invEmail, role: invRole }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["invitations", orgId, teamId] });
      setInvEmail("");
      setInvError("");
    },
    onError: (e) => {
      setInvError(e instanceof ApiError ? e.message : "招待に失敗しました");
    },
  });

  const cancelInvitation = useMutation({
    mutationFn: (invId: string) => organizationsApi.cancelInvitation(orgId, teamId, invId),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["invitations", orgId, teamId] }),
  });

  const assignProject = useMutation({
    mutationFn: () => organizationsApi.assignProject(orgId, teamId, projectId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["teamProjects", orgId, teamId] });
      setProjectId("");
      setProjectError("");
    },
    onError: (e) => {
      setProjectError(e instanceof ApiError ? e.message : "割り当てに失敗しました");
    },
  });

  const unassignProject = useMutation({
    mutationFn: (pid: string) => organizationsApi.unassignProject(orgId, teamId, pid),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["teamProjects", orgId, teamId] }),
  });

  return (
    <div className="min-h-screen bg-background">
      <header className="border-b">
        <div className="container mx-auto px-4 py-4 flex items-center gap-4">
          <Link
            href={`/organizations/${orgId}`}
            className="text-muted-foreground hover:text-foreground text-sm"
          >
            ← 組織へ戻る
          </Link>
          <h1 className="text-2xl font-bold">{team?.name ?? "チーム"}</h1>
        </div>
      </header>

      <main className="container mx-auto px-4 py-8">
        <Tabs defaultValue="members">
          <TabsList className="mb-6">
            <TabsTrigger value="members">メンバー</TabsTrigger>
            <TabsTrigger value="invitations">招待</TabsTrigger>
            <TabsTrigger value="projects">プロジェクト</TabsTrigger>
          </TabsList>

          {/* Members Tab */}
          <TabsContent value="members" className="space-y-6">
            <Card>
              <CardHeader>
                <CardTitle>メンバーを追加</CardTitle>
              </CardHeader>
              <CardContent>
                <form
                  onSubmit={(e) => {
                    e.preventDefault();
                    setMemberError("");
                    addMember.mutate();
                  }}
                  className="flex gap-2 items-end"
                >
                  <div className="flex-1 space-y-1">
                    <Label htmlFor="member-email">メールアドレス</Label>
                    <Input
                      id="member-email"
                      type="email"
                      value={memberEmail}
                      onChange={(e) => setMemberEmail(e.target.value)}
                      placeholder="user@example.com"
                      required
                    />
                  </div>
                  <div className="w-36 space-y-1">
                    <Label>ロール</Label>
                    <Select value={memberRole} onValueChange={(v) => v && setMemberRole(v)}>
                      <SelectTrigger>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {ROLES.map((r) => (
                          <SelectItem key={r} value={r}>
                            {r}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                  <Button type="submit" disabled={addMember.isPending}>
                    追加
                  </Button>
                </form>
                {memberError && <p className="text-sm text-destructive mt-2">{memberError}</p>}
              </CardContent>
            </Card>

            {membersLoading ? (
              <p className="text-muted-foreground">読み込み中...</p>
            ) : members && members.length > 0 ? (
              <div className="space-y-2">
                {members.map((m) => (
                  <Card key={m.id}>
                    <CardContent className="py-3 flex items-center justify-between">
                      <div>
                        <p className="font-medium">{m.name}</p>
                        <p className="text-sm text-muted-foreground">{m.email}</p>
                      </div>
                      <div className="flex items-center gap-2">
                        <Select
                          value={m.role}
                          onValueChange={(role) =>
                            role && updateRole.mutate({ userId: m.user_id, role })
                          }
                        >
                          <SelectTrigger className="w-28">
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            {ROLES.map((r) => (
                              <SelectItem key={r} value={r}>
                                {r}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                        <Badge className={roleColors[m.role] || "bg-gray-500"}>{m.role}</Badge>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => {
                            if (confirm(`${m.name} をチームから削除しますか？`)) {
                              removeMember.mutate(m.user_id);
                            }
                          }}
                        >
                          削除
                        </Button>
                      </div>
                    </CardContent>
                  </Card>
                ))}
              </div>
            ) : (
              <Card>
                <CardContent className="py-8 text-center text-muted-foreground">
                  メンバーがいません
                </CardContent>
              </Card>
            )}
          </TabsContent>

          {/* Invitations Tab */}
          <TabsContent value="invitations" className="space-y-6">
            <Card>
              <CardHeader>
                <CardTitle>招待を送る</CardTitle>
              </CardHeader>
              <CardContent>
                <form
                  onSubmit={(e) => {
                    e.preventDefault();
                    setInvError("");
                    createInvitation.mutate();
                  }}
                  className="flex gap-2 items-end"
                >
                  <div className="flex-1 space-y-1">
                    <Label htmlFor="inv-email">メールアドレス</Label>
                    <Input
                      id="inv-email"
                      type="email"
                      value={invEmail}
                      onChange={(e) => setInvEmail(e.target.value)}
                      placeholder="user@example.com"
                      required
                    />
                  </div>
                  <div className="w-36 space-y-1">
                    <Label>ロール</Label>
                    <Select value={invRole} onValueChange={(v) => v && setInvRole(v)}>
                      <SelectTrigger>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {ROLES.map((r) => (
                          <SelectItem key={r} value={r}>
                            {r}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                  <Button type="submit" disabled={createInvitation.isPending}>
                    招待
                  </Button>
                </form>
                {invError && <p className="text-sm text-destructive mt-2">{invError}</p>}
              </CardContent>
            </Card>

            {invLoading ? (
              <p className="text-muted-foreground">読み込み中...</p>
            ) : invitations && invitations.length > 0 ? (
              <div className="space-y-2">
                {invitations.map((inv) => (
                  <Card key={inv.id}>
                    <CardContent className="py-3 flex items-center justify-between">
                      <div>
                        <p className="font-medium">{inv.email}</p>
                        <p className="text-sm text-muted-foreground font-mono text-xs mt-1">
                          Token: {inv.token}
                        </p>
                        <p className="text-xs text-muted-foreground">
                          有効期限: {new Date(inv.expires_at).toLocaleDateString("ja-JP")}
                        </p>
                      </div>
                      <div className="flex items-center gap-2">
                        <Badge className={roleColors[inv.role] || "bg-gray-500"}>{inv.role}</Badge>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => cancelInvitation.mutate(inv.id)}
                        >
                          キャンセル
                        </Button>
                      </div>
                    </CardContent>
                  </Card>
                ))}
              </div>
            ) : (
              <Card>
                <CardContent className="py-8 text-center text-muted-foreground">
                  保留中の招待はありません
                </CardContent>
              </Card>
            )}
          </TabsContent>

          {/* Projects Tab */}
          <TabsContent value="projects" className="space-y-6">
            <Card>
              <CardHeader>
                <CardTitle>プロジェクトを割り当て</CardTitle>
              </CardHeader>
              <CardContent>
                <form
                  onSubmit={(e) => {
                    e.preventDefault();
                    setProjectError("");
                    assignProject.mutate();
                  }}
                  className="flex gap-2 items-end"
                >
                  <div className="flex-1 space-y-1">
                    <Label htmlFor="project-id">プロジェクト ID</Label>
                    <Input
                      id="project-id"
                      value={projectId}
                      onChange={(e) => setProjectId(e.target.value)}
                      placeholder="UUID..."
                      required
                    />
                  </div>
                  <Button type="submit" disabled={assignProject.isPending}>
                    割り当て
                  </Button>
                </form>
                {projectError && <p className="text-sm text-destructive mt-2">{projectError}</p>}
              </CardContent>
            </Card>

            {projLoading ? (
              <p className="text-muted-foreground">読み込み中...</p>
            ) : teamProjects && teamProjects.length > 0 ? (
              <div className="space-y-2">
                {teamProjects.map((p) => (
                  <Card key={p.project_id}>
                    <CardContent className="py-3 flex items-center justify-between">
                      <div>
                        <p className="font-medium">{p.name}</p>
                        <p className="text-sm text-muted-foreground">{p.slug}</p>
                      </div>
                      <div className="flex items-center gap-2">
                        <Badge
                          className={
                            p.status === "running" ? "bg-green-500" : "bg-gray-500"
                          }
                        >
                          {p.status}
                        </Badge>
                        <Link href={`/projects/${p.project_id}`}>
                          <Button variant="outline" size="sm">
                            開く
                          </Button>
                        </Link>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => {
                            if (confirm("このプロジェクトの割り当てを解除しますか？")) {
                              unassignProject.mutate(p.project_id);
                            }
                          }}
                        >
                          解除
                        </Button>
                      </div>
                    </CardContent>
                  </Card>
                ))}
              </div>
            ) : (
              <Card>
                <CardContent className="py-8 text-center text-muted-foreground">
                  割り当てられたプロジェクトはありません
                </CardContent>
              </Card>
            )}
          </TabsContent>
        </Tabs>
      </main>
    </div>
  );
}
