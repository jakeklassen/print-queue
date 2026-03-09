import { useEffect, useState } from "react";
import { Settings2, X, Check } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { PrinterSelector } from "@/components/printer-selector";
import { CapabilitiesForm } from "@/components/capabilities-form";
import { getPlatform, openPrinterDialog } from "@/lib/api";
import type { Preset, PrintSettings } from "@/lib/types";

const DEFAULT_SETTINGS: PrintSettings = {};

interface PresetFormProps {
  preset?: Preset;
  onSave: (data: {
    name: string;
    printer_id: string | null;
    paper_size_keyword: string;
    settings: PrintSettings;
    copies: number;
    auto_print: boolean;
    devmode_base64: string | null;
  }) => void;
  onCancel: () => void;
}

export function PresetForm({ preset, onSave, onCancel }: PresetFormProps) {
  const [name, setName] = useState(preset?.name ?? "");
  const [printerId, setPrinterId] = useState<string | null>(
    preset?.printer_id ?? null,
  );
  const [paperSizeKeyword, setPaperSizeKeyword] = useState(
    preset?.paper_size_keyword ?? "",
  );
  const [settings, setSettings] = useState<PrintSettings>(
    preset?.settings ?? DEFAULT_SETTINGS,
  );
  const [copies, setCopies] = useState(preset?.copies ?? 1);
  const [autoPrint, setAutoPrint] = useState(preset?.auto_print ?? true);
  const [devmodeBase64, setDevmodeBase64] = useState<string | null>(
    preset?.devmode_base64 ?? null,
  );
  const [isWindows, setIsWindows] = useState(false);
  const [dialogLoading, setDialogLoading] = useState(false);

  useEffect(() => {
    getPlatform().then((p) => setIsWindows(p === "windows"));
  }, []);

  const handleOpenDialog = async () => {
    if (!printerId) return;
    setDialogLoading(true);
    try {
      const b64 = await openPrinterDialog(printerId);
      setDevmodeBase64(b64);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (!msg.includes("cancelled")) {
        console.error("Printer dialog error:", msg);
      }
    } finally {
      setDialogLoading(false);
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSave({
      name,
      printer_id: printerId,
      paper_size_keyword: paperSizeKeyword,
      settings,
      copies,
      auto_print: autoPrint,
      devmode_base64: devmodeBase64,
    });
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-6">
      <div className="grid gap-1.5">
        <Label htmlFor="preset-name">Preset Name</Label>
        <Input
          id="preset-name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="e.g., 4x6 Photo Magnets"
          required
        />
      </div>

      <div className="grid gap-1.5">
        <Label>Printer</Label>
        <PrinterSelector value={printerId} onChange={setPrinterId} />
      </div>

      {isWindows && (
        <div className="grid gap-1.5">
          <Label>Driver Settings (Windows)</Label>
          <div className="flex items-center gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={!printerId || dialogLoading}
              onClick={handleOpenDialog}
            >
              <Settings2 className="mr-2 h-4 w-4" />
              {dialogLoading ? "Waiting for dialog..." : "Configure Printer Settings"}
            </Button>
            {devmodeBase64 ? (
              <div className="flex items-center gap-1.5">
                <span className="flex items-center gap-1 text-xs text-green-600">
                  <Check className="h-3.5 w-3.5" />
                  Configured
                </span>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="h-6 w-6"
                  onClick={() => setDevmodeBase64(null)}
                >
                  <X className="h-3.5 w-3.5" />
                </Button>
              </div>
            ) : (
              <span className="text-xs text-muted-foreground">Not configured</span>
            )}
          </div>
          <p className="text-xs text-muted-foreground">
            Opens the native printer driver dialog to capture all settings including vendor-specific options.
          </p>
        </div>
      )}

      <div className="grid gap-1.5">
        <Label htmlFor="keyword">Paper Size Keyword</Label>
        <Input
          id="keyword"
          value={paperSizeKeyword}
          onChange={(e) => setPaperSizeKeyword(e.target.value)}
          placeholder="e.g., 4x6, A4, letter"
          required
        />
        <p className="text-xs text-muted-foreground">
          Files with this keyword in their name will route to this preset.
        </p>
      </div>

      {!isWindows && (
        <div className="space-y-3">
          <Label className="text-sm font-medium">Print Settings</Label>
          <CapabilitiesForm
            printerId={printerId}
            settings={settings}
            onChange={setSettings}
          />
        </div>
      )}

      <div className="grid gap-1.5">
        <Label htmlFor="copies">Copies</Label>
        <Input
          id="copies"
          type="number"
          min={1}
          max={100}
          value={copies}
          onChange={(e) => setCopies(parseInt(e.target.value) || 1)}
        />
      </div>

      <div className="flex items-center gap-3">
        <Switch
          id="auto-print"
          checked={autoPrint}
          onCheckedChange={setAutoPrint}
        />
        <Label htmlFor="auto-print">Auto-print (no confirmation)</Label>
      </div>

      <div className="flex justify-end gap-2">
        <Button type="button" variant="outline" onClick={onCancel}>
          Cancel
        </Button>
        <Button type="submit">{preset ? "Save Changes" : "Create Preset"}</Button>
      </div>
    </form>
  );
}
