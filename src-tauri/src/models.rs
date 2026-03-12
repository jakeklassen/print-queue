use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub watch_folder: Option<String>,
    pub minimize_to_tray: bool,
    pub default_preset_id: Option<Uuid>,
    pub post_print_action: PostFileAction,
    pub post_zip_action: PostFileAction,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            watch_folder: None,
            minimize_to_tray: true,
            default_preset_id: None,
            post_print_action: PostFileAction::MoveToSubfolder,
            post_zip_action: PostFileAction::Delete,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PostFileAction {
    Delete,
    MoveToSubfolder,
    Keep,
}

/// Maps printer option key → selected value (e.g. "PageSize" → "4x6.Fullbleed").
pub type PrintSettings = HashMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: Uuid,
    pub name: String,
    pub printer_id: Option<String>,
    pub paper_size_keyword: String,
    pub settings: PrintSettings,
    pub copies: u32,
    pub auto_print: bool,
    pub scale_compensation: f64,
    #[serde(default)]
    pub devmode_base64: Option<String>,
    #[serde(default)]
    pub macos_print_info_base64: Option<String>,
    #[serde(default)]
    pub macos_page_format_base64: Option<String>,
    #[serde(default)]
    pub macos_print_settings_base64: Option<String>,
    #[serde(default)]
    pub macos_printer_name: Option<String>,
    #[serde(default)]
    pub macos_page_width_points: Option<f64>,
    #[serde(default)]
    pub macos_page_height_points: Option<f64>,
    #[serde(default)]
    pub macos_size_compensation_mm: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Preset {
    pub fn new(name: String, paper_size_keyword: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            printer_id: None,
            paper_size_keyword,
            settings: HashMap::new(),
            copies: 1,
            auto_print: true,
            scale_compensation: 1.0,
            devmode_base64: None,
            macos_print_info_base64: None,
            macos_page_format_base64: None,
            macos_print_settings_base64: None,
            macos_printer_name: None,
            macos_page_width_points: None,
            macos_page_height_points: None,
            macos_size_compensation_mm: None,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedFile {
    pub filename: String,
    pub hash: String,
    pub processed_at: DateTime<Utc>,
    pub job_count: u32,
    pub preset_id: Option<Uuid>,
}
