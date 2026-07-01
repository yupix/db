"use client";

import { useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import { useAuth } from "@/hooks/use-auth";
import { useProject } from "@/hooks/use-projects";
import { useProjectMutations } from "@/hooks/use-project-mutations";
import { projectsApi, type PoolSettings, type Environment, type Branch } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { MetricsCard } from "@/components/charts/metrics-card";
import { QueryStatsCard } from "@/components/charts/query-stats-card";
import Link from "next/link";

const statusColors: Record<string, string> = {
  running: "bg-green-500",
  stopped: "bg-yellow-500",
  creating: "bg-blue-500",
  resetting: "bg-purple-500",
  error: "bg-red-500",
};

export default function ProjectDetailPage() {
  const { id } = useParams<{ id: string }>();
  const { isAuthenticated, loadUser, isLoading: authLoading } = useAuth();
  const router = useRouter();
  const { data: project, isLoading } = useProject(id);
  const { start, stop, remove } = useProjectMutations(id);
  const [copied, setCopied] = useState<string | null>(null);
  const [poolSettings, setPoolSettings] = useState<PoolSettings | null>(null);
  const [poolLoading, setPoolLoading] = useState(false);
  const [poolForm, setPoolForm] = useState({
    pool_mode: "transaction",
    max_client_conn: 100,
    default_pool_size: 20,
  });
  const [environments, setEnvironments] = useState<Environment[]>([]);
  const [envLoading, setEnvLoading] = useState(false);
  const [envForm, setEnvForm] = useState({
    name: "",
    endpoint_type: "direct",
    is_default: false,
  });
  const [branches, setBranches] = useState<Branch[]>([]);
  const [branchLoading, setBranchLoading] = useState(false);
  const [branchForm, setBranchForm] = useState({
    name: "",
    parent_branch_id: "",
  });

  useEffect(() => {
    if (!isAuthenticated && !authLoading) {
      loadUser().catch(() => router.push("/login"));
    }
  }, [isAuthenticated, authLoading, loadUser, router]);

  useEffect(() => {
    if (id) {
      projectsApi.getPoolSettings(id).then((s) => {
        setPoolSettings(s);
        setPoolForm({
          pool_mode: s.pool_mode,
          max_client_conn: s.max_client_conn,
          default_pool_size: s.default_pool_size,
        });
      }).catch(() => {});
      projectsApi.listEnvironments(id).then(setEnvironments).catch(() => {});
      projectsApi.listBranches(id).then(setBranches).catch(() => {});
    }
  }, [id]);

  const handleAction = (action: "start" | "stop" | "delete") => {
    if (action === "delete") {
      if (!confirm("本当に削除しますか？")) return;
      remove.mutate(undefined, {
        onSuccess: () => router.push("/dashboard"),
        onError: (e) => alert(e instanceof Error ? e.message : "削除に失敗しました"),
      });
      return;
    }
    if (action === "start") {
      start.mutate(undefined, {
        onError: (e) => alert(e instanceof Error ? e.message : "起動に失敗しました"),
      });
    }
    if (action === "stop") {
      stop.mutate(undefined, {
        onError: (e) => alert(e instanceof Error ? e.message : "停止に失敗しました"),
      });
    }
  };

  const handleSavePool = async () => {
    if (!id) return;
    setPoolLoading(true);
    try {
      const updated = await projectsApi.updatePoolSettings(id, poolForm);
      setPoolSettings(updated);
    } catch (e) {
      alert(e instanceof Error ? e.message : "プール設定の更新に失敗しました");
    } finally {
      setPoolLoading(false);
    }
  };

  const handleCreateEnvironment = async () => {
    if (!id) return;
    setEnvLoading(true);
    try {
      const env = await projectsApi.createEnvironment(id, envForm);
      const updated = env.is_default
        ? environments.map((e) => ({ ...e, is_default: false }))
        : environments;
      setEnvironments([...updated, env]);
      setEnvForm({ name: "", endpoint_type: "direct", is_default: false });
    } catch (e) {
      alert(e instanceof Error ? e.message : "環境の作成に失敗しました");
    } finally {
      setEnvLoading(false);
    }
  };

  const handleDeleteEnvironment = async (envId: string) => {
    if (!id || !confirm("この環境を削除しますか？")) return;
    try {
      await projectsApi.deleteEnvironment(id, envId);
      setEnvironments(environments.filter((e) => e.id !== envId));
    } catch (e) {
      alert(e instanceof Error ? e.message : "環境の削除に失敗しました");
    }
  };

  const handleCreateBranch = async () => {
    if (!id) return;
    setBranchLoading(true);
    try {
      const branch = await projectsApi.createBranch(id, {
        name: branchForm.name,
        parent_branch_id: branchForm.parent_branch_id || undefined,
      });
      setBranches([...branches, branch]);
      setBranchForm({ name: "", parent_branch_id: "" });
    } catch (e) {
      alert(e instanceof Error ? e.message : "ブランチの作成に失敗しました");
    } finally {
      setBranchLoading(false);
    }
  };

  const handleDeleteBranch = async (branchId: string) => {
    if (!id || !confirm("このブランチを削除しますか？")) return;
    try {
      await projectsApi.deleteBranch(id, branchId);
      // DB側は ON DELETE SET NULL で子をルートに昇格させるので stateも追従
      setBranches((prev) =>
        prev
          .filter((b) => b.id !== branchId)
          .map((b) =>
            b.parent_branch_id === branchId ? { ...b, parent_branch_id: null } : b
          )
      );
    } catch (e) {
      alert(e instanceof Error ? e.message : "ブランチの削除に失敗しました");
    }
  };

  const handleResetBranch = async (branchId: string) => {
    if (!id || !confirm("このブランチをリセットしますか？データが上書きされます。")) return;
    try {
      const updated = await projectsApi.resetBranch(id, branchId);
      setBranches(branches.map((b) => (b.id === branchId ? updated : b)));
    } catch (e) {
      alert(e instanceof Error ? e.message : "ブランチのリセットに失敗しました");
    }
  };

  const copyToClipboard = (text: string, key: string) => {
    navigator.clipboard.writeText(text);
    setCopied(key);
    setTimeout(() => setCopied(null), 2000);
  };

  const isPending = start.isPending || stop.isPending || remove.isPending;

  if (authLoading || !isAuthenticated) return null;
  if (isLoading) return <div className="p-8">読み込み中...</div>;
  if (!project) return <div className="p-8">プロジェクトが見つかりません</div>;

  return (
    <div className="min-h-screen bg-background">
      <header className="border-b">
        <div className="container mx-auto px-4 py-4 flex items-center justify-between">
          <Link href="/dashboard" className="text-sm text-muted-foreground hover:underline">
            &larr; ダッシュボードに戻る
          </Link>
          {project.status === "running" && (
            <Link href={`/projects/${project.id}/editor`}>
              <Button variant="outline" size="sm">SQL エディタ &rarr;</Button>
            </Link>
          )}
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
          {/* Connection Info */}
          <Card>
            <CardHeader>
              <CardTitle>接続情報</CardTitle>
              <CardDescription>アプリケーションから接続するための情報</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {/* Direct connection */}
              <div>
                <Label className="text-xs text-muted-foreground">直接接続（Postgres）</Label>
                <div className="flex items-center gap-2 mt-1">
                  <code className="flex-1 p-2 bg-muted rounded text-sm break-all">
                    {project.connection_string}
                  </code>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => copyToClipboard(project.connection_string, "direct")}
                  >
                    {copied === "direct" ? "コピー済み!" : "コピー"}
                  </Button>
                </div>
                <p className="text-xs text-muted-foreground mt-1">Port: {project.port}</p>
              </div>

              {/* Pooled connection */}
              {project.pooled_connection_string && (
                <div>
                  <Label className="text-xs text-muted-foreground">
                    プール接続（PgBouncer）推奨
                  </Label>
                  <div className="flex items-center gap-2 mt-1">
                    <code className="flex-1 p-2 bg-muted rounded text-sm break-all">
                      {project.pooled_connection_string}
                    </code>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() =>
                        copyToClipboard(project.pooled_connection_string!, "pooled")
                      }
                    >
                      {copied === "pooled" ? "コピー済み!" : "コピー"}
                    </Button>
                  </div>
                  <p className="text-xs text-muted-foreground mt-1">
                    Port: {project.pgbouncer_port}
                  </p>
                </div>
              )}

              <div className="grid grid-cols-2 gap-4 text-sm">
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

          {/* Metrics */}
          <MetricsCard projectId={id} running={project.status === "running"} />

          {/* Query statistics */}
          <QueryStatsCard projectId={id} running={project.status === "running"} />

          {/* Pool Settings */}
          {poolSettings && (
            <Card>
              <CardHeader>
                <CardTitle>プール設定</CardTitle>
                <CardDescription>
                  PgBouncerのコネクションプール設定（変更は再起動後に反映されます）
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="space-y-2">
                  <Label htmlFor="pool_mode">プールモード</Label>
                  <select
                    id="pool_mode"
                    className="w-full p-2 border rounded bg-background"
                    value={poolForm.pool_mode}
                    onChange={(e) =>
                      setPoolForm({ ...poolForm, pool_mode: e.target.value })
                    }
                  >
                    <option value="session">session</option>
                    <option value="transaction">transaction</option>
                    <option value="statement">statement</option>
                  </select>
                </div>
                <div className="space-y-2">
                  <Label htmlFor="max_client_conn">最大クライアント接続数</Label>
                  <Input
                    id="max_client_conn"
                    type="number"
                    min={1}
                    max={10000}
                    value={poolForm.max_client_conn}
                    onChange={(e) =>
                      setPoolForm({
                        ...poolForm,
                        max_client_conn: parseInt(e.target.value) || 100,
                      })
                    }
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="default_pool_size">デフォルトプールサイズ</Label>
                  <Input
                    id="default_pool_size"
                    type="number"
                    min={1}
                    max={1000}
                    value={poolForm.default_pool_size}
                    onChange={(e) =>
                      setPoolForm({
                        ...poolForm,
                        default_pool_size: parseInt(e.target.value) || 20,
                      })
                    }
                  />
                </div>
                <Button onClick={handleSavePool} disabled={poolLoading}>
                  {poolLoading ? "保存中..." : "プール設定を保存"}
                </Button>
              </CardContent>
            </Card>
          )}

          {/* Environment Endpoints */}
          <Card>
            <CardHeader>
              <CardTitle>環境エンドポイント</CardTitle>
              <CardDescription>
                dev/staging/prod などの環境ラベルと接続先を管理
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {environments.length > 0 && (
                <div className="space-y-2">
                  {environments.map((env) => (
                    <div
                      key={env.id}
                      className="flex items-center justify-between p-2 border rounded"
                    >
                      <div className="flex items-center gap-2">
                        <Badge variant={env.is_default ? "default" : "secondary"}>
                          {env.name}
                        </Badge>
                        <span className="text-xs text-muted-foreground">
                          {env.endpoint_type}
                        </span>
                        <code className="text-xs bg-muted px-1 py-0.5 rounded truncate max-w-[200px]">
                          {env.connection_string}
                        </code>
                      </div>
                      <div className="flex gap-1">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() =>
                            copyToClipboard(env.connection_string, `env-${env.id}`)
                          }
                        >
                          {copied === `env-${env.id}` ? "コピー済み!" : "コピー"}
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          className="text-red-500"
                          onClick={() => handleDeleteEnvironment(env.id)}
                        >
                          削除
                        </Button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
              <div className="grid grid-cols-[1fr_auto_auto_auto] gap-2 items-end">
                <div className="space-y-1">
                  <Label htmlFor="env_name">環境名</Label>
                  <Input
                    id="env_name"
                    placeholder="development"
                    value={envForm.name}
                    onChange={(e) => setEnvForm({ ...envForm, name: e.target.value })}
                  />
                </div>
                <div className="space-y-1">
                  <Label htmlFor="env_type">タイプ</Label>
                  <select
                    id="env_type"
                    className="p-2 border rounded bg-background"
                    value={envForm.endpoint_type}
                    onChange={(e) =>
                      setEnvForm({ ...envForm, endpoint_type: e.target.value })
                    }
                  >
                    <option value="direct">direct</option>
                    <option value="pooled">pooled</option>
                  </select>
                </div>
                <div className="flex items-center gap-1 pb-1">
                  <input
                    type="checkbox"
                    id="env_default"
                    checked={envForm.is_default}
                    onChange={(e) =>
                      setEnvForm({ ...envForm, is_default: e.target.checked })
                    }
                  />
                  <Label htmlFor="env_default" className="text-xs">
                    デフォルト
                  </Label>
                </div>
                <Button
                  onClick={handleCreateEnvironment}
                  disabled={envLoading || !envForm.name}
                >
                  {envLoading ? "作成中..." : "追加"}
                </Button>
              </div>
            </CardContent>
          </Card>

          {/* Branches */}
          <Card>
            <CardHeader>
              <CardTitle>ブランチ</CardTitle>
              <CardDescription>
                データベースのコピー（ブランチ）を作成・管理。ツリー構造で親子関係を表示。
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {branches.length > 0 && (
                <BranchTree
                  branches={branches}
                  statusColors={statusColors}
                  copied={copied}
                  onCopy={copyToClipboard}
                  onReset={handleResetBranch}
                  onDelete={handleDeleteBranch}
                />
              )}
              <div className="grid grid-cols-[1fr_1fr_auto] gap-2 items-end">
                <div className="space-y-1">
                  <Label htmlFor="branch_name">ブランチ名</Label>
                  <Input
                    id="branch_name"
                    placeholder="my-branch"
                    value={branchForm.name}
                    onChange={(e) => setBranchForm({ ...branchForm, name: e.target.value })}
                  />
                </div>
                <div className="space-y-1">
                  <Label htmlFor="branch_parent">親ブランチ（任意）</Label>
                  <select
                    id="branch_parent"
                    className="p-2 border rounded bg-background w-full"
                    value={branchForm.parent_branch_id}
                    onChange={(e) =>
                      setBranchForm({ ...branchForm, parent_branch_id: e.target.value })
                    }
                  >
                    <option value="">メイン（プロジェクト本体）</option>
                    {branches
                      .filter((b) => b.status === "running")
                      .map((b) => (
                        <option key={b.id} value={b.id}>
                          {b.name}
                        </option>
                      ))}
                  </select>
                </div>
                <Button
                  onClick={handleCreateBranch}
                  disabled={branchLoading || !branchForm.name}
                >
                  {branchLoading ? "作成中..." : "ブランチ作成"}
                </Button>
              </div>
            </CardContent>
          </Card>

          {/* Actions */}
          <Card>
            <CardHeader>
              <CardTitle>操作</CardTitle>
            </CardHeader>
            <CardContent className="flex gap-2">
              {project.status === "stopped" && (
                <Button onClick={() => handleAction("start")} disabled={isPending}>
                  {start.isPending ? "起動中..." : "起動"}
                </Button>
              )}
              {project.status === "running" && (
                <Button
                  variant="outline"
                  onClick={() => handleAction("stop")}
                  disabled={isPending}
                >
                  {stop.isPending ? "停止中..." : "停止"}
                </Button>
              )}
              <Button
                variant="destructive"
                onClick={() => handleAction("delete")}
                disabled={isPending}
              >
                {remove.isPending ? "削除中..." : "削除"}
              </Button>
            </CardContent>
          </Card>
        </div>
      </main>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Branch Tree Component
// ---------------------------------------------------------------------------

interface BranchTreeProps {
  branches: Branch[];
  statusColors: Record<string, string>;
  copied: string | null;
  onCopy: (text: string, key: string) => void;
  onReset: (branchId: string) => void;
  onDelete: (branchId: string) => void;
}

function BranchTree({
  branches,
  statusColors,
  copied,
  onCopy,
  onReset,
  onDelete,
}: BranchTreeProps) {
  // Build tree: root branches (parent_branch_id == null or parent not in list) and their children
  const branchIds = new Set(branches.map((b) => b.id));
  const rootBranches = branches.filter(
    (b) => !b.parent_branch_id || !branchIds.has(b.parent_branch_id)
  );
  const childrenOf = (parentId: string) =>
    branches.filter((b) => b.parent_branch_id === parentId);

  const renderBranch = (branch: Branch, depth: number = 0) => {
    const children = childrenOf(branch.id);
    return (
      <div key={branch.id} className="space-y-1">
        <div
          className="flex items-center justify-between p-2 border rounded"
          style={{ marginLeft: `${depth * 24}px` }}
        >
          <div className="flex items-center gap-2 min-w-0">
            {depth > 0 && (
              <span className="text-muted-foreground text-sm select-none">└─</span>
            )}
            <Badge className={statusColors[branch.status] || "bg-gray-500"}>
              {branch.status}
            </Badge>
            <span className="font-medium truncate">{branch.name}</span>
            {depth === 0 && (
              <Badge variant="secondary" className="text-xs">top-level</Badge>
            )}
          </div>
          <div className="flex gap-1 shrink-0">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => onCopy(branch.connection_string, `branch-${branch.id}`)}
            >
              {copied === `branch-${branch.id}` ? "コピー済み!" : "コピー"}
            </Button>
            {branch.status === "running" && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => onReset(branch.id)}
              >
                リセット
              </Button>
            )}
            <Button
              variant="ghost"
              size="sm"
              className="text-red-500"
              onClick={() => onDelete(branch.id)}
            >
              削除
            </Button>
          </div>
        </div>
        {children.length > 0 && (
          <div className="space-y-1">
            {children.map((child) => renderBranch(child, depth + 1))}
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="space-y-1">
      {/* Main project as root */}
      <div className="flex items-center gap-2 p-2 border rounded bg-muted/50">
        <Badge className="bg-blue-500">main</Badge>
        <span className="font-medium">プロジェクト本体</span>
      </div>
      {rootBranches.map((branch) => renderBranch(branch, 0))}
    </div>
  );
}
