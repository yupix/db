"use client";

import { useState, useEffect } from "react";
import { useParams } from "next/navigation";
import { projectsApi, type PoolSettings, type Environment } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

export default function SettingsPage() {
  const { id } = useParams<{ id: string }>();
  const [poolSettings, setPoolSettings] = useState<PoolSettings | null>(null);
  const [poolForm, setPoolForm] = useState({ pool_mode: "transaction", max_client_conn: 100, default_pool_size: 20 });
  const [poolLoading, setPoolLoading] = useState(false);
  const [environments, setEnvironments] = useState<Environment[]>([]);
  const [envLoading, setEnvLoading] = useState(false);
  const [envForm, setEnvForm] = useState({ name: "", endpoint_type: "direct", is_default: false });

  useEffect(() => {
    if (!id) return;
    projectsApi.getPoolSettings(id).then((s) => {
      setPoolSettings(s);
      setPoolForm({ pool_mode: s.pool_mode, max_client_conn: s.max_client_conn, default_pool_size: s.default_pool_size });
    }).catch(() => {});
    projectsApi.listEnvironments(id).then(setEnvironments).catch(() => {});
  }, [id]);

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

  const handleCreateEnv = async () => {
    if (!id) return;
    setEnvLoading(true);
    try {
      const env = await projectsApi.createEnvironment(id, envForm);
      const updated = env.is_default ? environments.map((e) => ({ ...e, is_default: false })) : environments;
      setEnvironments([...updated, env]);
      setEnvForm({ name: "", endpoint_type: "direct", is_default: false });
    } catch (e) {
      alert(e instanceof Error ? e.message : "環境の作成に失敗しました");
    } finally {
      setEnvLoading(false);
    }
  };

  const handleDeleteEnv = async (envId: string) => {
    if (!id || !confirm("この環境を削除しますか？")) return;
    try {
      await projectsApi.deleteEnvironment(id, envId);
      setEnvironments(environments.filter((e) => e.id !== envId));
    } catch (e) {
      alert(e instanceof Error ? e.message : "環境の削除に失敗しました");
    }
  };

  return (
    <div className="p-6 space-y-6 max-w-3xl">
      <div>
        <h2 className="text-xl font-bold">Settings</h2>
        <p className="text-sm text-muted-foreground mt-1">プール設定・エンドポイント環境の管理</p>
      </div>

      {/* Pool settings */}
      {poolSettings && (
        <Card>
          <CardHeader>
            <CardTitle>プール設定（PgBouncer）</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-1">
              <Label>プールモード</Label>
              <select
                className="w-full border rounded-md px-3 py-2 text-sm bg-background"
                value={poolForm.pool_mode}
                onChange={(e) => setPoolForm({ ...poolForm, pool_mode: e.target.value })}
              >
                <option value="session">session</option>
                <option value="transaction">transaction</option>
                <option value="statement">statement</option>
              </select>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-1">
                <Label>最大クライアント接続数</Label>
                <Input
                  type="number"
                  value={poolForm.max_client_conn}
                  onChange={(e) => setPoolForm({ ...poolForm, max_client_conn: Number(e.target.value) })}
                />
              </div>
              <div className="space-y-1">
                <Label>デフォルトプールサイズ</Label>
                <Input
                  type="number"
                  value={poolForm.default_pool_size}
                  onChange={(e) => setPoolForm({ ...poolForm, default_pool_size: Number(e.target.value) })}
                />
              </div>
            </div>
            <Button onClick={handleSavePool} disabled={poolLoading}>
              {poolLoading ? "保存中..." : "保存"}
            </Button>
          </CardContent>
        </Card>
      )}

      {/* Environments */}
      <Card>
        <CardHeader>
          <CardTitle>エンドポイント環境</CardTitle>
          <CardDescription>開発・本番などの接続先を管理します</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            {environments.map((env) => (
              <div key={env.id} className="flex items-center justify-between p-2 border rounded-lg">
                <div className="flex items-center gap-2 min-w-0">
                  <Badge variant={env.is_default ? "default" : "secondary"}>{env.name}</Badge>
                  <span className="text-xs text-muted-foreground">{env.endpoint_type}</span>
                  <code className="text-xs bg-muted px-1 py-0.5 rounded truncate max-w-[200px]">
                    {env.connection_string}
                  </code>
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-red-500 shrink-0"
                  onClick={() => handleDeleteEnv(env.id)}
                >
                  削除
                </Button>
              </div>
            ))}
          </div>

          <div className="border-t pt-4 space-y-3">
            <p className="text-sm font-medium">環境を追加</p>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1">
                <Label>名前</Label>
                <Input
                  value={envForm.name}
                  onChange={(e) => setEnvForm({ ...envForm, name: e.target.value })}
                  placeholder="production"
                />
              </div>
              <div className="space-y-1">
                <Label>タイプ</Label>
                <select
                  className="w-full border rounded-md px-3 py-2 text-sm bg-background"
                  value={envForm.endpoint_type}
                  onChange={(e) => setEnvForm({ ...envForm, endpoint_type: e.target.value })}
                >
                  <option value="direct">direct</option>
                  <option value="pooled">pooled</option>
                </select>
              </div>
            </div>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={envForm.is_default}
                onChange={(e) => setEnvForm({ ...envForm, is_default: e.target.checked })}
              />
              デフォルトに設定
            </label>
            <Button onClick={handleCreateEnv} disabled={envLoading || !envForm.name}>
              {envLoading ? "作成中..." : "追加"}
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
