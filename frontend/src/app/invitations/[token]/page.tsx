"use client";

import { useEffect, useRef, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import { organizationsApi, ApiError } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import Link from "next/link";

export default function AcceptInvitationPage() {
  const { token } = useParams<{ token: string }>();
  const router = useRouter();
  const [status, setStatus] = useState<"idle" | "loading" | "success" | "error">("idle");
  const [errorMsg, setErrorMsg] = useState("");
  const hasRun = useRef(false);

  const accept = async () => {
    setStatus("loading");
    try {
      const result = await organizationsApi.acceptInvitation(token);
      if (result.accepted) {
        setStatus("success");
        setTimeout(() => router.push("/organizations"), 2000);
      }
    } catch (e) {
      setStatus("error");
      setErrorMsg(e instanceof ApiError ? e.message : "エラーが発生しました");
    }
  };

  useEffect(() => {
    // Guard against React StrictMode double-invoke: accepting twice would make
    // the second call hit an already-accepted invitation and flash a false error.
    if (token && !hasRun.current) {
      hasRun.current = true;
      accept();
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token]);

  return (
    <div className="min-h-screen bg-background flex items-center justify-center">
      <Card className="w-full max-w-md text-center">
        <CardHeader>
          <CardTitle>招待を承認</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          {status === "loading" && <p className="text-muted-foreground">処理中...</p>}
          {status === "success" && (
            <>
              <p className="text-green-600 font-medium">チームに参加しました！</p>
              <p className="text-sm text-muted-foreground">組織ページへリダイレクト中...</p>
            </>
          )}
          {status === "error" && (
            <>
              <p className="text-destructive">{errorMsg}</p>
              <Link href="/dashboard">
                <Button variant="outline">ダッシュボードへ</Button>
              </Link>
            </>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
