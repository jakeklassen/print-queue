use std::sync::Arc;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::jobs::{JobQueueState, JobStatus, PrintJob};
use crate::models::{AppConfig, Preset};
use crate::printing::{self, PrinterCapabilities, PrinterInfo};
use crate::storage::StorageState;
use crate::watcher::{self, WatcherState, WatcherStatus};

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
