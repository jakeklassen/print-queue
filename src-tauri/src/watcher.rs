use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use notify::event::CreateKind;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

use crate::jobs::{JobQueueState, JobStatus, PrintJob};
use crate::models::{PostFileAction, Preset};
use crate::parser::parse_size_keyword;
use crate::storage::StorageState;

/// How long after processing a file before we'll process it again.
/// This prevents duplicate events from the file watcher (Create + multiple Modify events)
/// from creating multiple queue entries.
const DEDUP_WINDOW: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WatcherStatus {
    Idle,
    Active,
    Paused,
    Error,
}

pub struct WatcherState {
    pub status: Mutex<WatcherStatus>,
    pub watcher: Mutex<Option<RecommendedWatcher>>,
    processed_zips: Mutex<HashSet<String>>,
    /// Tracks recently processed files with their processing timestamp.
    /// Events for the same path within DEDUP_WINDOW are ignored.
    recently_processed: Mutex<HashMap<PathBuf, Instant>>,
}

impl WatcherState {
    pub fn new() -> Self {
        Self {
            status: Mutex::new(WatcherStatus::Idle),
            watcher: Mutex::new(None),
            processed_zips: Mutex::new(HashSet::new()),
            recently_processed: Mutex::new(HashMap::new()),
        }
    }
}

const SUPPORTED_IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "tiff", "tif"];

fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_IMAGE_EXTS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn is_zip_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase() == "zip")
        .unwrap_or(false)
}

fn file_hash(path: &Path) -> Result<String, String> {
    let data = fs::read(path).map_err(|e| e.to_string())?;
    let hash = Sha256::digest(&data);
    Ok(hex::encode(hash))
}

fn extract_zip(zip_path: &Path, target_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let file = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut extracted = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }

        let name = entry
            .enclosed_name()
            .and_then(|p| p.file_name().map(|f| f.to_os_string()))
            .ok_or_else(|| "Invalid zip entry name".to_string())?;

        let out_path = target_dir.join(&name);

        // Only extract image files
        if !is_image_file(&out_path) {
            continue;
        }

        // Avoid overwriting existing files
        if out_path.exists() {
            continue;
        }

        let mut out_file = fs::File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out_file).map_err(|e| e.to_string())?;
        extracted.push(out_path);
    }

    Ok(extracted)
}

#[derive(Clone, Serialize)]
struct FileEvent {
    event_type: String,
    path: String,
    keyword: Option<String>,
    preset_name: Option<String>,
}

pub fn start_watcher(
    app_handle: AppHandle,
    watch_folder: String,
    watcher_state: Arc<WatcherState>,
    storage_state: Arc<StorageState>,
    job_queue: Arc<JobQueueState>,
) -> Result<(), String> {
    let folder = PathBuf::from(&watch_folder);
    if !folder.exists() {
        return Err(format!("Watch folder does not exist: {}", watch_folder));
    }

    // Stop any existing watcher
    stop_watcher_inner(&watcher_state);

    let app = app_handle.clone();
    let ws = watcher_state.clone();
    let ss = storage_state.clone();
    let jq = job_queue.clone();

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        match res {
            Ok(event) => {
                if matches!(event.kind, EventKind::Create(CreateKind::File) | EventKind::Modify(_)) {
                    for path in event.paths {
                        // Debounce: wait for file to finish writing
                        std::thread::sleep(Duration::from_millis(1500));
                        handle_file(&app, &path, &ws, &ss, &jq);
                    }
                }
            }
            Err(e) => {
                eprintln!("Watcher error: {}", e);
                if let Ok(mut status) = ws.status.lock() {
                    *status = WatcherStatus::Error;
                }
                let _ = app.emit("watcher-status", WatcherStatus::Error);
            }
        }
    })
    .map_err(|e| e.to_string())?;

    watcher
        .watch(folder.as_ref(), RecursiveMode::NonRecursive)
        .map_err(|e| e.to_string())?;

    // Store the watcher and update status
    *watcher_state.watcher.lock().map_err(|e| e.to_string())? = Some(watcher);
    *watcher_state.status.lock().map_err(|e| e.to_string())? = WatcherStatus::Active;
    let _ = app_handle.emit("watcher-status", WatcherStatus::Active);

    Ok(())
}

