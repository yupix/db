"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "@/hooks/use-auth";

export default function Home() {
  const { token } = useAuth();
  const router = useRouter();

  useEffect(() => {
    router.push(token ? "/dashboard" : "/login");
  }, [token, router]);

  return null;
}
