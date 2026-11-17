use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

pub const ALLOWED_POLL_INTERVALS: [u64; 4] = [2, 5, 10, 30];
pub const DEFAULT_BG_HEX: &str = "#1E5AA8";
pub const DEFAULT_PRESET: &str = "3x4";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JobStatus {
    Cho,
    DangXuLy,
    Dat,
    CanChupLai,
    Loi,
    DaXuat,
}

impl JobStatus {
    pub fn is_done(self) -> bool {
        matches!(
            self,
            Self::Dat | Self::CanChupLai | Self::Loi | Self::DaXuat
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub file_name: String,
    pub status: JobStatus,
    #[serde(default)]
    pub reason_code: Option<String>,
    #[serde(default)]
    pub reason_text: Option<String>,
    #[serde(default)]
    pub output_path: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueState {
    pub jobs: Vec<Job>,
    pub done: usize,
    pub total: usize,
    pub paused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub input_dir: Option<String>,
    pub output_dir: Option<String>,
    pub default_preset: String,
    pub default_bg_hex: String,
    pub cpu_limit_percent: u8,
    pub poll_interval_secs: u64,
    pub pause_when_busy: bool,
    pub pause_on_battery: bool,
    pub remove_original_after_export: bool,
    pub secure_delete: bool,
    pub local_log_enabled: bool,
    pub minor_consent_at: Option<String>,
    pub minor_consent_version: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            input_dir: None,
            output_dir: None,
            default_preset: DEFAULT_PRESET.to_string(),
            default_bg_hex: DEFAULT_BG_HEX.to_string(),
            cpu_limit_percent: 20,
            poll_interval_secs: 5,
            pause_when_busy: true,
            pause_on_battery: true,
            remove_original_after_export: false,
            secure_delete: false,
            local_log_enabled: true,
            minor_consent_at: None,
            minor_consent_version: None,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> AppResult<()> {
        if !ALLOWED_POLL_INTERVALS.contains(&self.poll_interval_secs) {
            return Err(AppError::settings_invalid(
                "Tần suất quét chỉ được là 2, 5, 10 hoặc 30 giây.",
            ));
        }

        if self.cpu_limit_percent == 0 || self.cpu_limit_percent > 100 {
            return Err(AppError::settings_invalid(
                "Giới hạn CPU phải nằm trong khoảng 1 đến 100 phần trăm.",
            ));
        }

        if self.default_preset.trim().is_empty() {
            return Err(AppError::settings_invalid(
                "Preset mặc định không được để trống.",
            ));
        }

        if !is_hex_color(&self.default_bg_hex) {
            return Err(AppError::settings_invalid("Màu nền phải có dạng #RRGGBB."));
        }

        if let Some(version) = &self.minor_consent_version
            && version.trim().is_empty()
        {
            return Err(AppError::settings_invalid(
                "Phiên bản đồng ý của người chưa thành niên không hợp lệ.",
            ));
        }

        Ok(())
    }
}

fn is_hex_color(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::{Job, JobStatus, Settings};

    #[test]
    fn job_status_uses_contract_names() {
        let expected = [
            (JobStatus::Cho, "\"CHO\""),
            (JobStatus::DangXuLy, "\"DANG_XU_LY\""),
            (JobStatus::Dat, "\"DAT\""),
            (JobStatus::CanChupLai, "\"CAN_CHUP_LAI\""),
            (JobStatus::Loi, "\"LOI\""),
            (JobStatus::DaXuat, "\"DA_XUAT\""),
        ];

        for (status, json) in expected {
            assert_eq!(serde_json::to_string(&status).unwrap(), json);
        }
    }

    #[test]
    fn default_settings_match_m1_contract() {
        let settings = Settings::default();
        assert_eq!(settings.default_preset, "3x4");
        assert_eq!(settings.default_bg_hex, "#1E5AA8");
        assert_eq!(settings.cpu_limit_percent, 20);
        assert_eq!(settings.poll_interval_secs, 5);
        assert!(settings.pause_when_busy);
        assert!(settings.pause_on_battery);
        assert!(!settings.remove_original_after_export);
        assert!(!settings.secure_delete);
        assert!(settings.local_log_enabled);
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn rejects_unsupported_poll_interval() {
        let settings = Settings {
            poll_interval_secs: 3,
            ..Settings::default()
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn job_serializes_only_contract_fields() {
        let job = Job {
            id: "job-1".to_string(),
            file_name: "anh.jpg".to_string(),
            status: JobStatus::Cho,
            reason_code: None,
            reason_text: None,
            output_path: None,
            updated_at: "2026-11-15T00:00:00Z".to_string(),
        };
        let value = serde_json::to_value(&job).unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(object.len(), 7);
        assert!(object.contains_key("file_name"));
        assert!(!object.contains_key("source_path"));
        assert!(!object.contains_key("sha256"));
    }
}