fn handle_file(
    app: &AppHandle,
    path: &Path,
    watcher_state: &WatcherState,
    storage_state: &StorageState,
    job_queue: &JobQueueState,
) {
    if !path.exists() || !path.is_file() {
        return;
    }

    // Skip files in "printed" subfolder
    if path.parent().and_then(|p| p.file_name()).map(|n| n == "printed").unwrap_or(false) {
        return;
    }

    // Skip files in "processed_zips" subfolder
    if path.parent().and_then(|p| p.file_name()).map(|n| n == "processed_zips").unwrap_or(false) {
        return;
    }

    // Dedup: skip if this file was processed recently
    {
        let mut recent = watcher_state.recently_processed.lock().unwrap();
        let now = Instant::now();

        // Clean up stale entries while we have the lock
        recent.retain(|_, timestamp| now.duration_since(*timestamp) < DEDUP_WINDOW);

        if let Some(last_processed) = recent.get(path) {
            if now.duration_since(*last_processed) < DEDUP_WINDOW {
                return; // Already processed recently, skip
            }
        }

        // Mark as processing now (before the sleep) to block concurrent events
        recent.insert(path.to_path_buf(), now);
    }

    let result = if is_zip_file(path) {
        handle_zip(app, path, watcher_state, storage_state)
    } else if is_image_file(path) {
        handle_image(app, path, storage_state, job_queue)
    } else {
        Ok(())
    };

    if let Err(e) = result {
        eprintln!("Error processing {}: {}", path.display(), e);
        let _ = app.emit(
            "file-event",
            FileEvent {
                event_type: "error".to_string(),
                path: path.display().to_string(),
                keyword: None,
                preset_name: None,
            },
        );
    }
}

fn handle_zip(
    app: &AppHandle,
    zip_path: &Path,
    watcher_state: &WatcherState,
    storage_state: &StorageState,
) -> Result<(), String> {
    // Check if already processed
    let hash = file_hash(zip_path)?;
    let key = format!(
        "{}:{}",
        zip_path.file_name().unwrap_or_default().to_string_lossy(),
        hash
    );

    {
        let mut processed = watcher_state.processed_zips.lock().map_err(|e| e.to_string())?;
        if processed.contains(&key) {
            return Ok(());
        }
        processed.insert(key);
    }

    let target_dir = zip_path.parent().unwrap_or(Path::new("."));
    let extracted = extract_zip(zip_path, target_dir)?;

    let _ = app.emit(
        "file-event",
        FileEvent {
            event_type: "zip_extracted".to_string(),
            path: zip_path.display().to_string(),
            keyword: None,
            preset_name: Some(format!("{} files extracted", extracted.len())),
        },
    );

    // Post-zip action
    let config = storage_state.config.lock().map_err(|e| e.to_string())?;
    match config.post_zip_action {
        PostFileAction::Delete => {
            fs::remove_file(zip_path).ok();
        }
        PostFileAction::MoveToSubfolder => {
            let processed_dir = target_dir.join("processed_zips");
            fs::create_dir_all(&processed_dir).ok();
            if let Some(name) = zip_path.file_name() {
                fs::rename(zip_path, processed_dir.join(name)).ok();
            }
        }
        PostFileAction::Keep => {}
    }

    Ok(())
}

