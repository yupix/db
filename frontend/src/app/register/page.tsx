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

export default function RegisterPage() {
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const { register, isLoading, error, user } = useAuth();
  const router = useRouter();

  useEffect(() => {
    if (user) router.push("/dashboard");
  }, [user, router]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      await register(email, password, name);
      router.push("/dashboard");
    } catch {}
  };

  return (
    <div className="min-h-screen grid lg:grid-cols-2 bg-background">
      {/* --- Left: brand panel --- */}
      <div className="relative hidden lg:flex flex-col justify-between overflow-hidden bg-gradient-to-br from-primary via-primary to-emerald-700 p-12 text-primary-foreground">
        <div
          className="absolute inset-0 opacity-[0.12]"
          style={{
            backgroundImage:
              "linear-gradient(to right, #fff 1px, transparent 1px), linear-gradient(to bottom, #fff 1px, transparent 1px)",
            backgroundSize: "40px 40px",
          }}
        />
        <div className="absolute -top-24 -right-24 size-96 rounded-full bg-white/10 blur-3xl" />
        <div className="absolute -bottom-32 -left-16 size-96 rounded-full bg-emerald-300/20 blur-3xl" />

        <div className="relative flex items-center gap-2.5">
          <div className="size-9 rounded-lg bg-white/15 backdrop-blur flex items-center justify-center ring-1 ring-white/25">
            <Database className="size-5" />
          </div>
          <span className="font-semibold text-lg tracking-tight">DB Console</span>
        </div>

        <div className="relative space-y-8 max-w-md">
          <div className="space-y-3">
            <h2 className="text-3xl font-bold leading-tight tracking-tight">
              数分で、あなたの
              <br />
              データベース基盤を。
            </h2>
            <p className="text-primary-foreground/70 leading-relaxed">
              アカウントを作成して、PostgreSQL の作成・運用をすぐに始められます。
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
          <div className="flex items-center gap-2.5 lg:hidden">
            <div className="size-9 rounded-lg bg-primary flex items-center justify-center">
              <Database className="size-5 text-primary-foreground" />
            </div>
            <span className="font-semibold text-lg">DB Console</span>
          </div>

          <div className="space-y-1.5">
            <h1 className="text-2xl font-bold tracking-tight">アカウント作成</h1>
            <p className="text-sm text-muted-foreground">必要な情報を入力してはじめましょう</p>
          </div>

          <form onSubmit={handleSubmit} className="space-y-5">
            {error && (
              <div className="text-sm text-destructive bg-destructive/10 border border-destructive/20 px-3 py-2.5 rounded-lg">
                {error}
              </div>
            )}
            <div className="space-y-2">
              <Label htmlFor="name">名前</Label>
              <Input
                id="name"
                placeholder="山田 太郎"
                value={name}
                onChange={(e) => setName(e.target.value)}
                required
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="email">メールアドレス</Label>
              <Input
                id="email"
                type="email"
                placeholder="you@example.com"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                required
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="password">パスワード</Label>
              <Input
                id="password"
                type="password"
                placeholder="8文字以上"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                minLength={8}
                required
              />
            </div>
            <Button type="submit" className="w-full group" disabled={isLoading}>
              {isLoading ? "登録中..." : "登録する"}
              {!isLoading && (
                <ArrowRight className="size-4 transition-transform group-hover:translate-x-0.5" />
              )}
            </Button>
          </form>

          <p className="text-sm text-center text-muted-foreground">
            すでにアカウントをお持ちの方は{" "}
            <Link href="/login" className="text-primary font-medium hover:underline">
              ログイン
            </Link>
          </p>
        </div>
      </div>
    </div>
  );
}
