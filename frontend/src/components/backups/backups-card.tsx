"use client";

import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { backupsApi, ApiError } from "@/lib/api";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";

const statusColors: Record<string, string> = {
  creating: "bg-blue-500",
  completed: "bg-green-500",
  failed: "bg-red-500",
  restoring: "bg-purple-500",
};

function formatBytes(bytes: number | null): string {
  if (bytes === null || bytes <= 0) return "-";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}

export function BackupsCard({ projectId, running }: { projectId: string; running: boolean }) {
  const qc = useQueryClient();
  const [error, setError] = useState("");
  const [policyError, setPolicyError] = useState("");

  const { data: backups, isLoading } = useQuery({
    queryKey: ["backups", projectId],
    queryFn: () => backupsApi.list(projectId),
    // Poll while any backup is in-flight so "creating" flips to a final state
    // without the user needing to refresh.
    refetchInterval: (query) => {
      const list = query.state.data;
      return list?.some((b) => b.status === "creating" || b.status === "restoring")
        ? 5_000
        : 30_000;
    },
  });

  const { data: policy } = useQuery({
    queryKey: ["backup-policy", projectId],
    queryFn: () => backupsApi.getPolicy(projectId),
  });

  const createBackup = useMutation({
    mutationFn: () => backupsApi.create(projectId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["backups", projectId] });
      setError("");
    },
    onError: (e) => setError(e instanceof ApiError ? e.message : "バックアップ作成に失敗しました"),
  });

  const deleteBackup = useMutation({
    mutationFn: (id: string) => backupsApi.delete(projectId, id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["backups", projectId] });
      setError("");
    },
    onError: (e) => setError(e instanceof ApiError ? e.message : "削除に失敗しました"),
  });

  const restoreBackup = useMutation({
    mutationFn: (id: string) => backupsApi.restore(projectId, id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["backups", projectId] });
      setError("");
    },
    onError: (e) => setError(e instanceof ApiError ? e.message : "復元に失敗しました"),
  });

  const updatePolicy = useMutation({
    mutationFn: (data: Partial<typeof policy>) => backupsApi.updatePolicy(projectId, data ?? {}),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["backup-policy", projectId] });
      setPolicyError("");
    },
    onError: (e) =>
      setPolicyError(e instanceof ApiError ? e.message : "ポリシーの更新に失敗しました"),
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle>バックアップ</CardTitle>
        <CardDescription>
          pg_dump によるスナップショット。手動作成、または日次スケジュールで自動作成できます。
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">
        {/* Retention policy */}
        {policy && (
          <div className="border rounded p-3 space-y-3">
            <div className="flex items-center gap-2">
              <input
                type="checkbox"
                id="backup-enabled"
                checked={policy.enabled}
                onChange={(e) => updatePolicy.mutate({ enabled: e.target.checked })}
              />
              <Label htmlFor="backup-enabled">自動バックアップを有効化</Label>
            </div>
            <div className="grid grid-cols-3 gap-3">
              <div className="space-y-1">
                <Label htmlFor="schedule-hour" className="text-xs">
                  実行時刻 (UTC時)
                </Label>
                <Input
                  id="schedule-hour"
                  type="number"
                  min={0}
                  max={23}
                  defaultValue={policy.schedule_hour}
                  onBlur={(e) =>
                    updatePolicy.mutate({ schedule_hour: Number(e.target.value) || 0 })
                  }
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="daily-keep" className="text-xs">
                  日次保持数
                </Label>
                <Input
                  id="daily-keep"
                  type="number"
                  min={1}
                  max={60}
                  defaultValue={policy.daily_keep}
                  onBlur={(e) =>
                    updatePolicy.mutate({ daily_keep: Number(e.target.value) || 7 })
                  }
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="weekly-keep" className="text-xs">
                  週次保持数
                </Label>
                <Input
                  id="weekly-keep"
                  type="number"
                  min={0}
                  max={52}
                  defaultValue={policy.weekly_keep}
                  onBlur={(e) =>
                    updatePolicy.mutate({ weekly_keep: Number(e.target.value) || 4 })
                  }
                />
              </div>
            </div>
            {policyError && <p className="text-sm text-destructive">{policyError}</p>}
          </div>
        )}

        <div className="flex items-center justify-between">
          <p className="text-sm text-muted-foreground">
            {running ? "" : "プロジェクトが起動中のみバックアップを作成できます"}
          </p>
          <Button
            onClick={() => createBackup.mutate()}
            disabled={!running || createBackup.isPending}
          >
            {createBackup.isPending ? "作成中..." : "今すぐバックアップ"}
          </Button>
        </div>
        {error && <p className="text-sm text-destructive">{error}</p>}

        {isLoading ? (
          <p className="text-sm text-muted-foreground">読み込み中...</p>
        ) : backups && backups.length > 0 ? (
          <div className="space-y-2">
            {backups.map((b) => (
              <div key={b.id} className="flex items-center justify-between p-2 border rounded">
                <div className="flex items-center gap-2 min-w-0">
                  <Badge className={statusColors[b.status] || "bg-gray-500"}>{b.status}</Badge>
                  <Badge variant="secondary" className="text-xs">
                    {b.kind === "scheduled" ? "自動" : "手動"}
                  </Badge>
                  <span className="text-sm truncate">
                    {new Date(b.created_at).toLocaleString("ja-JP")}
                  </span>
                  <span className="text-xs text-muted-foreground shrink-0">
                    {formatBytes(b.size_bytes)}
                  </span>
                  {b.status === "failed" && b.error && (
                    <span className="text-xs text-destructive truncate">{b.error}</span>
                  )}
                </div>
                <div className="flex gap-1 shrink-0">
                  {b.status === "completed" && (
                    <Button
                      variant="outline"
                      size="sm"
                      disabled={!running || restoreBackup.isPending}
                      onClick={() => {
                        if (
                          confirm(
                            "このバックアップで現在のデータベースを上書きします。よろしいですか？"
                          )
                        ) {
                          restoreBackup.mutate(b.id);
                        }
                      }}
                    >
                      復元
                    </Button>
                  )}
                  <Button
                    variant="ghost"
                    size="sm"
                    className="text-red-500"
                    onClick={() => {
                      if (confirm("このバックアップを削除します。ファイルも完全に削除されます。よろしいですか？")) {
                        deleteBackup.mutate(b.id);
                      }
                    }}
                  >
                    削除
                  </Button>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground py-4 text-center">
            バックアップがありません
          </p>
        )}
      </CardContent>
    </Card>
  );
}
