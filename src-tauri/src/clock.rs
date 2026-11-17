use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{AppError, AppResult};

/// Thời điểm hiện tại theo ISO 8601, múi giờ UTC.
pub fn now_iso8601() -> AppResult<String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| time_error())?;
    let total_seconds = i64::try_from(elapsed.as_secs()).map_err(|_| time_error())?;
    let days = total_seconds.div_euclid(86_400);
    let seconds_of_day = total_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;

    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
}

fn time_error() -> AppError {
    AppError::new(
        "SETTINGS_IO",
        "Không xác định được thời gian hiện tại.",
        "Hãy kiểm tra đồng hồ hệ thống rồi thử lại.",
    )
}

/// Chuyển số ngày kể từ 1970-01-01 thành năm, tháng, ngày theo lịch Gregory.
/// Thuật toán lấy từ Howard Hinnant, chỉ dùng số nguyên để tránh sai số.
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let shifted = days_since_epoch + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);

    (year, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::civil_from_days;

    #[test]
    fn converts_epoch_and_leap_day() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(10_957), (2000, 1, 1));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }
}
