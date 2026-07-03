"use client";

import { useEffect, useState, useRef, useCallback } from "react";
import { useParams, useRouter } from "next/navigation";
import { useAuth } from "@/hooks/use-auth";
import { useProject } from "@/hooks/use-projects";
import Editor, { type OnMount } from "@monaco-editor/react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ProjectPageHeader } from "@/components/project-page-header";

interface QueryResult {
  success: boolean;
  error?: string;
  columns?: string[];
  rows?: (string | null)[][];
  rows_affected?: number;
  execution_time_ms?: number;
}

interface HistoryItem {
  query: string;
  timestamp: number;
  success: boolean;
}

const API_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080";
const HISTORY_KEY = "sql_query_history";
const MAX_HISTORY = 50;

export default function SqlEditorPage() {
  const { id } = useParams<{ id: string }>();
  const { isAuthenticated, loadUser, isLoading: authLoading } = useAuth();
  const router = useRouter();
  const { data: project } = useProject(id);
  const [query, setQuery] = useState("SELECT * FROM information_schema.tables LIMIT 10;");
  const [result, setResult] = useState<QueryResult | null>(null);
  const [isExecuting, setIsExecuting] = useState(false);
  const [history, setHistory] = useState<HistoryItem[]>(() => {
    if (typeof window === "undefined") return [];
    const saved = localStorage.getItem(`${HISTORY_KEY}_${id}`);
    if (saved) {
      try {
        return JSON.parse(saved);
      } catch {
        return [];
      }
    }
    return [];
  });
  const [activeTab, setActiveTab] = useState<"results" | "history">("results");
  const wsRef = useRef<WebSocket | null>(null);
  const editorRef = useRef<unknown>(null);

  useEffect(() => {
    if (!isAuthenticated && !authLoading) {
      loadUser().catch(() => router.push("/login"));
    }
  }, [isAuthenticated, authLoading, loadUser, router]);

  const saveToHistory = useCallback(
    (q: string, success: boolean) => {
      const item: HistoryItem = { query: q, timestamp: Date.now(), success };
      const updated = [item, ...history].slice(0, MAX_HISTORY);
      setHistory(updated);
      localStorage.setItem(`${HISTORY_KEY}_${id}`, JSON.stringify(updated));
    },
    [history, id]
  );

  const connectWs = useCallback(() => {
    return new Promise<WebSocket>((resolve, reject) => {
      const wsUrl = `${API_URL.replace("http", "ws")}/api/projects/${id}/query`;
      const ws = new WebSocket(wsUrl);

      ws.onopen = () => resolve(ws);
      ws.onerror = () => reject(new Error("WebSocket connection failed"));
      ws.onclose = () => {
        wsRef.current = null;
      };
    });
  }, [id]);

  const executeQuery = async (overrideQuery?: string) => {
    const sql = overrideQuery ?? query;
    if (!sql.trim() || isExecuting) return;
    setIsExecuting(true);
    setResult(null);

    try {
      let ws = wsRef.current;
      if (!ws || ws.readyState !== WebSocket.OPEN) {
        ws = await connectWs();
        wsRef.current = ws;
      }

      const responsePromise = new Promise<QueryResult>((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error("Query timeout (30s)"));
        }, 30000);

        const handler = (event: MessageEvent) => {
          clearTimeout(timeout);
          ws!.removeEventListener("message", handler);
          try {
            const data: QueryResult = JSON.parse(event.data);
            resolve(data);
          } catch {
            reject(new Error("Failed to parse response"));
          }
        };

        ws!.addEventListener("message", handler);
        ws!.send(JSON.stringify({ query: sql }));
      });

      const res = await responsePromise;
      setResult(res);
      saveToHistory(sql, res.success);

      if (!res.success) {
        setActiveTab("results");
      }
    } catch (e) {
      const errMsg = e instanceof Error ? e.message : "Execution failed";
      setResult({ success: false, error: errMsg });
      saveToHistory(query, false);
      setActiveTab("results");
    } finally {
      setIsExecuting(false);
    }
  };

  const executeQueryRef = useRef(executeQuery);
  useEffect(() => {
    executeQueryRef.current = executeQuery;
  });

  const handleEditorMount: OnMount = (editor, monaco) => {
    editorRef.current = editor;
    editor.addCommand(
      monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter,
      () => executeQueryRef.current()
    );
  };

  const runExplain = () => {
    const explainSql = `EXPLAIN ANALYZE ${query.replace(/^EXPLAIN\s+(ANALYZE\s+)?/i, "")}`;
    setQuery(explainSql);
    executeQuery(explainSql);
  };

  const runFromHistory = (q: string) => {
    setQuery(q);
    setActiveTab("results");
  };

  const clearHistory = () => {
    setHistory([]);
    localStorage.removeItem(`${HISTORY_KEY}_${id}`);
  };

  useEffect(() => {
    return () => {
      if (wsRef.current) {
        wsRef.current.close();
      }
    };
  }, []);

  if (authLoading || !isAuthenticated) return null;

  return (
    <div>
      <ProjectPageHeader
        title="SQL エディタ"
        description={project ? `${project.name} · ${project.status}` : undefined}
        actions={
          <>
            <Button variant="outline" size="sm" onClick={runExplain} disabled={isExecuting}>
              EXPLAIN ANALYZE
            </Button>
            <Button size="sm" onClick={() => executeQuery()} disabled={isExecuting || !query.trim()}>
              {isExecuting ? "実行中..." : "実行 (Ctrl+Enter)"}
            </Button>
          </>
        }
      />

      <main className="p-6 space-y-4">
        {/* Editor */}
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm">クエリ</CardTitle>
          </CardHeader>
          <CardContent className="p-0">
            <div>
              <Editor
                height="200px"
                defaultLanguage="sql"
                value={query}
                onChange={(value) => setQuery(value || "")}
                onMount={handleEditorMount}
                options={{
                  minimap: { enabled: false },
                  fontSize: 14,
                  wordWrap: "on",
                  scrollBeyondLastLine: false,
                  automaticLayout: true,
                  tabSize: 2,
                }}
              />
            </div>
          </CardContent>
        </Card>

        {/* Results / History tabs */}
        <Card>
          <CardHeader className="pb-2">
            <div className="flex items-center justify-between">
              <div className="flex gap-4">
                <button
                  className={`text-sm font-medium ${activeTab === "results" ? "text-foreground" : "text-muted-foreground"}`}
                  onClick={() => setActiveTab("results")}
                >
                  結果
                </button>
                <button
                  className={`text-sm font-medium ${activeTab === "history" ? "text-foreground" : "text-muted-foreground"}`}
                  onClick={() => setActiveTab("history")}
                >
                  履歴 ({history.length})
                </button>
              </div>
              {result?.execution_time_ms != null && (
                <span className="text-xs text-muted-foreground">
                  {result.execution_time_ms} ms
                </span>
              )}
            </div>
          </CardHeader>
          <CardContent>
            {activeTab === "results" && (
              <ResultView result={result} isExecuting={isExecuting} />
            )}
            {activeTab === "history" && (
              <HistoryView
                history={history}
                onRun={runFromHistory}
                onClear={clearHistory}
              />
            )}
          </CardContent>
        </Card>
      </main>
    </div>
  );
}