fn handle_image(
    app: &AppHandle,
    image_path: &Path,
    storage_state: &StorageState,
    job_queue: &JobQueueState,
) -> Result<(), String> {
    let filename = image_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let keyword = parse_size_keyword(&filename);

    let presets = storage_state.presets.lock().map_err(|e| e.to_string())?;
    let config = storage_state.config.lock().map_err(|e| e.to_string())?;

    let matched_preset = if let Some(ref kw) = keyword {
        presets
            .iter()
            .find(|p| p.paper_size_keyword.to_lowercase() == kw.to_lowercase())
    } else {
        // Use default preset
        config
            .default_preset_id
            .and_then(|id| presets.iter().find(|p| p.id == id))
    };

    if let Some(preset) = matched_preset {
        // Create a job in the queue
        let mut job = PrintJob::new(
            filename.clone(),
            image_path.display().to_string(),
            Some(preset.id),
            Some(preset.name.clone()),
        );

        if preset.auto_print {
            // Mark as printing
            job.status = JobStatus::Printing;
            let job_id = job.id;
            job_queue.add_job(job);
            let _ = app.emit("job-updated", "");

            // Submit print job
            let print_result = submit_print_job(image_path, preset);
            match print_result {
                Ok(()) => {
                    job_queue.update_status(job_id, JobStatus::Complete, None);
                    let _ = app.emit("job-updated", "");

                    // Post-print action
                    match config.post_print_action {
                        PostFileAction::MoveToSubfolder => {
                            if let Some(parent) = image_path.parent() {
                                let printed_dir = parent.join("printed");
                                fs::create_dir_all(&printed_dir).ok();
                                if let Some(name) = image_path.file_name() {
                                    fs::rename(image_path, printed_dir.join(name)).ok();
                                }
                            }
                        }
                        PostFileAction::Delete => {
                            fs::remove_file(image_path).ok();
                        }
                        PostFileAction::Keep => {}
                    }
                }
                Err(e) => {
                    eprintln!("Print error for {}: {}", filename, e);
                    job_queue.update_status(job_id, JobStatus::Error, Some(e));
                    let _ = app.emit("job-updated", "");
                }
            }
        } else {
            // Not auto-print — leave as pending
            job_queue.add_job(job);
            let _ = app.emit("job-updated", "");
        }
    } else {
        // No matching preset — mark as skipped
        let mut job = PrintJob::new(
            filename.clone(),
            image_path.display().to_string(),
            None,
            None,
        );
        job.status = JobStatus::Skipped;
        job.error_message = Some(format!(
            "No preset matched{}",
            keyword.map(|k| format!(" for keyword '{}'", k)).unwrap_or_default()
        ));
        job_queue.add_job(job);
        let _ = app.emit("job-updated", "");
    }

    Ok(())
}

fn submit_print_job(file_path: &Path, preset: &Preset) -> Result<(), String> {
    let printer_id = preset
        .printer_id
        .as_ref()
        .ok_or_else(|| "No printer assigned to preset".to_string())?;

    #[cfg(target_os = "windows")]
    {
        submit_print_windows(file_path, printer_id, preset)
    }

    #[cfg(not(target_os = "windows"))]
    {
        submit_print_unix(file_path, printer_id, preset)
    }
}

// ─── Windows printing ────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn submit_print_windows(file_path: &Path, printer_id: &str, preset: &Preset) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    // PowerShell single-quoted strings: only ' needs escaping as ''
    let file_str = file_path.display().to_string().replace('\'', "''");
    let printer_escaped = printer_id.replace('\'', "''");

    let script_content = build_print_script(&printer_escaped, &file_str, preset);

    // Write script to a unique temp file (avoid races with concurrent jobs)
    let script_path = std::env::temp_dir().join(format!(
        "printqueue_{}.ps1",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));
    fs::write(&script_path, &script_content).map_err(|e| format!("Failed to write script: {}", e))?;

    eprintln!("[PrintQueue] Submitting print job: printer={}, file={}", printer_id, file_path.display());

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy", "Bypass",
            "-File",
            &script_path.display().to_string(),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to launch powershell: {}", e))?;

    // Clean up script
    fs::remove_file(&script_path).ok();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    eprintln!("[PrintQueue] PS stdout: {}", stdout.trim());
    if !stderr.is_empty() {
        eprintln!("[PrintQueue] PS stderr: {}", stderr.trim());
    }
    eprintln!("[PrintQueue] PS exit code: {:?}", output.status.code());

    if stdout.contains("SUCCESS") {
        return Ok(());
    }

    // If we got here, something went wrong
    let detail = if !stderr.is_empty() {
        stderr.trim().to_string()
    } else if !stdout.is_empty() {
        stdout.trim().to_string()
    } else {
        format!("Process exited with code {:?}", output.status.code())
    };

    Err(format!("Print failed: {}", detail))
}

