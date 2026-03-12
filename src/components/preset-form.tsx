import { useEffect, useState } from "react";
import { Settings2, X, Check } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { PrinterSelector } from "@/components/printer-selector";
import { CapabilitiesForm } from "@/components/capabilities-form";
import {
  configureMacosPrinter,
  getPlatform,
  openPrinterDialog,
} from "@/lib/api";
import type { Preset, PrintSettings } from "@/lib/types";

const DEFAULT_SETTINGS: PrintSettings = {};

export interface PresetFormData {
  name: string;
  printer_id: string | null;
  paper_size_keyword: string;
  settings: PrintSettings;
  copies: number;
  auto_print: boolean;
  scale_compensation: number;
  devmode_base64: string | null;
  macos_print_info_base64: string | null;
  macos_page_format_base64: string | null;
  macos_print_settings_base64: string | null;
  macos_printer_name: string | null;
  macos_page_width_points: number | null;
  macos_page_height_points: number | null;
  macos_size_compensation_mm: number | null;
}

interface PresetFormProps {
  preset?: Preset;
  onSave: (data: PresetFormData) => void;
  onCancel: () => void;
}

export function PresetForm({ preset, onSave, onCancel }: PresetFormProps) {
  const deriveMacosCompensationMm = (value?: Preset) => {
    if (value?.macos_size_compensation_mm != null) {
      return value.macos_size_compensation_mm;
    }

    if (
      value?.macos_page_width_points &&
      value.scale_compensation > 0 &&
      value.scale_compensation < 0.999
    ) {
      const pageWidthMm = (value.macos_page_width_points / 72) * 25.4;
      return (pageWidthMm * value.scale_compensation) - pageWidthMm;
    }

    return 0;
  };

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
  const [macosPrintInfoBase64, setMacosPrintInfoBase64] = useState<string | null>(
    preset?.macos_print_info_base64 ?? null,
  );
  const [macosPrinterName, setMacosPrinterName] = useState<string | null>(
    preset?.macos_printer_name ?? null,
  );
  const [macosPageFormatBase64, setMacosPageFormatBase64] = useState<string | null>(
    preset?.macos_page_format_base64 ?? null,
  );
  const [macosPrintSettingsBase64, setMacosPrintSettingsBase64] = useState<string | null>(
    preset?.macos_print_settings_base64 ?? null,
  );
  const [macosPageWidthPoints, setMacosPageWidthPoints] = useState<number | null>(
    preset?.macos_page_width_points ?? null,
  );
  const [macosPageHeightPoints, setMacosPageHeightPoints] = useState<number | null>(
    preset?.macos_page_height_points ?? null,
  );
  const [macosSizeCompensationMm, setMacosSizeCompensationMm] = useState<number>(
    deriveMacosCompensationMm(preset),
  );
  const [scaleCompensation] = useState(
    preset?.scale_compensation ?? 1.0,
  );
  const [platform, setPlatform] = useState<string | null>(null);
  const [dialogLoading, setDialogLoading] = useState(false);
  const [macosConfigError, setMacosConfigError] = useState<string | null>(null);

  useEffect(() => {
    getPlatform().then(setPlatform);
  }, []);

  const platformLoaded = platform !== null;
  const isWindows = platform === "windows";
  const isMacos = platform === "macos";
  const hasMacosNativeConfig = Boolean(
    macosPrintInfoBase64 &&
      macosPageFormatBase64 &&
      macosPrintSettingsBase64 &&
      macosPrinterName,
  );

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

  const handleConfigureMacos = async () => {
    setDialogLoading(true);
    setMacosConfigError(null);
    try {
      const config = await configureMacosPrinter(
        printerId,
        paperSizeKeyword || null,
      );
      if (
        !config.print_info_base64 ||
        !config.page_format_base64 ||
        !config.print_settings_base64 ||
        !config.printer_name
      ) {
        const missing = [
          !config.printer_name ? "printer_name" : null,
          !config.print_info_base64 ? "print_info_base64" : null,
          !config.page_format_base64 ? "page_format_base64" : null,
          !config.print_settings_base64 ? "print_settings_base64" : null,
        ]
          .filter(Boolean)
          .join(", ");
        setMacosConfigError(`macOS configure returned incomplete data: ${missing}`);
        return;
      }
      console.log("macOS printer configuration captured", {
        printer_name: config.printer_name,
        print_info_length: config.print_info_base64.length,
        page_format_length: config.page_format_base64.length,
        print_settings_length: config.print_settings_base64.length,
      });
      setPrinterId(config.printer_name);
      setMacosPrinterName(config.printer_name);
      setMacosPrintInfoBase64(config.print_info_base64);
      setMacosPageFormatBase64(config.page_format_base64);
      setMacosPrintSettingsBase64(config.print_settings_base64);
      setMacosPageWidthPoints(config.page_width_points);
      setMacosPageHeightPoints(config.page_height_points);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (!msg.includes("cancelled")) {
        console.error("macOS printer configuration error:", msg);
        setMacosConfigError(msg);
      }
    } finally {
      setDialogLoading(false);
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (isMacos && !hasMacosNativeConfig) {
      setMacosConfigError(
        "Run Configure Printer Settings and save the native macOS printer configuration before saving this preset.",
      );
      return;
    }
    onSave({
      name,
      printer_id: printerId,
      paper_size_keyword: paperSizeKeyword,
      settings,
      copies,
      auto_print: autoPrint,
      scale_compensation: isMacos ? 1.0 : scaleCompensation,
      devmode_base64: devmodeBase64,
      macos_print_info_base64: macosPrintInfoBase64,
      macos_page_format_base64: macosPageFormatBase64,
      macos_print_settings_base64: macosPrintSettingsBase64,
      macos_printer_name: macosPrinterName,
      macos_page_width_points: macosPageWidthPoints,
      macos_page_height_points: macosPageHeightPoints,
      macos_size_compensation_mm: isMacos ? macosSizeCompensationMm : null,
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

      {isMacos && (
        <div className="grid gap-1.5">
          <Label>Printer Settings (macOS)</Label>
          <div className="flex items-center gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={dialogLoading}
              onClick={handleConfigureMacos}
            >
              <Settings2 className="mr-2 h-4 w-4" />
              {dialogLoading ? "Opening print dialog..." : "Configure Printer Settings"}
            </Button>
            {hasMacosNativeConfig ? (
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
                  onClick={() => {
                    setMacosPrintInfoBase64(null);
                    setMacosPageFormatBase64(null);
                    setMacosPrintSettingsBase64(null);
                    setMacosPrinterName(null);
                  }}
                >
                  <X className="h-3.5 w-3.5" />
                </Button>
              </div>
            ) : (
              <span className="text-xs text-muted-foreground">Not configured</span>
            )}
          </div>
          <p className="text-xs text-muted-foreground">
            Opens the native macOS print dialog using a real sample page when available and stores the resulting print configuration for headless reuse.
          </p>
          <p className="text-xs text-muted-foreground">
            For exact template positioning, prefer custom page sizes in macOS Page Setup instead of vendor borderless paper presets.
          </p>
          {macosConfigError && (
            <p className="text-xs text-destructive">
              {macosConfigError}
            </p>
          )}
          {macosPrinterName && (
            <p className="text-xs text-muted-foreground">
              Captured printer: {macosPrinterName}
            </p>
          )}
          {hasMacosNativeConfig && (
            <p className="text-xs text-muted-foreground">
              Native config captured:
              {" "}
              printInfo {macosPrintInfoBase64?.length ?? 0} chars,
              {" "}
              pageFormat {macosPageFormatBase64?.length ?? 0} chars,
              {" "}
              printSettings {macosPrintSettingsBase64?.length ?? 0} chars.
            </p>
          )}
        </div>
      )}

      {isMacos && macosPrintInfoBase64 && (
        <div className="grid gap-1.5">
          <Label htmlFor="macos-size-compensation">Size Compensation (mm)</Label>
          <Input
            id="macos-size-compensation"
            type="number"
            step={0.1}
            value={macosSizeCompensationMm}
            onChange={(e) => {
              const value = Number.parseFloat(e.target.value);
              setMacosSizeCompensationMm(Number.isFinite(value) ? value : 0);
            }}
          />
          <p className="text-xs text-muted-foreground">
            Negative values shrink the printed content. Positive values enlarge it.
            Use this only when the measured print size needs correction.
          </p>
        </div>
      )}

      <div className="grid gap-1.5">
        <Label htmlFor="keyword">Paper Size Keyword</Label>
        <Input
          id="keyword"
          value={paperSizeKeyword}
          onChange={(e) => {
            setPaperSizeKeyword(e.target.value);
            if (isMacos) {
              setMacosConfigError(null);
            }
          }}
          placeholder="e.g., 4x6, A4, letter"
          required
        />
        <p className="text-xs text-muted-foreground">
          Files with this keyword in their name will route to this preset.
        </p>
      </div>

      {platformLoaded && !isWindows && !isMacos && (
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
        <Button type="submit">
          {preset ? "Save Changes" : "Create Preset"}
        </Button>
      </div>
    </form>
  );
}
