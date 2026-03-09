use std::sync::Arc;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::jobs::{JobQueueState, JobStatus, PrintJob};
use crate::models::{AppConfig, Preset};
use crate::printing::{self, PrinterCapabilities, PrinterInfo};
use crate::storage::StorageState;
use crate::watcher::{self, WatcherState, WatcherStatus};

#[tauri::command]
pub fn get_platform() -> String {
    if cfg!(target_os = "windows") {
        "windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else {
        "linux".to_string()
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub fn open_printer_dialog(printer_id: String) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let printer_escaped = printer_id.replace('\'', "''");

    let script = format!(
        r#"$ErrorActionPreference = 'Stop'
try {{
    Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class PrinterDialog {{
    [DllImport("winspool.drv", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool OpenPrinter(string pPrinterName, out IntPtr phPrinter, IntPtr pDefault);

    [DllImport("winspool.drv", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern int DocumentProperties(
        IntPtr hWnd, IntPtr hPrinter, string pDeviceName,
        IntPtr pDevModeOutput, IntPtr pDevModeInput, int fMode);

    [DllImport("winspool.drv", SetLastError = true)]
    public static extern bool ClosePrinter(IntPtr hPrinter);

    // fMode flags
    public const int DM_IN_BUFFER  = 8;
    public const int DM_IN_PROMPT  = 4;
    public const int DM_OUT_BUFFER = 2;
    // IDOK
    public const int IDOK = 1;
}}
"@

    $printerName = '{printer_name}'

    # Open printer handle
    $hPrinter = [IntPtr]::Zero
    if (-not [PrinterDialog]::OpenPrinter($printerName, [ref]$hPrinter, [IntPtr]::Zero)) {{
        throw "OpenPrinter failed for '$printerName' (error $([System.Runtime.InteropServices.Marshal]::GetLastWin32Error()))"
    }}

    try {{
        # Get required DEVMODE size
        $cbNeeded = [PrinterDialog]::DocumentProperties(
            [IntPtr]::Zero, $hPrinter, $printerName,
            [IntPtr]::Zero, [IntPtr]::Zero, 0)
        if ($cbNeeded -le 0) {{
            throw "DocumentProperties size query failed"
        }}

        # Allocate DEVMODE buffer
        $pDevMode = [System.Runtime.InteropServices.Marshal]::AllocHGlobal($cbNeeded)

        try {{
            # Get default DEVMODE first
            $ret = [PrinterDialog]::DocumentProperties(
                [IntPtr]::Zero, $hPrinter, $printerName,
                $pDevMode, [IntPtr]::Zero,
                [PrinterDialog]::DM_OUT_BUFFER)

            # Show the dialog with current defaults as input
            $ret = [PrinterDialog]::DocumentProperties(
                [IntPtr]::Zero, $hPrinter, $printerName,
                $pDevMode, $pDevMode,
                [PrinterDialog]::DM_IN_BUFFER -bor [PrinterDialog]::DM_IN_PROMPT -bor [PrinterDialog]::DM_OUT_BUFFER)

            if ($ret -ne [PrinterDialog]::IDOK) {{
                Write-Output "CANCELLED"
                exit 0
            }}

            # Copy DEVMODE bytes
            $devModeBytes = New-Object byte[] $cbNeeded
            [System.Runtime.InteropServices.Marshal]::Copy($pDevMode, $devModeBytes, 0, $cbNeeded)

            # Output as base64
            $b64 = [System.Convert]::ToBase64String($devModeBytes)
            Write-Output "DEVMODE:$b64"
        }} finally {{
            [System.Runtime.InteropServices.Marshal]::FreeHGlobal($pDevMode)
        }}
    }} finally {{
        [void][PrinterDialog]::ClosePrinter($hPrinter)
    }}
}} catch {{
    Write-Error "DIALOG_ERROR: $($_.Exception.Message)"
    exit 1
}}
"#,
        printer_name = printer_escaped,
    );

    let script_path = std::env::temp_dir().join(format!(
        "printqueue_dialog_{}.ps1",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));
    std::fs::write(&script_path, &script)
        .map_err(|e| format!("Failed to write dialog script: {}", e))?;

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy", "Bypass",
            "-File",
            &script_path.display().to_string(),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to launch powershell: {}", e))?;

    std::fs::remove_file(&script_path).ok();

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if stdout == "CANCELLED" {
        return Err("User cancelled the dialog".to_string());
    }

    if let Some(b64) = stdout.strip_prefix("DEVMODE:") {
        return Ok(b64.to_string());
    }

    Err(format!(
        "Printer dialog failed: {}",
        if !stderr.is_empty() { &stderr } else { &stdout }
    ))
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn open_printer_dialog(_printer_id: String) -> Result<String, String> {
    Err("Printer dialog is only available on Windows".to_string())
}

#[tauri::command]
pub fn get_config(state: State<Arc<StorageState>>) -> Result<AppConfig, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    Ok(config.clone())
}

#[tauri::command]
pub fn save_config(state: State<Arc<StorageState>>, config: AppConfig) -> Result<(), String> {
    state.save_config(&config)?;
    let mut current = state.config.lock().map_err(|e| e.to_string())?;
    *current = config;
    Ok(())
}

#[tauri::command]
pub fn list_presets(state: State<Arc<StorageState>>) -> Result<Vec<Preset>, String> {
    let presets = state.presets.lock().map_err(|e| e.to_string())?;
    Ok(presets.clone())
}

#[tauri::command]
pub fn create_preset(state: State<Arc<StorageState>>, name: String, paper_size_keyword: String) -> Result<Preset, String> {
    let preset = Preset::new(name, paper_size_keyword);
    let mut presets = state.presets.lock().map_err(|e| e.to_string())?;
    presets.push(preset.clone());
    state.save_presets(&presets)?;
    Ok(preset)
}

#[tauri::command]
pub fn update_preset(state: State<Arc<StorageState>>, preset: Preset) -> Result<Preset, String> {
    let mut presets = state.presets.lock().map_err(|e| e.to_string())?;
    let idx = presets.iter().position(|p| p.id == preset.id)
        .ok_or_else(|| format!("Preset {} not found", preset.id))?;
    presets[idx] = preset.clone();
    state.save_presets(&presets)?;
    Ok(preset)
}

#[tauri::command]
pub fn delete_preset(state: State<Arc<StorageState>>, id: Uuid) -> Result<(), String> {
    let mut presets = state.presets.lock().map_err(|e| e.to_string())?;
    let idx = presets.iter().position(|p| p.id == id)
        .ok_or_else(|| format!("Preset {} not found", id))?;
    presets.remove(idx);
    state.save_presets(&presets)?;
    Ok(())
}

#[tauri::command]
pub fn list_printers() -> Vec<PrinterInfo> {
    printing::discover_printers()
}

#[tauri::command]
pub fn get_printer_capabilities(printer_id: String) -> PrinterCapabilities {
    printing::get_printer_capabilities(&printer_id)
}

#[tauri::command]
pub fn start_watcher(
    app_handle: AppHandle,
    watch_folder: String,
    watcher_state: State<Arc<WatcherState>>,
    storage_state: State<Arc<StorageState>>,
    job_queue: State<Arc<JobQueueState>>,
) -> Result<(), String> {
    watcher::start_watcher(
        app_handle,
        watch_folder,
        (*watcher_state).clone(),
        (*storage_state).clone(),
        (*job_queue).clone(),
    )
}

#[tauri::command]
pub fn stop_watcher(watcher_state: State<Arc<WatcherState>>) -> Result<(), String> {
    watcher::stop_watcher_inner(&watcher_state);
    Ok(())
}

#[tauri::command]
pub fn get_watcher_status(watcher_state: State<Arc<WatcherState>>) -> Result<WatcherStatus, String> {
    let status = watcher_state.status.lock().map_err(|e| e.to_string())?;
    Ok(status.clone())
}

#[tauri::command]
pub fn list_jobs(queue: State<Arc<JobQueueState>>) -> Result<Vec<PrintJob>, String> {
    let jobs = queue.jobs.lock().map_err(|e| e.to_string())?;
    Ok(jobs.clone())
}

#[tauri::command]
pub fn cancel_job(queue: State<Arc<JobQueueState>>, id: Uuid) -> Result<(), String> {
    let mut jobs = queue.jobs.lock().map_err(|e| e.to_string())?;
    if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
        if job.status == JobStatus::Pending {
            job.status = JobStatus::Error;
            job.error_message = Some("Cancelled by user".to_string());
        }
    }
    Ok(())
}

#[tauri::command]
pub fn retry_job(
    queue: State<Arc<JobQueueState>>,
    id: Uuid,
) -> Result<(), String> {
    let mut jobs = queue.jobs.lock().map_err(|e| e.to_string())?;
    if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
        if job.status == JobStatus::Error {
            job.status = JobStatus::Pending;
            job.error_message = None;
            job.completed_at = None;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn reprint_job(
    queue: State<Arc<JobQueueState>>,
    id: Uuid,
) -> Result<(), String> {
    let jobs = queue.jobs.lock().map_err(|e| e.to_string())?;
    let original = jobs
        .iter()
        .find(|j| j.id == id)
        .ok_or_else(|| "Job not found".to_string())?
        .clone();
    drop(jobs);

    let new_job = PrintJob::new(
        original.filename,
        original.file_path,
        original.preset_id,
        original.preset_name,
    );
    queue.add_job(new_job);
    Ok(())
}
