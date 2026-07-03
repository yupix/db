"use client";

import { useState, useEffect } from "react";
import { useParams } from "next/navigation";
import { useProject } from "@/hooks/use-projects";
import { projectsApi, type Branch } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { ProjectPageHeader } from "@/components/project-page-header";

const statusColors: Record<string, string> = {
  running: "bg-green-500",
  stopped: "bg-yellow-500",
  creating: "bg-blue-500",
  error: "bg-red-500",
};

export default function BranchesPage() {
  const { id } = useParams<{ id: string }>();
  const { data: project } = useProject(id);
  const [branches, setBranches] = useState<Branch[]>([]);
  const [loading, setLoading] = useState(false);
  const [form, setForm] = useState({ name: "", parent_branch_id: "" });
  const [connModal, setConnModal] = useState<{ label: string; value: string } | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (id) projectsApi.listBranches(id).then(setBranches).catch(() => {});
  }, [id]);

  const handleCreate = async () => {
    if (!id || !form.name) return;
    setLoading(true);
    try {
      const branch = await projectsApi.createBranch(id, {
        name: form.name,
        parent_branch_id: form.parent_branch_id || undefined,
      });
      setBranches((prev) => [...prev, branch]);
      setForm({ name: "", parent_branch_id: "" });
    } catch (e) {
      alert(e instanceof Error ? e.message : "ブランチの作成に失敗しました");
    } finally {
      setLoading(false);
    }
  };

  const handleDelete = async (branchId: string) => {
    if (!id || !confirm("このブランチを削除しますか？")) return;
    try {
      await projectsApi.deleteBranch(id, branchId);
      setBranches((prev) =>
        prev
          .filter((b) => b.id !== branchId)
          .map((b) => b.parent_branch_id === branchId ? { ...b, parent_branch_id: null } : b)
      );
    } catch (e) {
      alert(e instanceof Error ? e.message : "削除に失敗しました");
    }
  };

  const handleReset = async (branchId: string) => {
    if (!id || !confirm("このブランチをリセットしますか？データが上書きされます。")) return;
    try {
      const updated = await projectsApi.resetBranch(id, branchId);
      setBranches((prev) => prev.map((b) => (b.id === branchId ? updated : b)));
    } catch (e) {
      alert(e instanceof Error ? e.message : "リセットに失敗しました");
    }
  };

  const copyToClipboard = (text: string) => {
    if (navigator.clipboard) {
      navigator.clipboard.writeText(text).catch(() => fallback(text));
    } else {
      fallback(text);
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const fallback = (text: string) => {
    const el = document.createElement("textarea");
    el.value = text;
    el.style.cssText = "position:fixed;opacity:0";
    document.body.appendChild(el);
    el.select();
    document.execCommand("copy");
    document.body.removeChild(el);
  };

  // Build tree
  const branchIds = new Set(branches.map((b) => b.id));
  const roots = branches.filter((b) => !b.parent_branch_id || !branchIds.has(b.parent_branch_id));
  const childrenOf = (pid: string) => branches.filter((b) => b.parent_branch_id === pid);

  const renderBranch = (branch: Branch, depth = 0): React.ReactNode => (
    <div key={branch.id} className="space-y-1">
      <div
        className="flex items-center justify-between p-3 border rounded-lg"
        style={{ marginLeft: `${depth * 24}px` }}
      >
        <div className="flex items-center gap-2 min-w-0">
          {depth > 0 && <span className="text-muted-foreground text-sm select-none">└─</span>}
          <Badge className={statusColors[branch.status] ?? "bg-gray-500"}>{branch.status}</Badge>
          <span className="font-medium truncate">{branch.name}</span>
          <span className="text-xs text-muted-foreground font-mono">:{branch.port}</span>
        </div>
        <div className="flex gap-1 shrink-0">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setConnModal({ label: `ブランチ: ${branch.name}`, value: branch.connection_string })}
          >
            接続
          </Button>
          {branch.status === "running" && (
            <Button variant="ghost" size="sm" onClick={() => handleReset(branch.id)}>
              リセット
            </Button>
          )}
          <Button variant="ghost" size="sm" className="text-red-500" onClick={() => handleDelete(branch.id)}>
            削除
          </Button>
        </div>
      </div>
      {childrenOf(branch.id).map((c) => renderBranch(c, depth + 1))}
    </div>
  );

  return (
    <div>
      <Dialog open={!!connModal} onOpenChange={(o) => !o && setConnModal(null)}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>{connModal?.label}</DialogTitle>
          </DialogHeader>
          <div className="space-y-3">
            <code className="block w-full p-3 bg-muted rounded-lg text-sm break-all select-all font-mono">
              {connModal?.value}
            </code>
            <Button className="w-full" onClick={() => connModal && copyToClipboard(connModal.value)}>
              {copied ? "コピー済み!" : "クリップボードにコピー"}
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      <ProjectPageHeader title="Branches" description="データベースのスナップショットコピーを管理します" />

      <div className="p-6 space-y-6">
      {/* Branch tree */}
      <Card>
        <CardHeader>
          <CardTitle>ブランチ一覧</CardTitle>
          <CardDescription>データベースのコピー（ブランチ）のツリー構造</CardDescription>
        </CardHeader>
        <CardContent className="space-y-1">
          {/* project root */}
          <div className="flex items-center gap-2 p-3 border rounded-lg bg-muted/50">
            <Badge className="bg-blue-500">main</Badge>
            <span className="font-medium">{project?.name ?? "プロジェクト本体"}</span>
          </div>
          {branches.length === 0 ? (
            <p className="text-sm text-muted-foreground py-4 text-center">ブランチがありません</p>
          ) : (
            roots.map((b) => renderBranch(b, 0))
          )}
        </CardContent>
      </Card>

      {/* Create form */}
      <Card>
        <CardHeader>
          <CardTitle>ブランチを作成</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-1">
            <Label htmlFor="branch_name">ブランチ名</Label>
            <Input
              id="branch_name"
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              placeholder="feature-branch"
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor="parent">親ブランチ（省略でプロジェクト本体から作成）</Label>
            <select
              id="parent"
              className="w-full border rounded-md px-3 py-2 text-sm bg-background"
              value={form.parent_branch_id}
              onChange={(e) => setForm({ ...form, parent_branch_id: e.target.value })}
            >
              <option value="">プロジェクト本体</option>
              {branches.map((b) => (
                <option key={b.id} value={b.id}>{b.name}</option>
              ))}
            </select>
          </div>
          <Button onClick={handleCreate} disabled={loading || !form.name}>
            {loading ? "作成中..." : "作成"}
          </Button>
        </CardContent>
      </Card>
      </div>
    </div>
  );
}
