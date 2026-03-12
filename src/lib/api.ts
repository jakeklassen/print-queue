import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  Preset,
  PrinterInfo,
  PrinterCapabilities,
  PrintJob,
  MacOSPrintConfiguration,
} from "./types";

// Config
export const getConfig = () => invoke<AppConfig>("get_config");
export const saveConfig = (config: AppConfig) =>
  invoke<void>("save_config", { config });

// Presets
export const listPresets = () => invoke<Preset[]>("list_presets");
export const createPreset = (name: string, paperSizeKeyword: string) =>
  invoke<Preset>("create_preset", {
    name,
    paperSizeKeyword,
  });
export const updatePreset = (preset: Preset) =>
  invoke<Preset>("update_preset", { preset });
export const deletePreset = (id: string) =>
  invoke<void>("delete_preset", { id });

// Printers
export const listPrinters = () => invoke<PrinterInfo[]>("list_printers");
export const getPrinterCapabilities = (printerId: string) =>
  invoke<PrinterCapabilities>("get_printer_capabilities", {
    printerId,
  });

// Watcher
export type WatcherStatus = "idle" | "active" | "paused" | "error";

export const startWatcher = (watchFolder: string) =>
  invoke<void>("start_watcher", { watchFolder });
export const stopWatcher = () => invoke<void>("stop_watcher");
export const getWatcherStatus = () =>
  invoke<WatcherStatus>("get_watcher_status");

// Jobs
export const listJobs = () => invoke<PrintJob[]>("list_jobs");
export const cancelJob = (id: string) => invoke<void>("cancel_job", { id });
export const retryJob = (id: string) => invoke<void>("retry_job", { id });
export const reprintJob = (id: string) => invoke<void>("reprint_job", { id });

// Platform
export const getPlatform = () => invoke<string>("get_platform");

// Printer dialog (Windows only)
export const openPrinterDialog = (printerId: string) =>
  invoke<string>("open_printer_dialog", { printerId });

export const getBorderlessScaleFactor = (
  printerId: string,
  paperSizeKeyword: string,
) =>
  invoke<number>("get_borderless_scale_factor", {
    printerId,
    paperSizeKeyword,
  });

export const configureMacosPrinter = (
  printerId: string | null,
  paperSizeKeyword: string | null,
) =>
  invoke<MacOSPrintConfiguration>("configure_macos_printer", {
    printerId,
    paperSizeKeyword,
  });
