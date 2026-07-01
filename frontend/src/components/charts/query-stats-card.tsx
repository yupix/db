"use client";

import { useQuery } from "@tanstack/react-query";
import { metricsApi } from "@/lib/api";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

function formatMs(ms: number): string {
  if (ms >= 1000) return `${(ms / 1000).toFixed(2)} s`;
  return `${ms.toFixed(1)} ms`;
}

export function QueryStatsCard({ projectId, running }: { projectId: string; running: boolean }) {
  const { data, isLoading } = useQuery({
    queryKey: ["query-stats", projectId],
    queryFn: () => metricsApi.queryStats(projectId),
    refetchInterval: 60_000,
    enabled: running,
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle>クエリ統計</CardTitle>
        <CardDescription>
          pg_stat_statements による総実行時間トップ20（実行時間の長い順）
        </CardDescription>
      </CardHeader>
      <CardContent>
        {!running ? (
          <p className="text-sm text-muted-foreground py-6 text-center">
            プロジェクトが起動中のみ取得できます
          </p>
        ) : isLoading ? (
          <p className="text-sm text-muted-foreground py-6 text-center">読み込み中...</p>
        ) : !data?.available ? (
          <p className="text-sm text-muted-foreground py-6 text-center">
            このプロジェクトでは pg_stat_statements が利用できません
            <br />
            <span className="text-xs">
              （Phase 7.1 より前に作成されたプロジェクトは、作り直すと有効になります）
            </span>
          </p>
        ) : data.stats.length === 0 ? (
          <p className="text-sm text-muted-foreground py-6 text-center">
            まだ記録されたクエリがありません
          </p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b text-left text-xs text-muted-foreground">
                  <th className="py-2 pr-2 font-medium">クエリ</th>
                  <th className="py-2 px-2 font-medium text-right">実行回数</th>
                  <th className="py-2 px-2 font-medium text-right">合計時間</th>
                  <th className="py-2 px-2 font-medium text-right">平均</th>
                  <th className="py-2 pl-2 font-medium text-right">行数</th>
                </tr>
              </thead>
              <tbody>
                {data.stats.map((s, i) => (
                  <tr key={i} className="border-b last:border-0 align-top">
                    <td className="py-2 pr-2">
                      <code className="text-xs break-all line-clamp-2 block max-w-[360px]">
                        {s.query}
                      </code>
                    </td>
                    <td className="py-2 px-2 text-right tabular-nums">
                      {s.calls.toLocaleString()}
                    </td>
                    <td className="py-2 px-2 text-right tabular-nums">
                      {formatMs(s.total_exec_time_ms)}
                    </td>
                    <td className="py-2 px-2 text-right tabular-nums">
                      {formatMs(s.mean_exec_time_ms)}
                    </td>
                    <td className="py-2 pl-2 text-right tabular-nums">
                      {s.rows.toLocaleString()}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