#[cfg(target_os = "windows")]
fn build_print_script(printer_name: &str, file_path: &str, preset: &Preset) -> String {
    let copies = preset.copies;

    // Priority: DEVMODE blob > Print Ticket XML > basic PrintDocument
    let settings_block = if let Some(ref b64) = preset.devmode_base64 {
        // DEVMODE-direct path: decode stored blob and apply directly
        format!(
            r#"
    # Apply DEVMODE blob directly (captured from native driver dialog)
    $devModeB64 = '{b64}'
    $devModeBytes = [System.Convert]::FromBase64String($devModeB64)

    Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Drawing.Printing;

public static class DevModeApplier {{
    [DllImport("kernel32.dll")]
    static extern IntPtr GlobalAlloc(uint flags, UIntPtr size);
    [DllImport("kernel32.dll")]
    static extern IntPtr GlobalLock(IntPtr hMem);
    [DllImport("kernel32.dll")]
    static extern bool GlobalUnlock(IntPtr hMem);

    public static void Apply(PrinterSettings ps, PageSettings page, byte[] dm) {{
        IntPtr hGlobal = GlobalAlloc(0x0042, (UIntPtr)dm.Length);
        IntPtr ptr = GlobalLock(hGlobal);
        Marshal.Copy(dm, 0, ptr, dm.Length);
        GlobalUnlock(hGlobal);
        ps.SetHdevmode(hGlobal);
        page.SetHdevmode(hGlobal);
    }}
}}
"@ -ReferencedAssemblies System.Drawing

    [DevModeApplier]::Apply($pd.PrinterSettings, $pd.DefaultPageSettings, $devModeBytes)
    $pd.PrinterSettings.Copies = {copies}
"#,
            b64 = b64,
            copies = copies,
        )
    } else if !preset.settings.is_empty() {
        // Print Ticket XML path (existing fallback)
        let mut settings_entries = String::new();
        for (key, value) in &preset.settings {
            let k = key.replace('\'', "''");
            let v = value.replace('\'', "''");
            settings_entries.push_str(&format!("    $settings['{}'] = '{}'\n", k, v));
        }

        format!(
            r#"
    # Apply custom print settings via Print Ticket
    Add-Type -AssemblyName System.Printing
    Add-Type -AssemblyName ReachFramework

    $settings = @{{}}
{settings_entries}

    $server = New-Object System.Printing.LocalPrintServer
    $queue = $server.GetPrintQueue($printerName)
    $ticket = $queue.DefaultPrintTicket

    # Read current ticket XML
    $xmlStream = $ticket.GetXmlStream()
    $reader = New-Object System.IO.StreamReader($xmlStream)
    $ticketXml = [xml]$reader.ReadToEnd()
    $reader.Close()
    $xmlStream.Close()

    $nsMgr = New-Object System.Xml.XmlNamespaceManager($ticketXml.NameTable)
    $nsMgr.AddNamespace('psf', 'http://schemas.microsoft.com/windows/2003/08/printing/printschemaframework')

    # Register all namespaces from document root
    foreach ($attr in $ticketXml.DocumentElement.Attributes) {{
        if ($attr.Prefix -eq 'xmlns' -and $attr.LocalName -ne 'xmlns') {{
            try {{ $nsMgr.AddNamespace($attr.LocalName, $attr.Value) }} catch {{}}
        }}
    }}

    # Apply each setting to the ticket XML
    foreach ($key in $settings.Keys) {{
        $value = $settings[$key]
        $feature = $ticketXml.SelectSingleNode("//psf:Feature[@name='$key']", $nsMgr)

        if ($feature) {{
            # Remove existing options
            $existingOpts = $feature.SelectNodes('psf:Option', $nsMgr)
            foreach ($o in $existingOpts) {{ [void]$feature.RemoveChild($o) }}

            $newOpt = $ticketXml.CreateElement('psf', 'Option', 'http://schemas.microsoft.com/windows/2003/08/printing/printschemaframework')
            $newOpt.SetAttribute('name', $value)
            [void]$feature.AppendChild($newOpt)
        }} else {{
            # Add new feature
            $newFeature = $ticketXml.CreateElement('psf', 'Feature', 'http://schemas.microsoft.com/windows/2003/08/printing/printschemaframework')
            $newFeature.SetAttribute('name', $key)
            $newOpt = $ticketXml.CreateElement('psf', 'Option', 'http://schemas.microsoft.com/windows/2003/08/printing/printschemaframework')
            $newOpt.SetAttribute('name', $value)
            [void]$newFeature.AppendChild($newOpt)
            [void]$ticketXml.DocumentElement.AppendChild($newFeature)
        }}
    }}

    # Rebuild and validate the ticket
    $memStream = New-Object System.IO.MemoryStream
    $ticketXml.Save($memStream)
    $memStream.Position = 0
    $newTicket = New-Object System.Printing.PrintTicket($memStream)
    $memStream.Close()

    $validation = $queue.MergeAndValidatePrintTicket($queue.DefaultPrintTicket, $newTicket)
    $validatedTicket = $validation.ValidatedPrintTicket
    $validatedTicket.CopyCount = {copies}

    # Convert PrintTicket -> DEVMODE bytes
    Add-Type -AssemblyName System.Printing
    $converter = New-Object System.Printing.Interop.PrintTicketConverter(
        $printerName,
        $queue.ClientPrintSchemaVersion
    )
    $devModeBytes = $converter.ConvertPrintTicketToDevMode(
        $validatedTicket,
        [System.Printing.Interop.BaseDevModeType]::PrinterDefault
    )
    $converter.Dispose()
    $queue.Dispose()
    $server.Dispose()

    # Apply DEVMODE to PrintDocument
    # We need P/Invoke to allocate an HGLOBAL from the byte array
    Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Drawing.Printing;

public static class DevModeApplier {{
    [DllImport("kernel32.dll")]
    static extern IntPtr GlobalAlloc(uint flags, UIntPtr size);
    [DllImport("kernel32.dll")]
    static extern IntPtr GlobalLock(IntPtr hMem);
    [DllImport("kernel32.dll")]
    static extern bool GlobalUnlock(IntPtr hMem);

    public static void Apply(PrinterSettings ps, PageSettings page, byte[] dm) {{
        IntPtr hGlobal = GlobalAlloc(0x0042, (UIntPtr)dm.Length);
        IntPtr ptr = GlobalLock(hGlobal);
        Marshal.Copy(dm, 0, ptr, dm.Length);
        GlobalUnlock(hGlobal);
        ps.SetHdevmode(hGlobal);
        page.SetHdevmode(hGlobal);
    }}
}}
"@ -ReferencedAssemblies System.Drawing

    [DevModeApplier]::Apply($pd.PrinterSettings, $pd.DefaultPageSettings, $devModeBytes)
"#,
            settings_entries = settings_entries,
            copies = copies,
        )
    } else {
        String::new()
    };

    format!(
        r#"$ErrorActionPreference = 'Stop'
try {{
    Add-Type -AssemblyName System.Drawing

    $printerName = '{printer_name}'
    $filePath = '{file_path}'

    $image = [System.Drawing.Image]::FromFile($filePath)

    $pd = New-Object System.Drawing.Printing.PrintDocument
    $pd.PrinterSettings.PrinterName = $printerName
    $pd.PrinterSettings.Copies = {copies}
{settings_block}
    $pd.add_PrintPage({{
        param($sender, $e)
        # Draw the image to fill the entire page (no scaling logic — the
        # paper size and margins are already set by the DEVMODE / Print Ticket)
        $dest = New-Object System.Drawing.RectangleF(0, 0, $e.PageBounds.Width, $e.PageBounds.Height)
        $e.Graphics.DrawImage($image, $dest)
        $e.HasMorePages = $false
    }})

    $pd.Print()
    $image.Dispose()
    $pd.Dispose()

    Write-Output "SUCCESS"
}} catch {{
    Write-Error "PRINT_ERROR: $($_.Exception.GetType().FullName): $($_.Exception.Message)"
    Write-Error "Stack: $($_.ScriptStackTrace)"
    exit 1
}}
"#,
        printer_name = printer_name,
        file_path = file_path,
        copies = copies,
        settings_block = settings_block,
    )
}

// ─── macOS / Linux printing ──────────────────────────────────────────────────

#[cfg(not(target_os = "windows"))]
fn submit_print_unix(file_path: &Path, printer_id: &str, preset: &Preset) -> Result<(), String> {
    use std::process::Command;

    let mut cmd = Command::new("lp");
    cmd.arg("-d").arg(printer_id);
    cmd.arg("-n").arg(preset.copies.to_string());

    // Pass all configured settings as -o key=value
    for (key, value) in &preset.settings {
        cmd.arg("-o").arg(format!("{}={}", key, value));
    }

    cmd.arg(file_path);

    let output = cmd.output().map_err(|e| e.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("lp failed: {}", stderr));
    }

    Ok(())
}

pub fn stop_watcher_inner(watcher_state: &WatcherState) {
    if let Ok(mut w) = watcher_state.watcher.lock() {
        *w = None;
    }
    if let Ok(mut s) = watcher_state.status.lock() {
        *s = WatcherStatus::Idle;
    }
}