function ResultView({
  result,
  isExecuting,
}: {
  result: QueryResult | null;
  isExecuting: boolean;
}) {
  if (isExecuting) {
    return <div className="py-8 text-center text-muted-foreground">実行中...</div>;
  }

  if (!result) {
    return (
      <div className="py-8 text-center text-muted-foreground">
        クエリを実行すると結果がここに表示されます
      </div>
    );
  }

  if (!result.success) {
    return (
      <div className="py-4">
        <div className="text-sm text-red-500 bg-red-50 p-3 rounded font-mono whitespace-pre-wrap">
          {result.error}
        </div>
      </div>
    );
  }

  if (!result.columns || result.columns.length === 0) {
    return (
      <div className="py-8 text-center text-muted-foreground">
        {result.rows_affected != null
          ? `${result.rows_affected} 行処理しました`
          : "クエリが正常に実行されました"}
      </div>
    );
  }

  return (
    <div className="overflow-auto max-h-[400px]">
      <table className="w-full text-sm">
        <thead className="sticky top-0 bg-muted">
          <tr>
            {result.columns.map((col, i) => (
              <th key={i} className="text-left p-2 font-medium border-b">
                {col}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {result.rows && result.rows.length > 0 ? (
            result.rows.map((row, i) => (
              <tr key={i} className="hover:bg-muted/50">
                {row.map((cell, j) => (
                  <td key={j} className="p-2 border-b truncate max-w-[300px]">
                    {cell === null ? (
                      <span className="text-muted-foreground italic">NULL</span>
                    ) : (
                      cell
                    )}
                  </td>
                ))}
              </tr>
            ))
          ) : (
            <tr>
              <td colSpan={result.columns.length} className="p-4 text-center text-muted-foreground">
                行がありません
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}

function HistoryView({
  history,
  onRun,
  onClear,
}: {
  history: HistoryItem[];
  onRun: (query: string) => void;
  onClear: () => void;
}) {
  if (history.length === 0) {
    return (
      <div className="py-8 text-center text-muted-foreground">
        クエリ履歴がありません
      </div>
    );
  }

  return (
    <div className="space-y-1">
      <div className="flex justify-end mb-2">
        <Button variant="ghost" size="sm" className="text-red-500" onClick={onClear}>
          履歴をクリア
        </Button>
      </div>
      {history.map((item, i) => (
        <div
          key={i}
          className="flex items-center justify-between p-2 border rounded hover:bg-muted/50 cursor-pointer"
          onClick={() => onRun(item.query)}
        >
          <div className="flex items-center gap-2 min-w-0">
            <Badge className={item.success ? "bg-green-500" : "bg-red-500"}>
              {item.success ? "OK" : "ERR"}
            </Badge>
            <code className="text-xs truncate max-w-[400px]">{item.query}</code>
          </div>
          <span className="text-xs text-muted-foreground shrink-0">
            {new Date(item.timestamp).toLocaleTimeString()}
          </span>
        </div>
      ))}
    </div>
  );
}
