export type PostFileAction = "delete" | "move_to_subfolder" | "keep";

export interface AppConfig {
  watch_folder: string | null;
  minimize_to_tray: boolean;
  default_preset_id: string | null;
  post_print_action: PostFileAction;
  post_zip_action: PostFileAction;
}

/** Maps printer option key → selected value */
export type PrintSettings = Record<string, string>;

export interface Preset {
  id: string;
  name: string;
  printer_id: string | null;
  paper_size_keyword: string;
  settings: PrintSettings;
  copies: number;
  auto_print: boolean;
  scale_compensation: number;
  created_at: string;
  updated_at: string;
}

export interface PrinterInfo {
  id: string;
  name: string;
  driver: string;
  is_default: boolean;
  is_online: boolean;
}

export interface PrinterOptionChoice {
  value: string;
  label: string;
}

export interface PrinterOption {
  key: string;
  label: string;
  choices: PrinterOptionChoice[];
  default_choice: string | null;
}

export interface PrinterCapabilities {
  options: PrinterOption[];
}

export type JobStatus = "pending" | "printing" | "complete" | "error" | "needs_attention";

export interface PrintJob {
  id: string;
  filename: string;
  file_path: string;
  preset_id: string | null;
  preset_name: string | null;
  status: JobStatus;
  error_message: string | null;
  created_at: string;
  completed_at: string | null;
}
