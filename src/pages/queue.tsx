import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  RefreshCw,
  XCircle,
  RotateCw,
  Printer,
  AlertCircle,
  CheckCircle2,
  Clock,
  Loader2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { listJobs, cancelJob, retryJob, reprintJob } from "@/lib/api";
import type { PrintJob, JobStatus } from "@/lib/types";

const statusConfig: Record<
  JobStatus,
  { icon: React.ElementType; label: string; variant: "default" | "secondary" | "destructive" | "outline" }
> = {
  pending: { icon: Clock, label: "Pending", variant: "secondary" },
  printing: { icon: Loader2, label: "Printing", variant: "default" },
  complete: { icon: CheckCircle2, label: "Complete", variant: "outline" },
  error: { icon: AlertCircle, label: "Error", variant: "destructive" },
  skipped: { icon: AlertCircle, label: "Skipped", variant: "secondary" },
};

export function QueuePage() {
  const [jobs, setJobs] = useState<PrintJob[]>([]);

  const refresh = async () => {
    try {
      const result = await listJobs();
      setJobs(result);
    } catch (err) {
      console.error("Failed to fetch jobs:", err);
    }
  };

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 2000);

    const unlisteners = [
      listen("job-updated", () => refresh()),
    ];

    return () => {
      clearInterval(interval);
      unlisteners.forEach((p) => p.then((unlisten) => unlisten()));
    };
  }, []);

  const handleCancel = async (id: string) => {
    await cancelJob(id);
    refresh();
  };

  const handleRetry = async (id: string) => {
    await retryJob(id);
    refresh();
  };

  const handleReprint = async (id: string) => {
    await reprintJob(id);
    refresh();
  };

  const activeJobs = jobs.filter(
    (j) => j.status === "pending" || j.status === "printing",
  );
  const completedJobs = jobs.filter(
    (j) => j.status === "complete" || j.status === "error" || j.status === "skipped",
  );

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold tracking-tight">Print Queue</h1>
        <Button variant="outline" size="sm" onClick={refresh}>
          <RefreshCw className="mr-2 h-4 w-4" />
          Refresh
        </Button>
      </div>

      {jobs.length === 0 ? (
        <Card>
          <CardContent className="py-8 text-center text-muted-foreground">
            No print jobs yet. Add files to your watch folder to get started.
          </CardContent>
        </Card>
      ) : (
        <>
          {activeJobs.length > 0 && (
            <div className="space-y-2">
              <h2 className="text-sm font-medium text-muted-foreground">
                Active ({activeJobs.length})
              </h2>
              <div className="grid gap-2">
                {activeJobs.map((job) => (
                  <JobCard
                    key={job.id}
                    job={job}
                    onCancel={handleCancel}
                    onRetry={handleRetry}
                    onReprint={handleReprint}
                  />
                ))}
              </div>
            </div>
          )}

          {completedJobs.length > 0 && (
            <div className="space-y-2">
              <h2 className="text-sm font-medium text-muted-foreground">
                History ({completedJobs.length})
              </h2>
              <ScrollArea className="h-[calc(100vh-280px)]">
                <div className="grid gap-2">
                  {completedJobs.map((job) => (
                    <JobCard
                      key={job.id}
                      job={job}
                      onCancel={handleCancel}
                      onRetry={handleRetry}
                      onReprint={handleReprint}
                    />
                  ))}
                </div>
              </ScrollArea>
            </div>
          )}
        </>
      )}
    </div>
  );
}

function JobCard({
  job,
  onCancel,
  onRetry,
  onReprint,
}: {
  job: PrintJob;
  onCancel: (id: string) => void;
  onRetry: (id: string) => void;
  onReprint: (id: string) => void;
}) {
  const config = statusConfig[job.status];
  const Icon = config.icon;

  return (
    <Card>
      <CardContent className="flex items-center justify-between py-3 px-4">
        <div className="flex items-center gap-3 min-w-0">
          <Printer className="h-8 w-8 text-muted-foreground shrink-0" />
          <div className="min-w-0">
            <p className="text-sm font-medium truncate">{job.filename}</p>
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              {job.preset_name && <span>{job.preset_name}</span>}
              <span>{new Date(job.created_at).toLocaleTimeString()}</span>
            </div>
            {job.error_message && (
              <p className="text-xs text-destructive mt-0.5">
                {job.error_message}
              </p>
            )}
          </div>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <Badge variant={config.variant}>
            <Icon
              className={`mr-1 h-3 w-3 ${job.status === "printing" ? "animate-spin" : ""}`}
            />
            {config.label}
          </Badge>
          {job.status === "pending" && (
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7"
              onClick={() => onCancel(job.id)}
            >
              <XCircle className="h-4 w-4" />
            </Button>
          )}
          {job.status === "error" && (
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7"
              onClick={() => onRetry(job.id)}
            >
              <RotateCw className="h-4 w-4" />
            </Button>
          )}
          {job.status === "complete" && (
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7"
              onClick={() => onReprint(job.id)}
            >
              <RotateCw className="h-4 w-4" />
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
