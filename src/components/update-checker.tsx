import { useEffect, useState } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { Button } from "@/components/ui/button";
import { Download } from "lucide-react";

export function UpdateChecker() {
  const [updateAvailable, setUpdateAvailable] = useState(false);
  const [version, setVersion] = useState("");
  const [installing, setInstalling] = useState(false);
  const [progress, setProgress] = useState("");

  useEffect(() => {
    checkForUpdate();
  }, []);

  const checkForUpdate = async () => {
    try {
      const update = await check();
      if (update) {
        setUpdateAvailable(true);
        setVersion(update.version);
      }
    } catch (e) {
      console.error("Update check failed:", e);
    }
  };

  const installUpdate = async () => {
    setInstalling(true);
    try {
      const update = await check();
      if (!update) return;

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
            setProgress("Restart to finish update");
            break;
        }
      });
    } catch (e) {
      console.error("Update install failed:", e);
      setInstalling(false);
      setProgress("");
    }
  };

  if (!updateAvailable) return null;

  return (
    <div className="mb-4 flex items-center gap-2 rounded-md border bg-muted/50 px-3 py-2 text-sm">
      <Download className="h-4 w-4 shrink-0" />
      <span className="truncate">
        {installing ? progress : `v${version} available`}
      </span>
      {!installing && (
        <Button size="sm" variant="outline" className="ml-auto h-7 text-xs" onClick={installUpdate}>
          Update
        </Button>
      )}
    </div>
  );
}
