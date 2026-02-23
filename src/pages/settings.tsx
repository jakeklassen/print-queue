import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Save } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { getConfig, saveConfig, startWatcher } from "@/lib/api";
import type { AppConfig, PostFileAction } from "@/lib/types";

export function SettingsPage() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [dirty, setDirty] = useState(false);

  useEffect(() => {
    getConfig().then(setConfig);
  }, []);

  const update = (partial: Partial<AppConfig>) => {
    if (!config) return;
    setConfig({ ...config, ...partial });
    setDirty(true);
  };

  const handleSave = async () => {
    if (!config) return;
    await saveConfig(config);
    setDirty(false);
  };

  const handleSelectFolder = async () => {
    const folder = await open({ directory: true });
    if (folder && config) {
      const newConfig: AppConfig = { ...config, watch_folder: folder };
      await saveConfig(newConfig);
      await startWatcher(folder);
      setConfig(newConfig);
      setDirty(false);
    }
  };

  if (!config) return null;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold tracking-tight">Settings</h1>
        {dirty && (
          <Button onClick={handleSave}>
            <Save className="mr-2 h-4 w-4" />
            Save Changes
          </Button>
        )}
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Watch Folder</CardTitle>
          <CardDescription>
            The folder PrintQueue monitors for new files.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex items-center gap-2">
            <code className="flex-1 rounded bg-muted px-3 py-2 text-sm">
              {config.watch_folder || "Not configured"}
            </code>
            <Button variant="outline" onClick={handleSelectFolder}>
              <FolderOpen className="mr-2 h-4 w-4" />
              Browse
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Behavior</CardTitle>
          <CardDescription>
            How the app behaves when minimized or closed.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <Label>Minimize to System Tray</Label>
              <p className="text-xs text-muted-foreground">
                Keep watching and printing in the background when the window is
                closed.
              </p>
            </div>
            <Switch
              checked={config.minimize_to_tray}
              onCheckedChange={(v) => update({ minimize_to_tray: v })}
            />
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">File Handling</CardTitle>
          <CardDescription>
            What happens to files after processing.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-1.5">
            <Label>After Printing an Image</Label>
            <Select
              value={config.post_print_action}
              onValueChange={(v: PostFileAction) =>
                update({ post_print_action: v })
              }
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="move_to_subfolder">
                  Move to "printed" subfolder
                </SelectItem>
                <SelectItem value="delete">Delete</SelectItem>
                <SelectItem value="keep">Keep in place</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <Separator />

          <div className="grid gap-1.5">
            <Label>After Extracting a Zip</Label>
            <Select
              value={config.post_zip_action}
              onValueChange={(v: PostFileAction) =>
                update({ post_zip_action: v })
              }
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="delete">Delete the zip</SelectItem>
                <SelectItem value="move_to_subfolder">
                  Move to "processed_zips" subfolder
                </SelectItem>
                <SelectItem value="keep">Keep in place</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
