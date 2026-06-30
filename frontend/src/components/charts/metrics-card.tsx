"use client";

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from "recharts";
import { metricsApi, type MetricsRange, type MetricPoint } from "@/lib/api";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";

const RANGES: { value: MetricsRange; label: string }[] = [
  { value: "1h", label: "1時間" },
  { value: "6h", label: "6時間" },
  { value: "24h", label: "24時間" },
  { value: "7d", label: "7日" },
  { value: "30d", label: "30日" },
];

function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}

function formatTime(iso: string, range: MetricsRange): string {
  const d = new Date(iso);
  if (range === "7d" || range === "30d") {
    return d.toLocaleDateString("ja-JP", { month: "numeric", day: "numeric", hour: "numeric" });
  }
  return d.toLocaleTimeString("ja-JP", { hour: "2-digit", minute: "2-digit" });
}

interface ChartRow {
  label: string;
  cpu: number;
  memUsedMb: number;
  memLimitMb: number;
  netRxMb: number;
  netTxMb: number;
}

export function MetricsCard({ projectId, running }: { projectId: string; running: boolean }) {
  const [range, setRange] = useState<MetricsRange>("1h");

  const { data, isLoading, error } = useQuery({
    queryKey: ["metrics", projectId, range],
    queryFn: () => metricsApi.get(projectId, range),
    // Live-refresh short ranges; long ranges change slowly so poll less often.
    refetchInterval: range === "1h" || range === "6h" ? 30_000 : 300_000,
    enabled: running,
  });

  const rows: ChartRow[] = (data?.points ?? []).map((p: MetricPoint) => ({
    label: formatTime(p.ts, range),
    cpu: Number(p.cpu_pct.toFixed(2)),
    memUsedMb: Number((p.mem_used_bytes / 1024 / 1024).toFixed(1)),
    memLimitMb: Number((p.mem_limit_bytes / 1024 / 1024).toFixed(1)),
    netRxMb: Number((p.net_rx_bytes / 1024 / 1024).toFixed(2)),
    netTxMb: Number((p.net_tx_bytes / 1024 / 1024).toFixed(2)),
  }));

  const latest = data?.points.at(-1);

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between">
          <div>
            <CardTitle>メトリクス</CardTitle>
            <CardDescription>
              コンテナのリソース使用状況（30秒ごとに収集）
            </CardDescription>
          </div>
          <div className="flex gap-1">
            {RANGES.map((r) => (
              <Button
                key={r.value}
                variant={range === r.value ? "default" : "outline"}
                size="sm"
                onClick={() => setRange(r.value)}
              >
                {r.label}
              </Button>
            ))}
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-6">
        {!running ? (
          <p className="text-sm text-muted-foreground py-8 text-center">
            プロジェクトが起動中のみメトリクスを収集します
          </p>
        ) : isLoading ? (
          <p className="text-sm text-muted-foreground py-8 text-center">読み込み中...</p>
        ) : error ? (
          <p className="text-sm text-destructive py-8 text-center">
            メトリクスの取得に失敗しました
          </p>
        ) : rows.length === 0 ? (
          <p className="text-sm text-muted-foreground py-8 text-center">
            まだデータがありません（収集まで最大30秒お待ちください）
          </p>
        ) : (
          <>
            {/* Current values */}
            {latest && (
              <div className="grid grid-cols-3 gap-4 text-sm">
                <div>
                  <p className="text-xs text-muted-foreground">CPU</p>
                  <p className="text-lg font-semibold">{latest.cpu_pct.toFixed(1)}%</p>
                </div>
                <div>
                  <p className="text-xs text-muted-foreground">メモリ</p>
                  <p className="text-lg font-semibold">
                    {formatBytes(latest.mem_used_bytes)}
                    <span className="text-xs text-muted-foreground font-normal">
                      {" "}/ {formatBytes(latest.mem_limit_bytes)}
                    </span>
                  </p>
                </div>
                <div>
                  <p className="text-xs text-muted-foreground">ネットワーク (累計)</p>
                  <p className="text-lg font-semibold">
                    ↓{formatBytes(latest.net_rx_bytes)} ↑{formatBytes(latest.net_tx_bytes)}
                  </p>
                </div>
              </div>
            )}

            {/* CPU chart */}
            <div>
              <p className="text-xs font-medium mb-2">CPU 使用率 (%)</p>
              <ResponsiveContainer width="100%" height={160}>
                <LineChart data={rows} margin={{ top: 4, right: 8, bottom: 0, left: -16 }}>
                  <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
                  <XAxis dataKey="label" tick={{ fontSize: 11 }} minTickGap={24} />
                  <YAxis tick={{ fontSize: 11 }} domain={[0, "auto"]} />
                  <Tooltip />
                  <Line
                    type="monotone"
                    dataKey="cpu"
                    name="CPU %"
                    stroke="#22c55e"
                    dot={false}
                    strokeWidth={2}
                    isAnimationActive={false}
                  />
                </LineChart>
              </ResponsiveContainer>
            </div>

            {/* Memory chart */}
            <div>
              <p className="text-xs font-medium mb-2">メモリ (MB)</p>
              <ResponsiveContainer width="100%" height={160}>
                <LineChart data={rows} margin={{ top: 4, right: 8, bottom: 0, left: -16 }}>
                  <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
                  <XAxis dataKey="label" tick={{ fontSize: 11 }} minTickGap={24} />
                  <YAxis tick={{ fontSize: 11 }} domain={[0, "auto"]} />
                  <Tooltip />
                  <Line
                    type="monotone"
                    dataKey="memUsedMb"
                    name="使用"
                    stroke="#3b82f6"
                    dot={false}
                    strokeWidth={2}
                    isAnimationActive={false}
                  />
                  <Line
                    type="monotone"
                    dataKey="memLimitMb"
                    name="上限"
                    stroke="#94a3b8"
                    strokeDasharray="4 4"
                    dot={false}
                    strokeWidth={1}
                    isAnimationActive={false}
                  />
                </LineChart>
              </ResponsiveContainer>
            </div>

            {/* Network chart */}
            <div>
              <p className="text-xs font-medium mb-2">ネットワーク累計 (MB)</p>
              <ResponsiveContainer width="100%" height={160}>
                <LineChart data={rows} margin={{ top: 4, right: 8, bottom: 0, left: -16 }}>
                  <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
                  <XAxis dataKey="label" tick={{ fontSize: 11 }} minTickGap={24} />
                  <YAxis tick={{ fontSize: 11 }} domain={[0, "auto"]} />
                  <Tooltip />
                  <Line
                    type="monotone"
                    dataKey="netRxMb"
                    name="受信"
                    stroke="#a855f7"
                    dot={false}
                    strokeWidth={2}
                    isAnimationActive={false}
                  />
                  <Line
                    type="monotone"
                    dataKey="netTxMb"
                    name="送信"
                    stroke="#f97316"
                    dot={false}
                    strokeWidth={2}
                    isAnimationActive={false}
                  />
                </LineChart>
              </ResponsiveContainer>
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}
