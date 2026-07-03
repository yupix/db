"use client";

import { useEffect } from "react";
import { useParams, useRouter, usePathname } from "next/navigation";
import Link from "next/link";
import { useAuth } from "@/hooks/use-auth";
import { useProject } from "@/hooks/use-projects";
import { Badge } from "@/components/ui/badge";

const statusColors: Record<string, string> = {
  running: "bg-green-500",
  stopped: "bg-yellow-500",
  creating: "bg-blue-500",
  resetting: "bg-purple-500",
  error: "bg-red-500",
};

const navItems = [
  { label: "Overview",    href: "",            icon: "⊙" },
  { label: "Branches",    href: "/branches",   icon: "⎇" },
  { label: "SQL Editor",  href: "/editor",     icon: "▶" },
  { label: "Monitoring",  href: "/monitoring", icon: "◈" },
  { label: "Settings",    href: "/settings",   icon: "⚙" },
  { label: "Backups",     href: "/backups",    icon: "⊘" },
];

export default function ProjectLayout({ children }: { children: React.ReactNode }) {
  const { id } = useParams<{ id: string }>();
  const { isAuthenticated, loadUser, isLoading: authLoading } = useAuth();
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
      <aside className="w-56 shrink-0 border-r flex flex-col">
        {/* Top: back + project name */}
        <div className="p-4 border-b space-y-2">
          <Link
            href="/dashboard"
            className="text-xs text-muted-foreground hover:text-foreground flex items-center gap-1"
          >
            ← ダッシュボード
          </Link>
          <div className="space-y-1">
            <p className="text-sm font-semibold truncate">{project?.name ?? "…"}</p>
            {project && (
              <Badge className={`text-xs ${statusColors[project.status] ?? "bg-gray-500"}`}>
                {project.status}
              </Badge>
            )}
          </div>
        </div>

        {/* Nav */}
        <nav className="flex-1 p-2 space-y-0.5">
          {navItems.map(({ label, href, icon }) => {
            const to = `${base}${href}`;
            const active = href === ""
              ? pathname === base
              : pathname.startsWith(to);
            return (
              <Link
                key={href}
                href={to}
                className={`flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors ${
                  active
                    ? "bg-accent text-accent-foreground font-medium"
                    : "text-muted-foreground hover:text-foreground hover:bg-accent/50"
                }`}
              >
                <span className="text-base leading-none">{icon}</span>
                {label}
              </Link>
            );
          })}
        </nav>
      </aside>

      {/* Main */}
      <div className="flex-1 min-w-0 overflow-auto">
        {children}
      </div>
    </div>
  );
}
