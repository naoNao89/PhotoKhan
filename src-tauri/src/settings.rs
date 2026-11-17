use std::path::PathBuf;

use crate::clock;
use crate::error::{AppError, AppResult};
use crate::model::Settings;
use crate::persistence::{atomic_write_json, load_json};

/// Cài đặt bền vững. Nếu tệp hỏng, lỗi được giữ lại để trả cho giao diện
/// thay vì làm ứng dụng dừng hoặc ghi đè dữ liệu hỏng.
pub struct SettingsStore {
    path: PathBuf,
    settings: Settings,
    load_error: Option<AppError>,
}

impl SettingsStore {
    pub fn load(path: PathBuf) -> Self {
        let mut store = Self {
            path,
            settings: Settings::default(),
            load_error: None,
        };

        let corrupt_error = AppError::new(
            "SETTINGS_INVALID",
            "Tệp cài đặt bị hỏng nên không thể đọc.",
            "Hãy sao lưu tệp settings.json, xóa tệp đó rồi mở lại ứng dụng.",
        );

        match load_json::<Settings>(&store.path, "SETTINGS_IO", corrupt_error, "settings.json") {
            Ok(Some(settings)) => {
                if let Err(error) = settings.validate() {
                    store.load_error = Some(error);
                } else {
                    store.settings = settings;
                }
            }
            Ok(None) => {}
            Err(error) => store.load_error = Some(error),
        }

        store
    }

    pub fn get(&self) -> AppResult<Settings> {
        self.ensure_available()?;
        Ok(self.settings.clone())
    }

    pub fn set(&mut self, settings: Settings) -> AppResult<()> {
        settings.validate()?;
        self.save(&settings)?;
        self.settings = settings;
        self.load_error = None;
        Ok(())
    }

    pub fn set_consent(&mut self, version: &str) -> AppResult<()> {
        self.ensure_available()?;
        if version.trim().is_empty() {
            return Err(AppError::settings_invalid(
                "Phiên bản đồng ý của người chưa thành niên không được để trống.",
            ));
        }

        let mut updated = self.settings.clone();
        updated.minor_consent_at = Some(clock::now_iso8601()?);
        updated.minor_consent_version = Some(version.to_string());
        updated.validate()?;
        self.save(&updated)?;
        self.settings = updated;
        Ok(())
    }

    fn ensure_available(&self) -> AppResult<()> {
        match &self.load_error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    fn save(&self, settings: &Settings) -> AppResult<()> {
        atomic_write_json(&self.path, settings, "SETTINGS_IO", "settings.json")
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::SettingsStore;

    fn unique_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "photokhan-test-{name}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn saves_and_reloads_settings_with_a_new_store_object() {
        let dir = unique_dir("settings-round-trip");
        let path = dir.join("settings.json");
        let mut store = SettingsStore::load(path.clone());
        let mut settings = store.get().unwrap();
        settings.cpu_limit_percent = 40;
        settings.poll_interval_secs = 10;
        settings.input_dir = Some("C:\\anh".to_string());
        store.set(settings.clone()).unwrap();

        let reloaded = SettingsStore::load(path);
        assert_eq!(reloaded.get().unwrap(), settings);
    }

    #[test]
    fn consent_stores_time_and_version() {
        let dir = unique_dir("settings-consent");
        let path = dir.join("settings.json");
        let mut store = SettingsStore::load(path.clone());
        store.set_consent("1.0").unwrap();

        let saved = store.get().unwrap();
        assert_eq!(saved.minor_consent_version.as_deref(), Some("1.0"));
        let timestamp = saved.minor_consent_at.unwrap();
        assert!(timestamp.ends_with('Z'));
        assert_eq!(timestamp.len(), 20);

        let reloaded = SettingsStore::load(path);
        let saved_again = reloaded.get().unwrap();
        assert_eq!(saved_again.minor_consent_version.as_deref(), Some("1.0"));
        assert_eq!(saved_again.minor_consent_at, Some(timestamp));
    }

    #[test]
    fn corrupt_settings_return_an_error() {
        let dir = unique_dir("settings-corrupt");
        let path = dir.join("settings.json");
        fs::write(&path, b"[broken").unwrap();

        let store = SettingsStore::load(path);
        assert_eq!(store.get().unwrap_err().code, "SETTINGS_INVALID");
    }

    #[test]
    fn invalid_interval_is_not_written() {
        let dir = unique_dir("settings-invalid");
        let path = dir.join("settings.json");
        let mut store = SettingsStore::load(path.clone());
        let mut settings = store.get().unwrap();
        settings.poll_interval_secs = 7;
        assert_eq!(store.set(settings).unwrap_err().code, "SETTINGS_INVALID");
        assert!(!path.exists());
    }
}
