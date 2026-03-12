import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  FolderOpen,
  Eye,
  EyeOff,
  Printer,
  AlertCircle,
  CheckCircle2,
  Clock,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import {
  getConfig,
  saveConfig,
  listPresets,
  listJobs,
  startWatcher,
  stopWatcher,
  getWatcherStatus,
  type WatcherStatus,
} from "@/lib/api";
import type { AppConfig, Preset, PrintJob } from "@/lib/types";

export function DashboardPage() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [presets, setPresets] = useState<Preset[]>([]);
  const [jobs, setJobs] = useState<PrintJob[]>([]);
  const [watcherStatus, setWatcherStatus] = useState<WatcherStatus>("idle");

  const refresh = async () => {
    try {
      const [c, p, j, ws] = await Promise.all([
        getConfig(),
        listPresets(),
        listJobs(),
        getWatcherStatus(),
      ]);
      setConfig(c);
      setPresets(p);
      setJobs(j);
      setWatcherStatus(ws);
    } catch (err) {
      console.error("Dashboard refresh error:", err);
    }
  };

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 3000);

    const unlisteners = [
      listen("watcher-status", (event) => {
        setWatcherStatus(event.payload as WatcherStatus);
      }),
      listen("job-updated", () => refresh()),
    ];

    return () => {
      clearInterval(interval);
      unlisteners.forEach((p) => p.then((u) => u()));
    };
  }, []);

  const handleSelectFolder = async () => {
    const folder = await open({ directory: true });

    if (folder && config) {
      const newConfig: AppConfig = { ...config, watch_folder: folder };
      await saveConfig(newConfig);
      await startWatcher(folder);
      refresh();
    }
  };

  const handleToggleWatcher = async () => {
    if (watcherStatus === "active") {
      await stopWatcher();
    } else if (config?.watch_folder) {
      await startWatcher(config.watch_folder);
    }

    refresh();
  };

  const recentJobs = jobs.slice(-10).reverse();
  const pendingCount = jobs.filter((j) => j.status === "pending").length;
  const completeCount = jobs.filter((j) => j.status === "complete").length;
  const errorCount = jobs.filter((j) => j.status === "error").length;

  const statusVariant: "default" | "secondary" | "destructive" | "outline" =
    watcherStatus === "active"
      ? "default"
      : watcherStatus === "error"
        ? "destructive"
        : "secondary";

  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-semibold tracking-tight">Dashboard</h1>

      <div className="grid gap-4 sm:grid-cols-2">
        {/* Watch Folder */}
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Watch Folder</CardTitle>
          </CardHeader>
          <CardContent>
            {config?.watch_folder ? (
              <div className="space-y-2">
                <p className="text-sm font-mono truncate">
                  {config.watch_folder}
                </p>
                <div className="flex items-center gap-2">
                  <Badge variant={statusVariant}>
                    {watcherStatus === "active" ? (
                      <Eye className="mr-1 h-3 w-3" />
                    ) : (
                      <EyeOff className="mr-1 h-3 w-3" />
                    )}
                    {watcherStatus.charAt(0).toUpperCase() +
                      watcherStatus.slice(1)}
                  </Badge>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={handleToggleWatcher}
                  >
                    {watcherStatus === "active" ? "Pause" : "Start"}
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={handleSelectFolder}
                  >
                    Change
                  </Button>
                </div>
              </div>
            ) : (
              <Button
                variant="outline"
                className="w-full"
                onClick={handleSelectFolder}
              >
                <FolderOpen className="mr-2 h-4 w-4" />
                Select Watch Folder
              </Button>
            )}
          </CardContent>
        </Card>

        {/* Presets Summary */}
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Presets</CardTitle>
          </CardHeader>
          <CardContent>
            {presets.length === 0 ? (
              <p className="text-sm text-muted-foreground">
                No presets configured yet.
              </p>
            ) : (
              <div className="space-y-1">
                {presets.slice(0, 4).map((p) => (
                  <div
                    key={p.id}
                    className="flex items-center justify-between text-sm"
                  >
                    <span className="truncate">{p.name}</span>
                    <Badge variant="outline" className="text-xs">
                      {p.paper_size_keyword}
                    </Badge>
                  </div>
                ))}
                {presets.length > 4 && (
                  <p className="text-xs text-muted-foreground">
                    +{presets.length - 4} more
                  </p>
                )}
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Job Stats */}
      <div className="grid gap-4 grid-cols-3">
        <Card>
          <CardContent className="flex items-center gap-3 py-3">
            <Clock className="h-5 w-5 text-muted-foreground" />
            <div>
              <p className="text-2xl font-bold">{pendingCount}</p>
              <p className="text-xs text-muted-foreground">Pending</p>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="flex items-center gap-3 py-3">
            <CheckCircle2 className="h-5 w-5 text-green-500" />
            <div>
              <p className="text-2xl font-bold">{completeCount}</p>
              <p className="text-xs text-muted-foreground">Complete</p>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="flex items-center gap-3 py-3">
            <AlertCircle className="h-5 w-5 text-destructive" />
            <div>
              <p className="text-2xl font-bold">{errorCount}</p>
              <p className="text-xs text-muted-foreground">Errors</p>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Recent Activity */}
      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-sm font-medium">Recent Activity</CardTitle>
          <CardDescription>Latest print jobs</CardDescription>
        </CardHeader>
        <CardContent>
          {recentJobs.length === 0 ? (
            <p className="text-sm text-muted-foreground">No recent activity.</p>
          ) : (
            <div className="space-y-2">
              {recentJobs.map((job) => (
                <div
                  key={job.id}
                  className="flex items-center justify-between text-sm"
                >
                  <div className="flex items-center gap-2 min-w-0">
                    <Printer className="h-4 w-4 text-muted-foreground shrink-0" />
                    <span className="truncate">{job.filename}</span>
                  </div>
                  <div className="flex items-center gap-2 shrink-0">
                    {job.preset_name && (
                      <Badge variant="outline" className="text-xs">
                        {job.preset_name}
                      </Badge>
                    )}
                    <Badge
                      variant={
                        job.status === "complete"
                          ? "outline"
                          : job.status === "error"
                            ? "destructive"
                            : "secondary"
                      }
                      className="text-xs"
                    >
                      {job.status}
                    </Badge>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
