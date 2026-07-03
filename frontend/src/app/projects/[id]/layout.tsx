"use client";

import { useEffect } from "react";
import { useParams, useRouter, usePathname } from "next/navigation";
import Link from "next/link";
import { useAuth } from "@/hooks/use-auth";
import { useProject } from "@/hooks/use-projects";
import {
  LayoutDashboard,
  GitBranch,
  Terminal,
  Activity,
  Settings,
  Archive,
  ChevronLeft,
  Database,
  LogOut,
} from "lucide-react";

const statusDot: Record<string, string> = {
  running: "bg-emerald-400",
  stopped: "bg-amber-400",
  creating: "bg-blue-400 animate-pulse",
  resetting: "bg-purple-400 animate-pulse",
  error: "bg-red-400",
};

const navItems = [
  { label: "Overview",   href: "",            Icon: LayoutDashboard },
  { label: "Branches",   href: "/branches",   Icon: GitBranch },
  { label: "SQL Editor", href: "/editor",     Icon: Terminal },
  { label: "Monitoring", href: "/monitoring", Icon: Activity },
  { label: "Settings",   href: "/settings",   Icon: Settings },
  { label: "Backups",    href: "/backups",    Icon: Archive },
];

export default function ProjectLayout({ children }: { children: React.ReactNode }) {
  const { id } = useParams<{ id: string }>();
  const { isAuthenticated, loadUser, isLoading: authLoading, user, logout } = useAuth();
  const router = useRouter();
  const pathname = usePathname();
  const { data: project } = useProject(id);

  useEffect(() => {
    if (!isAuthenticated && !authLoading) {
      loadUser().catch(() => router.push("/login"));
    }
  }, [isAuthenticated, authLoading, loadUser, router]);

  if (authLoading || !isAuthenticated) return null;

  const base = `/projects/${id}`;

  return (
    <div className="flex min-h-screen bg-background">
      {/* Sidebar (dark) */}
      <aside className="w-60 shrink-0 flex flex-col fixed inset-y-0 left-0 bg-neutral-950 text-neutral-300">
        {/* Brand */}
        <div className="h-14 px-4 flex items-center gap-2.5 border-b border-white/10">
          <div className="size-7 rounded-md bg-primary flex items-center justify-center">
            <Database className="size-4 text-primary-foreground" />
          </div>
          <span className="font-semibold text-sm text-white tracking-tight">DB Console</span>
        </div>

        {/* Project header */}
        <div className="p-4 border-b border-white/10">
          <Link
            href="/dashboard"
            className="text-xs text-neutral-500 hover:text-neutral-200 transition-colors flex items-center gap-1 mb-3"
          >
            <ChevronLeft className="size-3" />
            ダッシュボード
          </Link>
          <div className="flex items-center gap-2">
            <span className={`size-2 rounded-full ${statusDot[project?.status ?? ""] ?? "bg-neutral-500"}`} />
            <p className="text-sm font-semibold text-white truncate">{project?.name ?? "…"}</p>
          </div>
          <p className="text-xs text-neutral-500 mt-1 capitalize font-mono">{project?.status ?? "—"}</p>
        </div>

        {/* Nav */}
        <nav className="flex-1 p-3 overflow-y-auto">
          <p className="px-3 mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-neutral-600">
            Manage
          </p>
          <div className="space-y-0.5">
            {navItems.map(({ label, href, Icon }) => {
              const to = `${base}${href}`;
              const active = href === "" ? pathname === base : pathname.startsWith(to);
              return (
                <Link
                  key={href}
                  href={to}
                  className={`relative flex items-center gap-3 px-3 py-2 rounded-md text-sm transition-colors ${
                    active
                      ? "bg-primary/15 text-white font-medium"
                      : "text-neutral-400 hover:text-white hover:bg-white/5"
                  }`}
                >
                  {active && (
                    <span className="absolute left-0 top-1/2 -translate-y-1/2 h-5 w-0.5 rounded-r bg-primary" />
                  )}
                  <Icon className={`size-4 ${active ? "text-primary" : ""}`} />
                  {label}
                </Link>
              );
            })}
          </div>
        </nav>

        {/* User */}
        <div className="p-3 border-t border-white/10">
          <div className="flex items-center gap-2.5 px-1">
            <div className="size-7 rounded-full bg-gradient-to-br from-primary to-emerald-700 flex items-center justify-center text-xs font-semibold text-white shrink-0">
              {user?.email?.[0]?.toUpperCase() ?? "?"}
            </div>
            <span className="text-xs text-neutral-400 truncate flex-1">{user?.email}</span>
            <button
              onClick={logout}
              className="text-neutral-500 hover:text-white transition-colors p-1 rounded"
              title="ログアウト"
            >
              <LogOut className="size-4" />
            </button>
          </div>
        </div>
      </aside>

      {/* Main */}
      <div className="flex-1 min-w-0 ml-60 overflow-auto">{children}</div>
    </div>
  );
}
