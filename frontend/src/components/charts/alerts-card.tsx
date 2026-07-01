"use client";

import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { metricsApi, ApiError } from "@/lib/api";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
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

const METRICS = [
  { value: "cpu_pct", label: "CPU使用率" },
  { value: "mem_pct", label: "メモリ使用率" },
];

const COMPARISONS = [
  { value: "gt", label: "を超えたら" },
  { value: "lt", label: "を下回ったら" },
];

export function AlertsCard({ projectId }: { projectId: string }) {
  const qc = useQueryClient();
  const [metric, setMetric] = useState("cpu_pct");
  const [comparison, setComparison] = useState("gt");
  const [threshold, setThreshold] = useState(80);
  const [error, setError] = useState("");

  const { data: alerts, isLoading } = useQuery({
    queryKey: ["alerts", projectId],
    queryFn: () => metricsApi.listAlerts(projectId),
    // Poll so `triggered` reflects the collector's latest evaluation (30s cadence).
    refetchInterval: 30_000,
  });

  const createAlert = useMutation({
    mutationFn: () => metricsApi.createAlert(projectId, { metric, comparison, threshold }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["alerts", projectId] });
      setError("");
    },
    onError: (e) => setError(e instanceof ApiError ? e.message : "作成に失敗しました"),
  });

  const toggleAlert = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      metricsApi.updateAlert(projectId, id, { enabled }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["alerts", projectId] }),
  });

  const deleteAlert = useMutation({
    mutationFn: (id: string) => metricsApi.deleteAlert(projectId, id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["alerts", projectId] }),
  });

  const metricLabel = (m: string) => METRICS.find((x) => x.value === m)?.label ?? m;
  const comparisonLabel = (c: string) => COMPARISONS.find((x) => x.value === c)?.label ?? c;

  return (
    <Card>
      <CardHeader>
        <CardTitle>アラート閾値</CardTitle>
        <CardDescription>
          CPU・メモリ使用率がしきい値を超えたら通知対象としてマークします
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <form
          onSubmit={(e) => {
            e.preventDefault();
            createAlert.mutate();
          }}
          className="grid grid-cols-[1fr_1fr_auto_auto] gap-2 items-end"
        >
          <div className="space-y-1">
            <Label>メトリクス</Label>
            <Select value={metric} onValueChange={(v) => v && setMetric(v)}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {METRICS.map((m) => (
                  <SelectItem key={m.value} value={m.value}>
                    {m.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1">
            <Label>条件</Label>
            <Select value={comparison} onValueChange={(v) => v && setComparison(v)}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {COMPARISONS.map((c) => (
                  <SelectItem key={c.value} value={c.value}>
                    {c.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1 w-24">
            <Label htmlFor="threshold">しきい値 (%)</Label>
            <Input
              id="threshold"
              type="number"
              min={0}
              max={100}
              value={threshold}
              onChange={(e) => setThreshold(Number(e.target.value) || 0)}
            />
          </div>
          <Button type="submit" disabled={createAlert.isPending}>
            追加
          </Button>
        </form>
        {error && <p className="text-sm text-destructive">{error}</p>}

        {isLoading ? (
          <p className="text-sm text-muted-foreground">読み込み中...</p>
        ) : alerts && alerts.length > 0 ? (
          <div className="space-y-2">
            {alerts.map((a) => (
              <div
                key={a.id}
                className="flex items-center justify-between p-2 border rounded"
              >
                <div className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={a.enabled}
                    onChange={(e) =>
                      toggleAlert.mutate({ id: a.id, enabled: e.target.checked })
                    }
                  />
                  <span className={a.enabled ? "" : "text-muted-foreground line-through"}>
                    {metricLabel(a.metric)} が {a.threshold}% {comparisonLabel(a.comparison)}
                  </span>
                  {a.enabled && a.triggered && (
                    <Badge className="bg-red-500">発火中</Badge>
                  )}
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-red-500"
                  onClick={() => deleteAlert.mutate(a.id)}
                >
                  削除
                </Button>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground py-4 text-center">
            アラートルールがありません
          </p>
        )}
      </CardContent>
    </Card>
  );
}
