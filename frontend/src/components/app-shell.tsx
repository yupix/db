"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useAuth } from "@/hooks/use-auth";
import { Database, LayoutGrid, Users, LogOut } from "lucide-react";

const navItems = [
  { label: "Projects", href: "/dashboard", Icon: LayoutGrid, match: ["/dashboard", "/projects"] },
  { label: "組織 / チーム", href: "/organizations", Icon: Users, match: ["/organizations"] },
];

export function AppShell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const { user, logout } = useAuth();

  return (
    <div className="flex min-h-screen bg-background">
      {/* Sidebar (dark) */}
      <aside className="w-60 shrink-0 flex flex-col fixed inset-y-0 left-0 bg-neutral-950 text-neutral-300">
        <div className="h-14 px-4 flex items-center gap-2.5 border-b border-white/10">
          <div className="size-7 rounded-md bg-primary flex items-center justify-center">
            <Database className="size-4 text-primary-foreground" />
          </div>
          <span className="font-semibold text-sm text-white tracking-tight">DB Console</span>
        </div>

        <nav className="flex-1 p-3 overflow-y-auto">
          <p className="px-3 mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-neutral-600">
            Workspace
          </p>
          <div className="space-y-0.5">
            {navItems.map(({ label, href, Icon, match }) => {
              const active = match.some((m) => pathname.startsWith(m));
              return (
                <Link
                  key={href}
                  href={href}
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

      <div className="flex-1 min-w-0 ml-60">{children}</div>
    </div>
  );
}
