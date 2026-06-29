"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "@/hooks/use-auth";

export default function Home() {
  const { isAuthenticated, isLoading, loadUser } = useAuth();
  const router = useRouter();

  useEffect(() => {
    if (isLoading) return;
    if (isAuthenticated) {
      router.push("/dashboard");
    } else {
      loadUser()
        .then(() => router.push("/dashboard"))
        .catch(() => router.push("/login"));
    }
  }, [isAuthenticated, isLoading, loadUser, router]);

  return null;
}
