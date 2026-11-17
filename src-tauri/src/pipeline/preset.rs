use crate::error::{AppError, AppResult};

/// Thông số một preset ảnh thẻ (SRS mục 4, đơn vị pixel tại 300 dpi).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Preset {
    pub id: &'static str,
    pub width: u32,
    pub height: u32,
    /// Tỷ lệ chiều cao mặt trên chiều cao ảnh (SRS mục 4).
    pub face_ratio: f32,
    /// Tâm hai mắt tính từ đáy ảnh (SRS mục 4).
    pub eye_line_from_bottom: f32,
}

/// M2 chỉ hỗ trợ các preset có tỷ lệ mặt 70–80%. Preset 5x5 hoãn sang M4.
pub const PRESETS: [Preset; 4] = [
    Preset {
        id: "2x3",
        width: 236,
        height: 354,
        face_ratio: 0.75,
        eye_line_from_bottom: 0.55,
    },
    Preset {
        id: "3x4",
        width: 354,
        height: 472,
        face_ratio: 0.75,
        eye_line_from_bottom: 0.55,
    },
    Preset {
        id: "4x6",
        width: 472,
        height: 709,
        face_ratio: 0.75,
        eye_line_from_bottom: 0.55,
    },
    Preset {
        id: "3.5x4.5",
        width: 413,
        height: 531,
        face_ratio: 0.75,
        eye_line_from_bottom: 0.55,
    },
];

pub const DEFAULT_PRESET_ID: &str = "3x4";

pub fn by_id(id: &str) -> AppResult<Preset> {
    let wanted = id.trim();
    PRESETS
        .iter()
        .find(|preset| preset.id.eq_ignore_ascii_case(wanted))
        .copied()
        .ok_or_else(AppError::preset_unsupported)
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_PRESET_ID, PRESETS, by_id};

    #[test]
    fn preset_pixels_match_srs_table() {
        let expected = [
            ("2x3", 236, 354),
            ("3x4", 354, 472),
            ("4x6", 472, 709),
            ("3.5x4.5", 413, 531),
        ];

        assert_eq!(PRESETS.len(), expected.len());
        for (id, width, height) in expected {
            let preset = by_id(id).unwrap();
            assert_eq!((preset.width, preset.height), (width, height));
            assert_eq!(preset.face_ratio, 0.75);
            assert_eq!(preset.eye_line_from_bottom, 0.55);
        }
        assert_eq!(by_id(DEFAULT_PRESET_ID).unwrap().id, "3x4");
    }

    #[test]
    fn unsupported_preset_returns_an_error() {
        assert_eq!(by_id("5x5").unwrap_err().code, "E006");
        assert_eq!(by_id("").unwrap_err().code, "E006");
    }

    #[test]
    fn preset_lookup_is_case_insensitive_and_trims() {
        assert_eq!(by_id(" 3X4 ").unwrap().id, "3x4");
    }
}
