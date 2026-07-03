"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "@/hooks/use-auth";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Database, GitBranch, Gauge, ShieldCheck, ArrowRight } from "lucide-react";
import Link from "next/link";

const features = [
  { Icon: GitBranch, title: "ブランチング", desc: "本番データを一瞬でコピーして検証環境を作成" },
  { Icon: Gauge, title: "モニタリング", desc: "CPU・メモリ・クエリ統計をリアルタイム可視化" },
  { Icon: ShieldCheck, title: "自動バックアップ", desc: "スケジュール実行とワンクリック復元" },
];

export default function LoginPage() {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const { login, isLoading, error, user } = useAuth();
  const router = useRouter();

  useEffect(() => {
    if (user) router.push("/dashboard");
  }, [user, router]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      await login(email, password);
      router.push("/dashboard");
    } catch {}
  };

  return (
    <div className="min-h-screen grid lg:grid-cols-2 bg-background">
      {/* --- Left: brand panel --- */}
      <div className="relative hidden lg:flex flex-col justify-between overflow-hidden bg-gradient-to-br from-primary via-primary to-emerald-700 p-12 text-primary-foreground">
        {/* grid texture */}
        <div
          className="absolute inset-0 opacity-[0.12]"
          style={{
            backgroundImage:
              "linear-gradient(to right, #fff 1px, transparent 1px), linear-gradient(to bottom, #fff 1px, transparent 1px)",
            backgroundSize: "40px 40px",
          }}
        />
        {/* glow */}
        <div className="absolute -top-24 -right-24 size-96 rounded-full bg-white/10 blur-3xl" />
        <div className="absolute -bottom-32 -left-16 size-96 rounded-full bg-emerald-300/20 blur-3xl" />

        {/* logo */}
        <div className="relative flex items-center gap-2.5">
          <div className="size-9 rounded-lg bg-white/15 backdrop-blur flex items-center justify-center ring-1 ring-white/25">
            <Database className="size-5" />
          </div>
          <span className="font-semibold text-lg tracking-tight">DB Console</span>
        </div>

        {/* pitch */}
        <div className="relative space-y-8 max-w-md">
          <div className="space-y-3">
            <h2 className="text-3xl font-bold leading-tight tracking-tight">
              PostgreSQL を、
              <br />
              コンソールから思いのままに。
            </h2>
            <p className="text-primary-foreground/70 leading-relaxed">
              インスタンスの作成・ブランチ・監視・バックアップまで。
              開発に必要なデータベース運用をひとつの画面で。
            </p>
          </div>

          <div className="space-y-4">
            {features.map(({ Icon, title, desc }) => (
              <div key={title} className="flex gap-3">
                <div className="mt-0.5 size-9 shrink-0 rounded-lg bg-white/10 ring-1 ring-white/15 flex items-center justify-center">
                  <Icon className="size-4" />
                </div>
                <div>
                  <p className="font-medium text-sm">{title}</p>
                  <p className="text-sm text-primary-foreground/60">{desc}</p>
                </div>
              </div>
            ))}
          </div>
        </div>

        <div className="relative text-xs text-primary-foreground/50">
          © {new Date().getFullYear()} DB Console · Self-hosted PostgreSQL platform
        </div>
      </div>

      {/* --- Right: form --- */}
      <div className="flex items-center justify-center p-6 sm:p-12">
        <div className="w-full max-w-sm space-y-8">
          {/* mobile logo */}
          <div className="flex items-center gap-2.5 lg:hidden">
            <div className="size-9 rounded-lg bg-primary flex items-center justify-center">
              <Database className="size-5 text-primary-foreground" />
            </div>
            <span className="font-semibold text-lg">DB Console</span>
          </div>

          <div className="space-y-1.5">
            <h1 className="text-2xl font-bold tracking-tight">ログイン</h1>
            <p className="text-sm text-muted-foreground">
              アカウント情報を入力してコンソールへ
            </p>
          </div>

          <form onSubmit={handleSubmit} className="space-y-5">
            {error && (
              <div className="text-sm text-destructive bg-destructive/10 border border-destructive/20 px-3 py-2.5 rounded-lg">
                {error}
              </div>
            )}
            <div className="space-y-2">
              <Label htmlFor="email">メールアドレス</Label>
              <Input
                id="email"
                type="email"
                placeholder="you@example.com"
                value={email}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => setEmail(e.target.value)}
                required
              />
            </div>
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <Label htmlFor="password">パスワード</Label>
              </div>
              <Input
                id="password"
                type="password"
                placeholder="••••••••"
                value={password}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => setPassword(e.target.value)}
                required
              />
            </div>
            <Button type="submit" className="w-full group" disabled={isLoading}>
              {isLoading ? "ログイン中..." : "ログイン"}
              {!isLoading && (
                <ArrowRight className="size-4 transition-transform group-hover:translate-x-0.5" />
              )}
            </Button>
          </form>

          <p className="text-sm text-center text-muted-foreground">
            アカウントがない方は{" "}
            <Link href="/register" className="text-primary font-medium hover:underline">
              新規登録
            </Link>
          </p>
        </div>
      </div>
    </div>
  );
}
