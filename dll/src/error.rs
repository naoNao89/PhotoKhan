#![allow(dead_code)]

use std::fmt;

/// Lỗi trả về cho giao diện theo đúng cấu trúc trong CONTRACT.md.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AppError {
    pub code: String,
    pub message_vi: String,
    pub action_vi: String,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(
        code: impl Into<String>,
        message_vi: impl Into<String>,
        action_vi: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message_vi: message_vi.into(),
            action_vi: action_vi.into(),
        }
    }

    pub fn queue_io() -> Self {
        Self::new(
            "QUEUE_IO",
            "Không đọc hoặc ghi được tệp hàng đợi.",
            "Hãy kiểm tra quyền truy cập thư mục dữ liệu của ứng dụng rồi thử lại.",
        )
    }

    pub fn queue_corrupt() -> Self {
        Self::new(
            "QUEUE_CORRUPT",
            "Tệp hàng đợi bị hỏng nên không thể đọc.",
            "Hãy sao lưu tệp queue.json, xóa tệp đó rồi mở lại ứng dụng.",
        )
    }

    pub fn settings_io() -> Self {
        Self::new(
            "SETTINGS_IO",
            "Không đọc hoặc ghi được tệp cài đặt.",
            "Hãy kiểm tra quyền truy cập thư mục dữ liệu của ứng dụng rồi thử lại.",
        )
    }

    pub fn settings_invalid(message_vi: impl Into<String>) -> Self {
        Self::new(
            "SETTINGS_INVALID",
            message_vi,
            "Hãy kiểm tra lại các giá trị trong phần Cài đặt.",
        )
    }

    pub fn watcher_io() -> Self {
        Self::new(
            "WATCHER_IO",
            "Không theo dõi được thư mục ảnh gốc.",
            "Hãy kiểm tra thư mục còn tồn tại và ứng dụng có quyền đọc thư mục đó.",
        )
    }

    pub fn dialog_error() -> Self {
        Self::new(
            "DIALOG_ERROR",
            "Không mở được hộp thoại chọn thư mục.",
            "Hãy thử lại hoặc chọn thư mục bằng cách nhập đường dẫn trong Cài đặt.",
        )
    }

    pub fn worker_io() -> Self {
        Self::new(
            "WORKER_IO",
            "Không khởi động được luồng xử lý nền.",
            "Hãy đóng bớt ứng dụng đang chạy rồi mở lại PhotoKhan.",
        )
    }

    pub fn input_dir_invalid() -> Self {
        Self::new(
            "E007",
            "Thư mục ảnh gốc không tồn tại hoặc không có quyền truy cập.",
            "Hãy chọn lại thư mục ảnh gốc trong phần Cài đặt.",
        )
    }

    pub fn not_ready() -> Self {
        Self::new(
            "NOT_READY",
            "Chức năng xuất ảnh sẽ có ở bản sau.",
            "Vui lòng chờ bản cập nhật tiếp theo.",
        )
    }
}

/// Mã lỗi M2 (SRS phụ lục B). Chưa có lệnh Tauri nào gọi (D1) nên tạm cho phép
/// dead_code cho tới khi nối pipeline ở milestone sau.
#[allow(dead_code)]
impl AppError {
    pub fn image_unreadable() -> Self {
        Self::new(
            "E001",
            "Không đọc được ảnh. Tệp có thể bị hỏng hoặc không đúng định dạng.",
            "Hãy chọn ảnh khác hoặc chụp lại ảnh mới.",
        )
    }

    pub fn unsupported_format() -> Self {
        Self::new(
            "E001",
            "Định dạng ảnh này chưa được hỗ trợ ở bản hiện tại.",
            "Hãy dùng ảnh JPG, PNG, BMP, TIFF hoặc WebP.",
        )
    }

    pub fn face_not_found() -> Self {
        Self::new(
            "E002",
            "Không tìm thấy khuôn mặt trong ảnh.",
            "Hãy chụp lại với khuôn mặt nhìn thẳng và đủ sáng.",
        )
    }

    pub fn multiple_faces() -> Self {
        Self::new(
            "E003",
            "Ảnh có nhiều người.",
            "Hãy chọn ảnh chỉ có một người.",
        )
    }

    pub fn model_invalid() -> Self {
        Self::new(
            "E004",
            "Thiếu tệp model hoặc mã kiểm tra không khớp.",
            "Hãy sao chép lại thư mục models cạnh ứng dụng rồi thử lại.",
        )
    }

    pub fn model_runtime() -> Self {
        Self::new(
            "E004",
            "Không nạp được model nhận diện khuôn mặt.",
            "Hãy sao chép lại thư mục models cạnh ứng dụng rồi thử lại.",
        )
    }

    pub fn output_write_failed() -> Self {
        Self::new(
            "E005",
            "Không ghi được ảnh ra thư mục đầu ra.",
            "Hãy kiểm tra dung lượng ổ đĩa và quyền ghi thư mục đầu ra.",
        )
    }

    pub fn crop_invalid() -> Self {
        Self::new(
            "E006",
            "Không tính được khung cắt hợp lệ cho ảnh này.",
            "Hãy chọn ảnh khác có khuôn mặt rõ hơn.",
        )
    }

    pub fn preset_unsupported() -> Self {
        Self::new(
            "E006",
            "Loại ảnh thẻ này chưa được hỗ trợ ở bản hiện tại.",
            "Hãy chọn ảnh 2x3, 3x4, 4x6 hoặc 3.5x4.5.",
        )
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message_vi)
    }
}

impl std::error::Error for AppError {}
