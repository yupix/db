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
  running: "bg-emerald-500",
  stopped: "bg-amber-500",
  creating: "bg-blue-500 animate-pulse",
  resetting: "bg-purple-500 animate-pulse",
  error: "bg-red-500",
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
      {/* Sidebar */}
      <aside className="w-60 shrink-0 border-r bg-sidebar flex flex-col fixed inset-y-0 left-0">
        {/* Brand */}
        <div className="h-14 px-4 flex items-center gap-2 border-b">
          <div className="size-7 rounded-md bg-primary flex items-center justify-center">
            <Database className="size-4 text-primary-foreground" />
          </div>
          <span className="font-semibold text-sm">DB Console</span>
        </div>

        {/* Project header */}
        <div className="p-4 border-b">
          <Link
            href="/dashboard"
            className="text-xs text-muted-foreground hover:text-foreground flex items-center gap-1 mb-2"
          >
            <ChevronLeft className="size-3" />
            ダッシュボード
          </Link>
          <div className="flex items-center gap-2">
            <span className={`size-2 rounded-full ${statusDot[project?.status ?? ""] ?? "bg-gray-400"}`} />
            <p className="text-sm font-semibold truncate">{project?.name ?? "…"}</p>
          </div>
          <p className="text-xs text-muted-foreground mt-0.5 capitalize">{project?.status}</p>
        </div>

        {/* Nav */}
        <nav className="flex-1 p-2 space-y-0.5 overflow-y-auto">
          {navItems.map(({ label, href, Icon }) => {
            const to = `${base}${href}`;
            const active = href === "" ? pathname === base : pathname.startsWith(to);
            return (
              <Link
                key={href}
                href={to}
                className={`flex items-center gap-3 px-3 py-2 rounded-md text-sm transition-colors ${
                  active
                    ? "bg-sidebar-accent text-sidebar-accent-foreground font-medium"
                    : "text-muted-foreground hover:text-foreground hover:bg-sidebar-accent/50"
                }`}
              >
                <Icon className="size-4" />
                {label}
              </Link>
            );
          })}
        </nav>

        {/* User */}
        <div className="p-3 border-t">
          <div className="flex items-center gap-2 px-1">
            <div className="size-7 rounded-full bg-accent flex items-center justify-center text-xs font-semibold text-accent-foreground shrink-0">
              {user?.email?.[0]?.toUpperCase() ?? "?"}
            </div>
            <span className="text-xs text-muted-foreground truncate flex-1">{user?.email}</span>
            <button
              onClick={logout}
              className="text-muted-foreground hover:text-foreground p-1 rounded"
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
