import { useEffect, useRef, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { Button } from "@/components/ui/button";
import { Download, RefreshCw } from "lucide-react";

export function UpdateChecker() {
  const [version, setVersion] = useState("");
  const [state, setState] = useState<
    "idle" | "available" | "downloading" | "ready" | "error"
  >("idle");
  const [progress, setProgress] = useState("");
  const updateRef = useRef<Update | null>(null);

  useEffect(() => {
    check()
      .then((update) => {
        if (update) {
          updateRef.current = update;
          setVersion(update.version);
          setState("available");
        }
      })
      .catch((e) => console.error("Update check failed:", e));
  }, []);

  const installUpdate = async () => {
    const update = updateRef.current;
    if (!update) {
      setState("error");
      setProgress("Update no longer available");
      return;
    }

    setState("downloading");
    try {
      let totalLength = 0;
      let downloaded = 0;

      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            totalLength = event.data.contentLength ?? 0;
            setProgress("Downloading...");
            break;
          case "Progress":
            downloaded += event.data.chunkLength;
            if (totalLength > 0) {
              const pct = Math.round((downloaded / totalLength) * 100);
              setProgress(`Downloading... ${pct}%`);
            }
            break;
          case "Finished":
            setProgress("Update installed — restart the app to apply");
            break;
        }
      });
      setState("ready");
    } catch (e) {
      console.error("Update install failed:", e);
      setState("error");
      setProgress(e instanceof Error ? e.message : "Update failed");
    }
  };

  if (state === "idle") return null;

  return (
    <div className="mb-4 flex items-center gap-2 rounded-md border bg-muted/50 px-3 py-2 text-sm">
      {state === "ready" ? (
        <RefreshCw className="h-4 w-4 shrink-0" />
      ) : (
        <Download className="h-4 w-4 shrink-0" />
      )}
      <span className="truncate">
        {state === "available" && `v${version} available`}
        {state === "downloading" && progress}
        {state === "ready" && progress}
        {state === "error" && progress}
      </span>
      {state === "available" && (
        <Button
          size="sm"
          variant="outline"
          className="ml-auto h-7 text-xs"
          onClick={installUpdate}
        >
          Update
        </Button>
      )}
      {state === "error" && (
        <Button
          size="sm"
          variant="outline"
          className="ml-auto h-7 text-xs"
          onClick={() => {
            setState("available");
            setProgress("");
          }}
        >
          Retry
        </Button>
      )}
    </div>
  );
}
