use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Printing,
    Complete,
    Error,
    NeedsAttention,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintJob {
    pub id: Uuid,
    pub filename: String,
    pub file_path: String,
    pub preset_id: Option<Uuid>,
    pub preset_name: Option<String>,
    pub status: JobStatus,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl PrintJob {
    pub fn new(
        filename: String,
        file_path: String,
        preset_id: Option<Uuid>,
        preset_name: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            filename,
            file_path,
            preset_id,
            preset_name,
            status: JobStatus::Pending,
            error_message: None,
            created_at: Utc::now(),
            completed_at: None,
        }
    }
}

pub struct JobQueueState {
    pub jobs: Mutex<Vec<PrintJob>>,
}

impl JobQueueState {
    pub fn new() -> Self {
        Self {
            jobs: Mutex::new(Vec::new()),
        }
    }

    pub fn add_job(&self, job: PrintJob) {
        if let Ok(mut jobs) = self.jobs.lock() {
            jobs.push(job);
        }
    }

    pub fn update_status(&self, job_id: Uuid, status: JobStatus, error: Option<String>) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
                job.status = status;
                job.error_message = error;
                if matches!(job.status, JobStatus::Complete | JobStatus::Error) {
                    job.completed_at = Some(Utc::now());
                }
            }
        }
    }
}
