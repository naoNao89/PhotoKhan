/* ==========================================================================
   PhotoKhan – logic giao diện
   Gọi backend Tauri đúng CONTRACT.md; khi chạy trực tiếp trong trình duyệt
   sẽ dùng dữ liệu mô phỏng, không gọi mạng, không dùng API key.
   ========================================================================== */

(() => {
  "use strict";

  const APP_VERSION = "0.1.0";
  const CONSENT_VERSION = "1.0";
  const STORAGE_SETTINGS = "photokhan.mock.settings.v1";
  const STORAGE_UI = "photokhan.ui.v1";
  const STORAGE_HISTORY = "photokhan.history.v1";
  const DEMO_FOLDER_IN = "C:\\Users\\Public\\Documents\\AnhGoc";
  const DEMO_FOLDER_OUT = "C:\\Users\\Public\\Documents\\AnhThe";

  const hasTauri = Boolean(
    window.__TAURI__ &&
      window.__TAURI__.core &&
      typeof window.__TAURI__.core.invoke === "function"
  );

  const el = {};
  for (const node of document.querySelectorAll("[id]")) {
    el[node.id] = node;
  }

  /* ------------------------------------------------------------------------
     Preset, màu nền, mẫu áo
     ------------------------------------------------------------------------ */

  const PRESET_ORDER = ["2x3", "3x4", "4x6", "3.5x4.5", "5x5", "tuy-chinh"];

  const PRESET_DEFS = {
    "2x3": {
      id: "2x3",
      name: "Ảnh 2×3 cm",
      size: "2 × 3 cm",
      px: [236, 354],
      bg: "#FFFFFF",
      face: "70–80%",
      desc: "Đơn từ, giấy hành chính, hồ sơ đơn giản.",
      note: "Ảnh 2×3 cm · 236 × 354 px · nền trắng. Mặt chiếm 70–80% chiều cao ảnh.",
    },
    "3x4": {
      id: "3x4",
      name: "Ảnh 3×4 cm",
      size: "3 × 4 cm",
      px: [354, 472],
      bg: "#1E5AA8",
      face: "70–80%",
      desc: "Học sinh, sinh viên, bằng lái xe (nền xanh).",
      note: "Ảnh 3×4 cm · 354 × 472 px · nền xanh dương. Mặt chiếm 70–80% chiều cao ảnh.",
    },
    "4x6": {
      id: "4x6",
      name: "Ảnh 4×6 cm",
      size: "4 × 6 cm",
      px: [472, 709],
      bg: "#FFFFFF",
      face: "70–80%",
      desc: "CCCD cũ, hồ sơ, thi THPT/ĐGNL (JPG ≤ 5 MB).",
      note: "Ảnh 4×6 cm · 472 × 709 px · nền trắng. Phù hợp hồ sơ và thi THPT/ĐGNL.",
    },
    "3.5x4.5": {
      id: "3.5x4.5",
      name: "Ảnh 3,5×4,5 cm",
      size: "3,5 × 4,5 cm",
      px: [413, 531],
      bg: "#FFFFFF",
      face: "70–80%",
      desc: "Thị thực Schengen, hồ sơ xuất cảnh.",
      note: "Ảnh 3,5×4,5 cm · 413 × 531 px · nền trắng. Mặt chiếm 70–80% chiều cao ảnh.",
    },
    "5x5": {
      id: "5x5",
      name: "Ảnh 5×5 cm",
      size: "5 × 5 cm",
      px: [591, 591],
      bg: "#FFFFFF",
      face: "50–69%",
      desc: "Visa Mỹ, Canada; tâm mắt cách đáy 28–35 mm.",
      note: "Ảnh 5×5 cm · 591 × 591 px · nền trắng. Mặt chiếm 50–69%, tâm mắt cách đáy 28–35 mm.",
    },
    "tuy-chinh": {
      id: "tuy-chinh",
      name: "Tùy chỉnh",
      size: "Tự nhập mm",
      px: [413, 531],
      bg: "#FFFFFF",
      face: "50–85%",
      desc: "Dành cho mẫu chưa có trong danh sách.",
      note: "Kích thước tùy chỉnh, độ phân giải 300 dpi. Hãy đối chiếu quy định của nơi nhận hồ sơ.",
    },
  };

  const BG_COLORS = [
    { name: "Xanh dương đậm", hex: "#1E5AA8" },
    { name: "Xanh nhạt", hex: "#C9E2F7" },
    { name: "Trắng", hex: "#FFFFFF" },
    { name: "Xám", hex: "#D9DEE7" },
  ];

  const OUTFITS = [
    { id: "ao-trang", name: "Áo trắng học sinh" },
    { id: "so-mi", name: "Áo sơ mi công sở" },
    { id: "vest", name: "Áo vest" },
  ];

  const JOB_STATUS = {
    CHO: { label: "Chờ xử lý", tone: "wait" },
    DANG_XU_LY: { label: "Đang xử lý", tone: "busy" },
    DAT: { label: "Đạt", tone: "ok" },
    CAN_CHUP_LAI: { label: "Cần chụp lại", tone: "warn" },
    LOI: { label: "Lỗi", tone: "error" },
    DA_XUAT: { label: "Đã xuất", tone: "done" },
  };

  const DEFAULT_SETTINGS = {
    input_dir: null,
    output_dir: null,
    default_preset: "3x4",
    default_bg_hex: "#1E5AA8",
    cpu_limit_percent: 20,
    poll_interval_secs: 5,
    pause_when_busy: true,
    pause_on_battery: true,
    remove_original_after_export: false,
    secure_delete: false,
    local_log_enabled: true,
    minor_consent_at: null,
    minor_consent_version: null,
  };

  const DEFAULT_UI = {
    export_format: "jpg",
    name_template: "{tên_gốc}_{preset}_{màu}",
    start_with_windows: false,
    minimize_to_tray: true,
    thread_limit: 0,
    matte_accuracy: 60,
    align_head: true,
    eye_level_percent: 55,
    custom_width_mm: 35,
    custom_height_mm: 45,
    face_ratio: 75,
    custom_bg_hex: "#1E5AA8",
    outfit_template_name: null,
    outfit_enabled: true,
    selected_outfit: "so-mi",
    wizard_done: false,
  };

  const state = {
    settings: { ...DEFAULT_SETTINGS },
    ui: { ...DEFAULT_UI },
    queue: { jobs: [], done: 0, total: 0, paused: false },
    history: [],
    selectedJobId: null,
    view: "home",
    jobImages: new Map(),
    crops: new Map(),
    preview: {
      jobId: null,
      url: null,
      imgLoaded: false,
      naturalWidth: 0,
      naturalHeight: 0,
      matte: true,
      outfit: "so-mi",
      drawing: false,
    },
    mock: {
      running: false,
      timer: null,
      demo: false,
      durations: [],
      startedAt: 0,
      counter: 0,
    },
    wizard: null,
  };

  const prefersReducedMotion =
    window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  /* ------------------------------------------------------------------------
     Hàm tiện ích
     ------------------------------------------------------------------------ */

  function qs(selector, root = document) {
    return root.querySelector(selector);
  }

  function qsa(selector, root = document) {
    return Array.from(root.querySelectorAll(selector));
  }

  function nowIso() {
    return new Date().toISOString();
  }

  function clone(value) {
    return JSON.parse(JSON.stringify(value));
  }

  function escapeHtml(value) {
    return String(value == null ? "" : value)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#039;");
  }

  function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
  }

  function loadJson(key, fallback) {
    try {
      const raw = window.localStorage.getItem(key);
      if (!raw) return clone(fallback);
      return JSON.parse(raw);
    } catch (error) {
      return clone(fallback);
    }
  }

  function saveJson(key, value) {
    try {
      window.localStorage.setItem(key, JSON.stringify(value));
    } catch (error) {
      /* Bộ nhớ cục bộ có thể bị chặn; giao diện vẫn chạy bình thường. */
    }
  }

  function normalizeSettings(raw) {
    const result = { ...DEFAULT_SETTINGS };
    if (!raw || typeof raw !== "object") return result;

    for (const key of Object.keys(DEFAULT_SETTINGS)) {
      if (!Object.prototype.hasOwnProperty.call(raw, key)) continue;
      const value = raw[key];
      if (value === null) {
        result[key] = null;
        continue;
      }
      if (typeof DEFAULT_SETTINGS[key] === "boolean") {
        result[key] = Boolean(value);
      } else if (typeof DEFAULT_SETTINGS[key] === "number") {
        const number = Number(value);
        if (Number.isFinite(number)) result[key] = number;
      } else if (typeof value === "string") {
        result[key] = value;
      }
    }

    if (!PRESET_DEFS[result.default_preset]) result.default_preset = "3x4";
    result.default_bg_hex = normalizeHex(result.default_bg_hex) || "#1E5AA8";
    result.cpu_limit_percent = clamp(Number(result.cpu_limit_percent) || 20, 5, 60);
    if (![2, 5, 10, 30].includes(Number(result.poll_interval_secs))) {
      result.poll_interval_secs = 5;
    }
    return result;
  }

  function normalizeUi(raw) {
    const result = { ...DEFAULT_UI };
    if (!raw || typeof raw !== "object") return result;

    for (const key of Object.keys(DEFAULT_UI)) {
      if (!Object.prototype.hasOwnProperty.call(raw, key)) continue;
      const value = raw[key];
      if (value === null) {
        result[key] = null;
      } else if (typeof DEFAULT_UI[key] === "boolean") {
        result[key] = Boolean(value);
      } else if (typeof DEFAULT_UI[key] === "number") {
        const number = Number(value);
        if (Number.isFinite(number)) result[key] = number;
      } else if (typeof value === "string") {
        result[key] = value;
      }
    }

    result.export_format = ["jpg", "png", "pdf"].includes(result.export_format)
      ? result.export_format
      : "jpg";
    result.selected_outfit = OUTFITS.some((item) => item.id === result.selected_outfit)
      ? result.selected_outfit
      : "so-mi";
    result.custom_width_mm = clamp(Number(result.custom_width_mm) || 35, 10, 150);
    result.custom_height_mm = clamp(Number(result.custom_height_mm) || 45, 10, 200);
    result.face_ratio = clamp(Number(result.face_ratio) || 75, 50, 85);
    result.matte_accuracy = clamp(Number(result.matte_accuracy) || 60, 0, 100);
    result.eye_level_percent = clamp(Number(result.eye_level_percent) || 55, 50, 65);
    result.thread_limit = clamp(Number(result.thread_limit) || 0, 0, 8);
    result.custom_bg_hex = normalizeHex(result.custom_bg_hex) || "#1E5AA8";
    if (!result.name_template) result.name_template = DEFAULT_UI.name_template;
    return result;
  }

  function normalizeHex(value) {
    if (typeof value !== "string") return null;
    let hex = value.trim().toUpperCase();
    if (!hex.startsWith("#")) hex = "#" + hex;
    if (/^#[0-9A-F]{3}$/.test(hex)) {
      hex = "#" + hex[1] + hex[1] + hex[2] + hex[2] + hex[3] + hex[3];
    }
    return /^#[0-9A-F]{6}$/.test(hex) ? hex : null;
  }

  function formatNumber(value) {
    return new Intl.NumberFormat("vi-VN").format(value);
  }

  function formatDateTime(iso) {
    const date = new Date(iso);
    if (Number.isNaN(date.getTime())) return "Không rõ";
    try {
      return new Intl.DateTimeFormat("vi-VN", {
        day: "2-digit",
        month: "2-digit",
        year: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      }).format(date);
    } catch (error) {
      return date.toLocaleString();
    }
  }

  function formatDuration(seconds) {
    if (!Number.isFinite(seconds) || seconds <= 0) return "—";
    const total = Math.round(seconds);
    const minutes = Math.floor(total / 60);
    const rest = total % 60;
    if (minutes <= 0) return `${rest} giây`;
    return `${minutes} phút ${String(rest).padStart(2, "0")} giây`;
  }

  function baseName(path) {
    if (!path) return "";
    const parts = String(path).split(/[\\/]/);
    return parts[parts.length - 1] || String(path);
  }

  function getCustomPreset() {
    const widthMm = clamp(Number(state.ui.custom_width_mm) || 35, 10, 150);
    const heightMm = clamp(Number(state.ui.custom_height_mm) || 45, 10, 200);
    const widthPx = Math.round((widthMm / 25.4) * 300);
    const heightPx = Math.round((heightMm / 25.4) * 300);
    return {
      ...PRESET_DEFS["tuy-chinh"],
      name: `Tùy chỉnh ${widthMm}×${heightMm} mm`,
      size: `${widthMm} × ${heightMm} mm`,
      px: [widthPx, heightPx],
      note: `Tùy chỉnh ${widthMm}×${heightMm} mm · ${widthPx} × ${heightPx} px · độ phân giải 300 dpi.`,
    };
  }

  function activePreset() {
    if (state.settings.default_preset === "tuy-chinh") return getCustomPreset();
    return PRESET_DEFS[state.settings.default_preset] || PRESET_DEFS["3x4"];
  }

  function presetAspect() {
    const preset = activePreset();
    return preset.px[0] / preset.px[1];
  }

  function currentOutfit() {
    return state.ui.outfit_enabled ? state.ui.selected_outfit : null;
  }

  function outfitName(id) {
    const found = OUTFITS.find((item) => item.id === id);
    return found ? found.name : "Không ghép áo";
  }

  function presetOptions() {
    return PRESET_ORDER.map((id) => {
      const preset = id === "tuy-chinh" ? getCustomPreset() : PRESET_DEFS[id];
      return {
        id,
        name: preset.name,
        size: preset.size,
        px: preset.px,
        face: preset.face,
        desc: preset.desc,
        bg: preset.bg,
      };
    });
  }

  /* ------------------------------------------------------------------------
     Thông báo & trạng thái
     ------------------------------------------------------------------------ */

  function toast(message, options = {}) {
    const tone = options.tone || "info";
    const title =
      options.title ||
      (tone === "success"
        ? "Thành công"
        : tone === "warn"
          ? "Cần chú ý"
          : tone === "error"
            ? "Có lỗi"
            : "Thông báo");

    const iconId =
      tone === "success"
        ? "i-check"
        : tone === "warn"
          ? "i-alert"
          : tone === "error"
            ? "i-alert"
            : "i-info";

    const node = document.createElement("div");
    node.className = `toast ${tone}`;
    node.innerHTML = `
      <span class="toast-icon"><svg class="icon" viewBox="0 0 24 24"><use href="#${iconId}"></use></svg></span>
      <div class="toast-body">
        <span class="toast-title">${escapeHtml(title)}</span>
        <p class="toast-message">${escapeHtml(message)}</p>
      </div>
      <button class="toast-close" type="button" aria-label="Đóng thông báo">
        <svg class="icon" viewBox="0 0 24 24"><use href="#i-close"></use></svg>
      </button>
    `;

    const remove = () => {
      if (!node.isConnected || node.classList.contains("is-leaving")) return;
      node.classList.add("is-leaving");
      window.setTimeout(() => node.remove(), 220);
    };

    qs(".toast-close", node).addEventListener("click", remove);
    el.toastArea.appendChild(node);
    window.setTimeout(remove, Number(options.timeout) || 4800);
  }

  function showAppError(error, fallbackMessage) {
    if (error && typeof error === "object" && (error.message_vi || error.code)) {
      const title = error.code ? `Lỗi ${error.code}` : "Có lỗi xảy ra";
      const parts = [error.message_vi || fallbackMessage];
      if (error.action_vi) parts.push(`Gợi ý: ${error.action_vi}`);
      toast(parts.join(" "), { tone: "error", title, timeout: 7200 });
      return;
    }
    const message =
      (error && error.message) || (typeof error === "string" ? error : "") || fallbackMessage;
    toast(String(message), { tone: "error", title: "Có lỗi xảy ra", timeout: 6500 });
  }

  function setStatus(text, tone = "ready") {
    el.statusText.textContent = text;
    el.bottomStatusText.textContent = text;
    el.statusPill.dataset.tone = tone;
    const bottom = el.bottomStatusText.closest(".bottom-status");
    if (bottom) bottom.dataset.tone = tone;
  }

  function announceTitle(title, subtitle) {
    el.viewTitle.textContent = title;
    el.viewSubtitle.textContent = subtitle;
  }

  /* ------------------------------------------------------------------------
     Gọi backend (Tauri) và mô phỏng
     ------------------------------------------------------------------------ */

  async function invokeCommand(command, args) {
    if (!hasTauri) throw new Error("Backend Tauri chưa sẵn sàng.");
    return window.__TAURI__.core.invoke(command, args);
  }

  function promptFolder(kind, current) {
    const label = kind === "input" ? "thư mục ảnh gốc" : "thư mục lưu ảnh";
    const fallback =
      current ||
      (kind === "input"
        ? state.settings.input_dir || DEMO_FOLDER_IN
        : state.settings.output_dir || DEMO_FOLDER_OUT);
    const value = window.prompt(
      `Chế độ mô phỏng: nhập đường dẫn ${label} (không có dữ liệu nào được gửi đi).`,
      fallback
    );
    if (value === null) return null;
    const trimmed = value.trim();
    return trimmed || null;
  }

  async function chooseFolder(kind) {
    if (hasTauri) {
      const command = kind === "input" ? "choose_input_folder" : "choose_output_folder";
      try {
        const path = await invokeCommand(command);
        return typeof path === "string" && path.trim() ? path : null;
      } catch (error) {
        showAppError(error, "Không mở được hộp thoại chọn thư mục.");
        return null;
      }
    }
    return promptFolder(kind);
  }

  async function loadSettingsFromBackend() {
    if (!hasTauri) {
      state.settings = normalizeSettings(loadJson(STORAGE_SETTINGS, DEFAULT_SETTINGS));
      state.ui = normalizeUi(loadJson(STORAGE_UI, DEFAULT_UI));
      state.history = loadJson(STORAGE_HISTORY, []);
      return;
    }
    try {
      const raw = await invokeCommand("get_settings");
      state.settings = normalizeSettings(raw);
      state.ui = normalizeUi(loadJson(STORAGE_UI, DEFAULT_UI));
      state.history = loadJson(STORAGE_HISTORY, []);
    } catch (error) {
      showAppError(error, "Không đọc được cài đặt từ backend.");
      state.settings = normalizeSettings(null);
    }
  }

  let settingsSaveTimer = null;

  async function saveSettingsToBackend(options = {}) {
    if (!hasTauri) {
      saveJson(STORAGE_SETTINGS, state.settings);
      return true;
    }
    try {
      await invokeCommand("set_settings", { settings: clone(state.settings) });
      return true;
    } catch (error) {
      if (!options.quiet) showAppError(error, "Không lưu được cài đặt.");
      return false;
    }
  }

  function scheduleSettingsSave() {
    window.clearTimeout(settingsSaveTimer);
    settingsSaveTimer = window.setTimeout(() => {
      saveSettingsToBackend({ quiet: true });
      saveJson(STORAGE_UI, state.ui);
    }, 420);
  }

  function persistUi() {
    saveJson(STORAGE_UI, state.ui);
  }

  function persistHistory() {
    saveJson(STORAGE_HISTORY, state.history);
  }

  /* ------------------------------------------------------------------------
     Dữ liệu mẫu (chỉ dùng khi chưa có backend)
     ------------------------------------------------------------------------ */

  function makeMockPhoto(seed) {
    const canvas = document.createElement("canvas");
    canvas.width = 720;
    canvas.height = 960;
    const ctx = canvas.getContext("2d");
    if (!ctx) return "";

    const hue = 195 + ((seed * 41) % 95);
    const wall = ctx.createLinearGradient(0, 0, 420, 960);
    wall.addColorStop(0, `hsl(${hue} 38% 90%)`);
    wall.addColorStop(0.55, `hsl(${hue} 28% 80%)`);
    wall.addColorStop(1, `hsl(${hue + 12} 24% 68%)`);
    ctx.fillStyle = wall;
    ctx.fillRect(0, 0, 720, 960);

    ctx.fillStyle = "rgba(255,255,255,0.42)";
    ctx.beginPath();
    ctx.arc(120, 120, 190, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = "rgba(8, 21, 46, 0.08)";
    ctx.fillRect(0, 760, 720, 200);

    const shirtColors = [
      "#1E5AA8",
      "#334155",
      "#0F766E",
      "#7C2D12",
      "#475569",
      "#1F2937",
      "#9A3412",
      "#365314",
    ];
    const skinColors = ["#F2C6A0", "#E8B98F", "#D9A377", "#C98B62"];
    const hairColors = ["#2B2B2B", "#3B2A1E", "#1F2937", "#5B3A29"];

    const shirt = shirtColors[seed % shirtColors.length];
    const skin = skinColors[seed % skinColors.length];
    const hair = hairColors[seed % hairColors.length];

    // Vai và thân
    ctx.fillStyle = shirt;
    ctx.beginPath();
    ctx.moveTo(150, 960);
    ctx.bezierCurveTo(170, 735, 265, 650, 360, 650);
    ctx.bezierCurveTo(455, 650, 550, 735, 570, 960);
    ctx.closePath();
    ctx.fill();

    // Cổ áo
    ctx.fillStyle = "rgba(255,255,255,0.9)";
    ctx.beginPath();
    ctx.moveTo(310, 665);
    ctx.lineTo(360, 735);
    ctx.lineTo(410, 665);
    ctx.lineTo(385, 650);
    ctx.lineTo(360, 690);
    ctx.lineTo(335, 650);
    ctx.closePath();
    ctx.fill();

    // Cà vạt nhỏ
    ctx.fillStyle = "rgba(15,23,42,0.22)";
    ctx.beginPath();
    ctx.moveTo(356, 700);
    ctx.lineTo(364, 700);
    ctx.lineTo(368, 795);
    ctx.lineTo(360, 820);
    ctx.lineTo(352, 795);
    ctx.closePath();
    ctx.fill();

    // Cổ
    ctx.fillStyle = skin;
    ctx.fillRect(325, 570, 70, 110);
    ctx.fillStyle = "rgba(0,0,0,0.07)";
    ctx.fillRect(325, 645, 70, 35);

    // Tai
    ctx.fillStyle = skin;
    ctx.beginPath();
    ctx.ellipse(245, 470, 24, 40, 0, 0, Math.PI * 2);
    ctx.ellipse(475, 470, 24, 40, 0, 0, Math.PI * 2);
    ctx.fill();

    // Đầu
    ctx.beginPath();
    ctx.ellipse(360, 455, 112, 138, 0, 0, Math.PI * 2);
    ctx.fill();

    // Tóc
    ctx.fillStyle = hair;
    ctx.beginPath();
    ctx.ellipse(360, 355, 118, 84, 0, Math.PI, Math.PI * 2);
    ctx.lineTo(478, 420);
    ctx.bezierCurveTo(430, 385, 290, 385, 242, 420);
    ctx.closePath();
    ctx.fill();

    // Lông mày
    ctx.strokeStyle = hair;
    ctx.lineWidth = 6;
    ctx.lineCap = "round";
    ctx.beginPath();
    ctx.moveTo(298, 414);
    ctx.quadraticCurveTo(320, 403, 340, 412);
    ctx.moveTo(380, 412);
    ctx.quadraticCurveTo(400, 403, 422, 414);
    ctx.stroke();

    // Mắt
    ctx.fillStyle = "#243244";
    ctx.beginPath();
    ctx.ellipse(320, 448, 8, 10, 0, 0, Math.PI * 2);
    ctx.ellipse(400, 448, 8, 10, 0, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = "rgba(255,255,255,0.75)";
    ctx.beginPath();
    ctx.arc(323, 444, 2.6, 0, Math.PI * 2);
    ctx.arc(403, 444, 2.6, 0, Math.PI * 2);
    ctx.fill();

    // Mũi
    ctx.strokeStyle = "rgba(120,70,40,0.38)";
    ctx.lineWidth = 4;
    ctx.beginPath();
    ctx.moveTo(360, 458);
    ctx.quadraticCurveTo(354, 500, 366, 512);
    ctx.stroke();

    // Miệng
    ctx.strokeStyle = "#A4523D";
    ctx.lineWidth = 5;
    ctx.beginPath();
    ctx.moveTo(326, 540);
    ctx.quadraticCurveTo(360, 560, 394, 540);
    ctx.stroke();

    // Nhãn ảnh mẫu
    ctx.fillStyle = "rgba(8, 21, 46, 0.58)";
    ctx.beginPath();
    ctx.roundRect(28, 28, 148, 40, 10);
    ctx.fill();
    ctx.fillStyle = "#FFFFFF";
    ctx.font = '600 20px "Segoe UI", Arial, sans-serif';
    ctx.fillText("ẢNH MẪU", 52, 55);

    return canvas.toDataURL("image/jpeg", 0.88);
  }

  function createDemoJobs() {
    const demo = [
      { name: "NguyenVanAn_01.jpg", status: "DAT", code: null, reason: null },
      { name: "TranThiBinh_02.jpg", status: "DAT", code: null, reason: null },
      { name: "LeMinhChau_03.jpg", status: "DAT", code: null, reason: null },
      {
        name: "PhamQuocHuy_04.jpg",
        status: "CAN_CHUP_LAI",
        code: "Q01",
        reason: "Ảnh bị mờ, vui lòng chụp lại",
      },
      {
        name: "HoangThuHa_05.jpg",
        status: "CAN_CHUP_LAI",
        code: "Q03",
        reason: "Ảnh quá tối/quá sáng",
      },
      {
        name: "DoDucLong_06.jpg",
        status: "LOI",
        code: "E002",
        reason: "Không tìm thấy khuôn mặt",
      },
      { name: "VuMaiLinh_07.jpg", status: "CHO", code: null, reason: null },
      { name: "BuiTuanKiet_08.jpg", status: "CHO", code: null, reason: null },
    ];

    state.jobImages.forEach((item) => {
      if (item.objectUrl) URL.revokeObjectURL(item.url);
    });
    state.jobImages.clear();
    state.crops.clear();

    const jobs = demo.map((item, index) => {
      const id = `demo-${index + 1}`;
      const image = makeMockPhoto(index + 1);
      state.jobImages.set(id, { url: image, name: item.name, objectUrl: false });
      return {
        id,
        file_name: item.name,
        status: item.status,
        reason_code: item.code,
        reason_text: item.reason,
        output_path: null,
        updated_at: new Date(Date.now() - (demo.length - index) * 47000).toISOString(),
      };
    });

    state.queue = {
      jobs,
      done: computeDone(jobs),
      total: jobs.length,
      paused: false,
    };
    state.mock.demo = true;
    state.mock.running = false;

    const firstValid = jobs.find((job) => job.status === "DAT") || jobs[0];
    state.selectedJobId = firstValid ? firstValid.id : null;

    el.demoBanner.classList.remove("hidden");
  }

  function computeDone(jobs) {
    return jobs.filter((job) =>
      ["DAT", "CAN_CHUP_LAI", "LOI", "DA_XUAT"].includes(job.status)
    ).length;
  }

  function clearDemoQueue(options = {}) {
    for (const item of state.jobImages.values()) {
      if (item.objectUrl) URL.revokeObjectURL(item.url);
    }
    state.jobImages.clear();
    state.crops.clear();
    state.queue = { jobs: [], done: 0, total: 0, paused: false };
    state.selectedJobId = null;
    state.mock.demo = false;
    state.mock.running = false;
    state.mock.durations = [];
    window.clearTimeout(state.mock.timer);
    el.demoBanner.classList.add("hidden");
    renderQueue();
    renderPreview();
    renderHomeSummary();
    if (!options.silent) {
      setStatus("Sẵn sàng", "ready");
      toast("Đã xóa dữ liệu mẫu. Bấm Chọn ảnh để bắt đầu với ảnh của bạn.", {
        tone: "info",
      });
    }
  }

  /* ------------------------------------------------------------------------
     Trạng thái hàng đợi
     ------------------------------------------------------------------------ */

  function isProcessing() {
    return (
      state.mock.running ||
      state.queue.jobs.some((job) => job.status === "DANG_XU_LY")
    );
  }

  function findJob(id) {
    return state.queue.jobs.find((job) => job.id === id) || null;
  }

  function applyJobUpdate(job) {
    if (!job || !job.id) return;
    const index = state.queue.jobs.findIndex((item) => item.id === job.id);
    if (index >= 0) {
      state.queue.jobs[index] = { ...state.queue.jobs[index], ...job };
    } else {
      state.queue.jobs.push({ ...job });
    }
    state.queue.total = state.queue.jobs.length;
    state.queue.done = computeDone(state.queue.jobs);
    state.queue.paused = Boolean(state.queue.paused);

    if (isProcessing()) {
      setStatus("Đang xử lý…", "busy");
    } else if (state.queue.paused) {
      setStatus("Tạm dừng", "paused");
    }

    renderQueue();
    renderHomeSummary();
    renderPreview();
  }

  function handleBatchCompleted(payload) {
    const ok = Math.max(0, Number(payload && payload.ok) || 0);
    const needs = Math.max(0, Number(payload && payload.needs_retake) || 0);
    const failed = Math.max(0, Number(payload && payload.failed) || 0);
    const total = ok + needs + failed;

    state.mock.running = false;
    state.queue.paused = false;
    window.clearTimeout(state.mock.timer);
    setStatus("Sẵn sàng", "ready");

    let message = `Xong ${ok}/${total || state.queue.total} ảnh.`;
    if (needs > 0) message += ` ${needs} ảnh cần chụp lại.`;
    if (failed > 0) message += ` ${failed} ảnh lỗi.`;
    toast(message, { tone: needs || failed ? "warn" : "success", title: "Hoàn tất lô ảnh" });

    renderQueue();
    renderHomeSummary();
    renderPreview();
  }

  /* ------------------------------------------------------------------------
     Hiển thị Trang chủ
     ------------------------------------------------------------------------ */

  function syncHomeForm() {
    const preset = activePreset();

    el.inputDirText.textContent = state.settings.input_dir || "Chưa chọn";
    el.inputDirText.title = state.settings.input_dir || "Chưa chọn thư mục ảnh gốc";
    el.inputDirText.classList.toggle("is-empty", !state.settings.input_dir);

    el.outputDirText.textContent = state.settings.output_dir || "Chưa chọn";
    el.outputDirText.title = state.settings.output_dir || "Chưa chọn thư mục lưu ảnh";
    el.outputDirText.classList.toggle("is-empty", !state.settings.output_dir);

    el.presetSelect.value = state.settings.default_preset;
    el.presetHint.textContent = `${preset.size} · ${preset.px[0]} × ${preset.px[1]} px · nền ${
      state.settings.default_bg_hex === "#FFFFFF" ? "trắng" : "theo màu đã chọn"
    }.`;
    el.presetNoteText.textContent =
      preset.id === "tuy-chinh"
        ? preset.note
        : `${preset.note} Tâm mắt đặt ở ${state.ui.eye_level_percent}% chiều cao tính từ đáy.`;

    el.customSizeField.classList.toggle("hidden", preset.id !== "tuy-chinh");
    el.customWidth.value = state.ui.custom_width_mm;
    el.customHeight.value = state.ui.custom_height_mm;
    el.faceRatio.value = state.ui.face_ratio;
    el.faceRatioOut.textContent = `${state.ui.face_ratio}%`;

    el.outfitToggle.checked = state.ui.outfit_enabled;
    el.outfitSelect.disabled = !state.ui.outfit_enabled;
    el.outfitSelect.value = state.ui.selected_outfit;

    renderSwatches(el.bgSwatches, state.settings.default_bg_hex, "home");
    renderSwatches(el.previewSwatches, state.settings.default_bg_hex, "preview");
  }

  function renderSwatches(container, selectedHex, scope) {
    if (!container) return;
    const hex = normalizeHex(selectedHex) || "#FFFFFF";
    container.innerHTML = BG_COLORS.map((color) => {
      const active = color.hex === hex;
      return `
        <button class="swatch${active ? " is-active" : ""}" type="button"
          data-hex="${color.hex}" data-scope="${scope}" aria-pressed="${active}"
          title="${escapeHtml(color.name)}">
          <span class="swatch-color" style="background:${color.hex}"></span>
          <span class="swatch-label">${escapeHtml(color.name)}</span>
        </button>
      `;
    }).join("");
  }

  function renderQueue() {
    const jobs = state.queue.jobs;
    const total = jobs.length;

    el.queueEmpty.classList.toggle("hidden", total > 0);
    const table = el.queueBody.closest("table");
    if (table) table.classList.toggle("hidden", total === 0);

    el.queueBody.innerHTML = jobs
      .map((job, index) => {
        const meta = JOB_STATUS[job.status] || { label: job.status, tone: "wait" };
        const selected = job.id === state.selectedJobId;
        const reasonCode = job.reason_code
          ? `<span class="reason-code">${escapeHtml(job.reason_code)}</span>`
          : "";
        let reasonText = job.reason_text || "";
        if (!reasonText) {
          if (job.status === "CHO") reasonText = "Đang chờ xử lý";
          else if (job.status === "DANG_XU_LY") reasonText = "Đang xử lý ảnh…";
          else if (job.status === "DAT") reasonText = "Sẵn sàng xuất ảnh";
          else if (job.status === "DA_XUAT")
            reasonText = job.output_path ? baseName(job.output_path) : "Đã lưu vào thư mục đầu ra";
          else reasonText = "—";
        }

        return `
          <tr data-job-id="${escapeHtml(job.id)}" class="${selected ? "is-selected" : ""}">
            <td>
              <div class="file-cell">
                <span class="file-index">${String(index + 1).padStart(2, "0")}</span>
                <div class="file-info">
                  <span class="file-name" title="${escapeHtml(job.file_name)}">${escapeHtml(
                    job.file_name
                  )}</span>
                  <span class="file-id">${escapeHtml(job.id)}</span>
                </div>
              </div>
            </td>
            <td>
              <span class="job-status tone-${meta.tone}">${escapeHtml(meta.label)}</span>
            </td>
            <td>
              <div class="reason-cell">
                <span class="reason-text${reasonText === "—" ? " is-empty" : ""}" title="${escapeHtml(
                  reasonText
                )}">${escapeHtml(reasonText)}</span>
                ${reasonCode}
              </div>
            </td>
            <td>
              <button class="row-action" type="button" data-action="preview" title="Xem trước ảnh">
                <svg class="icon" viewBox="0 0 24 24"><use href="#i-eye"></use></svg>
                <span class="sr-only">Xem trước ${escapeHtml(job.file_name)}</span>
              </button>
            </td>
          </tr>
        `;
      })
      .join("");

    renderHomeSummary();
  }

  let queueLottieAnimation = null;

  function syncQueueLottie(processing) {
    if (!window.lottie || !el.queueLottie) return;
    if (processing && !queueLottieAnimation) {
      try {
        queueLottieAnimation = window.lottie.loadAnimation({
          container: el.queueLottie,
          renderer: "svg",
          loop: true,
          autoplay: true,
          path: "lottie/dang-xu-ly.json",
        });
      } catch (error) {
        queueLottieAnimation = null;
      }
    } else if (!processing && queueLottieAnimation) {
      try {
        queueLottieAnimation.destroy();
      } catch (error) {
        /* Bỏ qua lỗi khi hủy animation. */
      }
      queueLottieAnimation = null;
      el.queueLottie.innerHTML = "";
    }
  }

  function renderHomeSummary() {
    const total = state.queue.jobs.length;
    const done = state.queue.done;
    const paused = state.queue.paused;
    const processing = isProcessing();
    const percent = total > 0 ? Math.round((done / total) * 100) : 0;

    el.queueSummary.textContent =
      total > 0
        ? `${formatNumber(total)} ảnh · ${formatNumber(done)} đã xong${paused ? " · tạm dừng" : ""}`
        : "Chưa có ảnh nào trong lô.";

    el.bottomQueueText.textContent =
      total > 0
        ? `${formatNumber(done)}/${formatNumber(total)} ảnh đã xử lý`
        : "Chưa có ảnh trong lô";

    el.progressPercent.textContent = `${percent}%`;
    el.progressFill.style.width = `${percent}%`;
    el.progressBar.setAttribute("aria-valuenow", String(percent));
    el.doneText.textContent = `${formatNumber(done)}/${formatNumber(total)} ảnh`;

    if (processing) {
      el.progressLabel.textContent = `Đang xử lý ${formatNumber(done)}/${formatNumber(total)} ảnh`;
    } else if (paused && total > 0 && done < total) {
      el.progressLabel.textContent = "Đã tạm dừng";
    } else if (total > 0 && done >= total) {
      el.progressLabel.textContent = "Đã xử lý xong";
    } else if (total > 0) {
      el.progressLabel.textContent = "Sẵn sàng xử lý";
    } else {
      el.progressLabel.textContent = "Chưa bắt đầu";
    }

    if (processing && total > done) {
      const average =
        state.mock.durations.length > 0
          ? state.mock.durations.reduce((sum, value) => sum + value, 0) /
            state.mock.durations.length
          : 1.1;
      const remaining = (total - done) * average;
      el.etaText.textContent = `Ước tính còn khoảng ${formatDuration(remaining)}.`;
    } else if (total > 0 && done >= total) {
      el.etaText.textContent = "Đã hoàn tất. Bạn có thể xuất ảnh.";
    } else if (paused) {
      el.etaText.textContent = "Bấm Tiếp tục để xử lý các ảnh còn lại.";
    } else {
      el.etaText.textContent = "Sẵn sàng khi bạn bấm Bắt đầu.";
    }

    const hasWork = total > 0 && done < total;
    el.btnPauseQueue.disabled = !hasWork;
    el.btnPauseQueue.title = hasWork
      ? paused
        ? "Tiếp tục xử lý hàng đợi"
        : "Tạm dừng sau ảnh đang xử lý"
      : "Không có ảnh nào đang chờ";
    el.pauseLabel.textContent = paused ? "Tiếp tục" : "Tạm dừng";
    const pauseIcon = qs("use", el.btnPauseQueue);
    if (pauseIcon) pauseIcon.setAttribute("href", paused ? "#i-play" : "#i-pause");

    el.btnStartLabel.textContent = state.queue.paused
      ? "Tiếp tục xử lý"
      : processing
        ? "Tạm dừng"
        : "Bắt đầu xử lý";
    const startIcon = qs("use", el.btnStartQueue);
    if (startIcon) {
      startIcon.setAttribute("href", processing && !state.queue.paused ? "#i-pause" : "#i-play");
    }

    el.queueLottie.classList.toggle("is-visible", processing);
    syncQueueLottie(processing);
  }

  /* ------------------------------------------------------------------------
     Lịch sử, Giới thiệu, Cài đặt
     ------------------------------------------------------------------------ */

  function renderHistory() {
    const items = state.history;
    el.historyEmpty.classList.toggle("hidden", items.length > 0);
    el.historyList.innerHTML = items
      .map((record) => {
        const preset = PRESET_DEFS[record.preset] || PRESET_DEFS["3x4"];
        return `
          <div class="history-item" data-history-id="${escapeHtml(record.id)}">
            <div class="history-main">
              <span class="history-title">${escapeHtml(
                `${record.count} ảnh · ${preset.name} · ${record.bg_hex}`
              )}</span>
              <span class="history-meta" title="${escapeHtml(record.output_dir || "")}">
                ${escapeHtml(formatDateTime(record.date))} · ${escapeHtml(
                  record.output_dir || "Chưa có thư mục đầu ra"
                )}
              </span>
            </div>
            <div class="history-stats">
              <span class="hist-stat ok">${formatNumber(record.ok || 0)} đạt</span>
              ${
                record.needs_retake
                  ? `<span class="hist-stat warn">${formatNumber(record.needs_retake)} chụp lại</span>`
                  : ""
              }
              ${
                record.failed
                  ? `<span class="hist-stat error">${formatNumber(record.failed)} lỗi</span>`
                  : ""
              }
            </div>
            <div class="history-actions">
              <button class="btn btn-ghost btn-tiny" type="button" data-history-action="open">
                <svg class="icon" viewBox="0 0 24 24"><use href="#i-folder"></use></svg>
                Mở thư mục
              </button>
            </div>
          </div>
        `;
      })
      .join("");
  }

  function renderAbout() {
    const hasConsent = Boolean(state.settings.minor_consent_at);
    const consentText = hasConsent
      ? `Đã xác nhận dữ liệu lúc ${formatDateTime(
          state.settings.minor_consent_at
        )} · Phiên bản nội dung ${state.settings.minor_consent_version || "—"}.`
      : "Chưa có xác nhận dữ liệu. Bạn cần hoàn tất bước xác nhận trước khi xử lý ảnh.";
    el.aboutConsentText.textContent = consentText;
    el.consentInfo.textContent = consentText;
  }

  function fillSelect(select, options, value) {
    select.innerHTML = options
      .map(
        (option) =>
          `<option value="${escapeHtml(option.value)}">${escapeHtml(option.label)}</option>`
      )
      .join("");
    if (value != null) select.value = value;
  }

  function renderSettingsForm() {
    el.setInputDirText.textContent = state.settings.input_dir || "Chưa chọn";
    el.setInputDirText.title = state.settings.input_dir || "Chưa chọn thư mục ảnh gốc";
    el.setInputDirText.classList.toggle("is-empty", !state.settings.input_dir);
    el.setOutputDirText.textContent = state.settings.output_dir || "Chưa chọn";
    el.setOutputDirText.title = state.settings.output_dir || "Chưa chọn thư mục lưu ảnh";
    el.setOutputDirText.classList.toggle("is-empty", !state.settings.output_dir);

    el.setDefaultPreset.value = state.settings.default_preset;
    el.setDefaultBgHex.value = state.settings.default_bg_hex;
    el.setCustomBgHex.value = state.ui.custom_bg_hex;
    el.setExportFormat.value = state.ui.export_format;
    el.setNameTemplate.value = state.ui.name_template;

    el.setCpuLimit.value = state.settings.cpu_limit_percent;
    el.setCpuLimitOut.textContent = `${state.settings.cpu_limit_percent}%`;
    el.setThreadLimit.value = state.ui.thread_limit;
    el.setThreadLimitOut.textContent =
      Number(state.ui.thread_limit) === 0 ? "Tự động" : `${state.ui.thread_limit} luồng`;
    el.setPollInterval.value = String(state.settings.poll_interval_secs);
    el.setStartWithWindows.checked = state.ui.start_with_windows;
    el.setMinimizeToTray.checked = state.ui.minimize_to_tray;

    el.setPauseBusy.checked = state.settings.pause_when_busy;
    el.setPauseBattery.checked = state.settings.pause_on_battery;

    el.setMatteAccuracy.value = state.ui.matte_accuracy;
    el.setMatteOut.textContent = String(state.ui.matte_accuracy);
    el.setAlignHead.checked = state.ui.align_head;
    el.setEyeLevel.value = state.ui.eye_level_percent;
    el.setEyeLevelOut.textContent = `${state.ui.eye_level_percent}%`;

    el.outfitTemplateName.textContent = state.ui.outfit_template_name || "Chưa chọn file";
    el.outfitTemplateName.title = state.ui.outfit_template_name || "";

    el.setRemoveOriginal.checked = state.settings.remove_original_after_export;
    el.removeOriginalWarning.classList.toggle(
      "is-visible",
      state.settings.remove_original_after_export
    );
    el.setSecureDelete.checked = state.settings.secure_delete;
    el.setLogEnabled.checked = state.settings.local_log_enabled;

    renderSwatches(el.setBgSwatches, state.settings.default_bg_hex, "settings");
    renderAbout();
  }

  function renderPreview() {
    const job = findJob(state.preview.jobId);
    const hasJob = Boolean(job);
    const image = job ? state.jobImages.get(job.id) : null;

    el.btnReprocess.disabled = !hasJob;
    el.btnRemoveJob.disabled = !hasJob;
    el.zoomRange.disabled = !hasJob;
    el.matteToggle.disabled = !hasJob;
    el.previewOutfit.disabled = !hasJob || !state.ui.outfit_enabled;
    el.previewOutfit.value = state.ui.selected_outfit;

    if (!job) {
      el.previewBadge.textContent = "Chưa chọn ảnh";
      el.previewFile.textContent = "Chọn một ảnh trong hàng đợi để xem trước.";
      el.stageEmpty.classList.remove("hidden");
      el.cropWrap.classList.add("hidden");
      el.originalSize.textContent = "";
      el.resultSize.textContent = "";
      el.resultCanvas.classList.add("hidden");
      el.resultImg.classList.add("hidden");
      el.resultPlaceholder.classList.remove("hidden");
      el.mockBadge.classList.add("hidden");
      el.previewBadge.className = "badge";
      state.preview.imgLoaded = false;
      state.preview.url = null;
      return;
    }

    const meta = JOB_STATUS[job.status] || { label: job.status, tone: "wait" };
    el.previewBadge.textContent = meta.label;
    el.previewBadge.className = `badge tone-${meta.tone}`;
    el.previewFile.textContent = job.file_name;
    el.previewFile.title = job.file_name;

    const preset = activePreset();
    el.resultSize.textContent = `${preset.px[0]} × ${preset.px[1]} px`;
    el.zoomRange.value = String(getCrop(job.id).z);
    el.zoomOut.textContent = `${getCrop(job.id).z.toFixed(1)}×`;
    el.matteHint.textContent = state.preview.matte ? "Đang bật" : "Đang tắt";

    if (image && image.url) {
      if (state.preview.jobId !== job.id || state.preview.url !== image.url) {
        state.preview.url = image.url;
        state.preview.imgLoaded = false;
        el.originalImg.src = image.url;
      }
      el.stageEmpty.classList.add("hidden");
      el.cropWrap.classList.remove("hidden");
      el.mockBadge.classList.toggle("hidden", !state.mock.demo);
      el.resultCanvas.classList.remove("hidden");
      el.resultImg.classList.add("hidden");
      el.resultPlaceholder.classList.add("hidden");
      if (state.preview.imgLoaded) {
        clampCrop(getCrop(job.id));
        renderCropBox();
        drawPreviewResult();
      }
      return;
    }

    // Không có ảnh cục bộ: thử dùng ảnh kết quả từ backend nếu có.
    state.preview.imgLoaded = false;
    el.stageEmpty.classList.remove("hidden");
    el.cropWrap.classList.add("hidden");
    el.originalSize.textContent = "";
    el.mockBadge.classList.add("hidden");

    const outputPath = job.output_path;
    if (hasTauri && outputPath && window.__TAURI__.core.convertFileSrc) {
      try {
        el.resultImg.src = window.__TAURI__.core.convertFileSrc(outputPath);
        el.resultImg.classList.remove("hidden");
        el.resultCanvas.classList.add("hidden");
        el.resultPlaceholder.classList.add("hidden");
      } catch (error) {
        el.resultImg.classList.add("hidden");
        el.resultCanvas.classList.add("hidden");
        el.resultPlaceholder.classList.remove("hidden");
      }
    } else {
      el.resultImg.classList.add("hidden");
      el.resultCanvas.classList.add("hidden");
      el.resultPlaceholder.classList.remove("hidden");
    }
  }

  function renderAll() {
    syncHomeForm();
    renderQueue();
    renderSettingsForm();
    renderHistory();
    renderAbout();
    renderPreview();
    renderHomeSummary();
  }

  /* ------------------------------------------------------------------------
     Xem trước & khung cắt
     ------------------------------------------------------------------------ */

  function getCrop(jobId) {
    if (!state.crops.has(jobId)) {
      state.crops.set(jobId, { cx: 0.5, cy: 0.44, z: 1 });
    }
    return state.crops.get(jobId);
  }

  function clampCrop(crop) {
    if (!state.preview.imgLoaded || !state.preview.naturalWidth) return crop;
    const naturalWidth = state.preview.naturalWidth;
    const naturalHeight = state.preview.naturalHeight;
    const aspect = presetAspect();
    const baseWidth = Math.min(naturalWidth, naturalHeight * aspect);
    const cropWidth = baseWidth / clamp(crop.z, 1, 3);
    const cropHeight = cropWidth / aspect;
    const halfWidth = cropWidth / naturalWidth / 2;
    const halfHeight = cropHeight / naturalHeight / 2;

    crop.z = clamp(crop.z, 1, 3);
    crop.cx = halfWidth >= 0.5 ? 0.5 : clamp(crop.cx, halfWidth, 1 - halfWidth);
    crop.cy = halfHeight >= 0.5 ? 0.5 : clamp(crop.cy, halfHeight, 1 - halfHeight);
    return crop;
  }

  function syncCropControls() {
    if (!state.preview.jobId) return;
    const crop = getCrop(state.preview.jobId);
    el.zoomRange.value = String(crop.z);
    el.zoomOut.textContent = `${crop.z.toFixed(1)}×`;
  }

  function renderCropBox() {
    if (!state.preview.imgLoaded || !state.preview.jobId) return;
    if (el.cropWrap.classList.contains("hidden")) return;

    const imageRect = el.originalImg.getBoundingClientRect();
    const wrapRect = el.cropWrap.getBoundingClientRect();
    if (!imageRect.width || !imageRect.height) return;

    const crop = clampCrop(getCrop(state.preview.jobId));
    const naturalWidth = state.preview.naturalWidth;
    const naturalHeight = state.preview.naturalHeight;
    const aspect = presetAspect();
    const baseWidth = Math.min(naturalWidth, naturalHeight * aspect);
    const cropWidth = baseWidth / crop.z;
    const cropHeight = cropWidth / aspect;

    const scaleX = imageRect.width / naturalWidth;
    const scaleY = imageRect.height / naturalHeight;
    const left = imageRect.left - wrapRect.left + (crop.cx * naturalWidth - cropWidth / 2) * scaleX;
    const top = imageRect.top - wrapRect.top + (crop.cy * naturalHeight - cropHeight / 2) * scaleY;

    el.cropBox.style.left = `${left}px`;
    el.cropBox.style.top = `${top}px`;
    el.cropBox.style.width = `${cropWidth * scaleX}px`;
    el.cropBox.style.height = `${cropHeight * scaleY}px`;
    syncCropControls();
  }

  function drawPreviewResult() {
    if (!state.preview.jobId || !state.preview.imgLoaded) return;
    const crop = clampCrop(getCrop(state.preview.jobId));
    const preset = activePreset();
    const canvas = el.resultCanvas;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const width = preset.px[0];
    const height = preset.px[1];
    if (canvas.width !== width || canvas.height !== height) {
      canvas.width = width;
      canvas.height = height;
    }

    const naturalWidth = state.preview.naturalWidth;
    const naturalHeight = state.preview.naturalHeight;
    const aspect = width / height;
    const baseWidth = Math.min(naturalWidth, naturalHeight * aspect);
    const cropWidth = baseWidth / crop.z;
    const cropHeight = cropWidth / aspect;
    const sourceX = crop.cx * naturalWidth - cropWidth / 2;
    const sourceY = crop.cy * naturalHeight - cropHeight / 2;

    ctx.save();
    ctx.fillStyle = state.settings.default_bg_hex || "#FFFFFF";
    ctx.fillRect(0, 0, width, height);
    ctx.imageSmoothingEnabled = true;
    ctx.imageSmoothingQuality = "high";
    try {
      ctx.drawImage(
        el.originalImg,
        sourceX,
        sourceY,
        cropWidth,
        cropHeight,
        0,
        0,
        width,
        height
      );
    } catch (error) {
      /* Ảnh có thể chưa sẵn sàng; bỏ qua khung hình này. */
    }
    ctx.restore();
    el.resultCanvas.classList.remove("hidden");
    el.resultPlaceholder.classList.add("hidden");
  }

  function handleOriginalLoaded() {
    const image = el.originalImg;
    if (!image.naturalWidth || !image.naturalHeight) return;
    state.preview.imgLoaded = true;
    state.preview.naturalWidth = image.naturalWidth;
    state.preview.naturalHeight = image.naturalHeight;
    el.originalSize.textContent = `${image.naturalWidth} × ${image.naturalHeight} px`;

    if (state.preview.jobId) {
      clampCrop(getCrop(state.preview.jobId));
      renderCropBox();
      syncCropControls();
      drawPreviewResult();
    }
  }

  function selectJob(id) {
    if (!findJob(id)) return;
    state.selectedJobId = id;
    state.preview.jobId = id;
    state.preview.url = null;
    state.preview.imgLoaded = false;
    renderQueue();
    renderPreview();
    if (window.innerWidth > 940) {
      el.cropBox.focus({ preventScroll: true });
    }
  }

  function moveCropBy(dx, dy) {
    if (!state.preview.jobId || !state.preview.imgLoaded) return;
    const crop = getCrop(state.preview.jobId);
    crop.cx += dx;
    crop.cy += dy;
    clampCrop(crop);
    renderCropBox();
    drawPreviewResult();
  }

  function setCropZoom(value) {
    if (!state.preview.jobId) return;
    const crop = getCrop(state.preview.jobId);
    crop.z = clamp(Number(value) || 1, 1, 3);
    clampCrop(crop);
    renderCropBox();
    syncCropControls();
    drawPreviewResult();
  }

  function bindCropDrag() {
    let drag = null;

    el.cropBox.addEventListener("pointerdown", (event) => {
      if (!state.preview.jobId || !state.preview.imgLoaded) return;
      event.preventDefault();
      const rect = el.originalImg.getBoundingClientRect();
      const jobId = state.preview.jobId;
      const crop = getCrop(jobId);
      drag = {
        jobId,
        mode: event.target === el.cropHandle ? "resize" : "move",
        startX: event.clientX,
        startY: event.clientY,
        startCrop: { ...crop },
        rectWidth: rect.width,
        rectHeight: rect.height,
      };
      el.cropBox.setPointerCapture?.(event.pointerId);
    });

    el.cropBox.addEventListener("pointermove", (event) => {
      if (!drag) return;
      const crop = getCrop(drag.jobId);
      if (drag.mode === "move") {
        crop.cx = drag.startCrop.cx + (event.clientX - drag.startX) / drag.rectWidth;
        crop.cy = drag.startCrop.cy + (event.clientY - drag.startY) / drag.rectHeight;
      } else {
        const deltaX = event.clientX - drag.startX;
        const deltaY = event.clientY - drag.startY;
        const delta = Math.abs(deltaX) > Math.abs(deltaY) ? deltaX : deltaY;
        crop.z = drag.startCrop.z - (delta / drag.rectWidth) * 1.7;
      }
      clampCrop(crop);
      renderCropBox();
      drawPreviewResult();
    });

    const endDrag = (event) => {
      if (!drag) return;
      drag = null;
      try {
        el.cropBox.releasePointerCapture?.(event.pointerId);
      } catch (error) {
        /* Trình duyệt cũ có thể không hỗ trợ pointer capture. */
      }
    };

    el.cropBox.addEventListener("pointerup", endDrag);
    el.cropBox.addEventListener("pointercancel", endDrag);

    el.stage.addEventListener(
      "wheel",
      (event) => {
        if (!state.preview.jobId || !state.preview.imgLoaded) return;
        event.preventDefault();
        const crop = getCrop(state.preview.jobId);
        crop.z = clamp(crop.z - event.deltaY * 0.0015, 1, 3);
        clampCrop(crop);
        renderCropBox();
        syncCropControls();
        drawPreviewResult();
      },
      { passive: false }
    );
  }

  /* ------------------------------------------------------------------------
     Chọn ảnh / thư mục
     ------------------------------------------------------------------------ */

  function revokeJobImages() {
    for (const item of state.jobImages.values()) {
      if (item.objectUrl) URL.revokeObjectURL(item.url);
    }
    state.jobImages.clear();
    state.crops.clear();
  }

  function handleLocalFiles(fileList) {
    const files = Array.from(fileList || []).filter((file) => {
      return (
        (file.type && file.type.startsWith("image/")) ||
        /\.(jpe?g|png|webp|bmp|tiff?|heic)$/i.test(file.name)
      );
    });

    if (files.length === 0) {
      toast("Không tìm thấy ảnh hợp lệ trong thư mục đã chọn.", {
        tone: "warn",
        title: "Chưa có ảnh",
      });
      return;
    }

    state.mock.running = false;
    window.clearTimeout(state.mock.timer);
    revokeJobImages();
    state.queue = { jobs: [], done: 0, total: 0, paused: false };
    state.selectedJobId = null;
    state.preview.jobId = null;
    state.mock.demo = false;
    el.demoBanner.classList.add("hidden");

    const stamp = Date.now();
    const jobs = files.map((file, index) => {
      const id = `file-${stamp}-${index + 1}`;
      const url = URL.createObjectURL(file);
      state.jobImages.set(id, { url, name: file.name, objectUrl: true });
      return {
        id,
        file_name: file.name,
        status: "CHO",
        reason_code: null,
        reason_text: null,
        output_path: null,
        updated_at: nowIso(),
      };
    });

    state.queue = { jobs, done: 0, total: jobs.length, paused: false };
    const folderName = (files[0].webkitRelativePath || "").split("/")[0];
    state.settings.input_dir = folderName
      ? `${DEMO_FOLDER_IN}\\${folderName}`
      : "Thư mục đã chọn trên trình duyệt";
    if (!state.settings.output_dir) state.settings.output_dir = DEMO_FOLDER_OUT;

    state.selectedJobId = jobs[0].id;
    state.preview.jobId = jobs[0].id;
    state.preview.url = null;
    state.preview.imgLoaded = false;

    if (!hasTauri) {
      saveJson(STORAGE_SETTINGS, state.settings);
    }
    renderAll();
    setStatus("Sẵn sàng", "ready");
    toast(`Đã thêm ${formatNumber(jobs.length)} ảnh vào hàng đợi.`, {
      tone: "success",
      title: "Đã chọn ảnh",
    });
  }

  async function chooseInputFolder() {
    if (hasTauri) {
      const path = await chooseFolder("input");
      if (!path) return;
      state.settings.input_dir = path;
      await saveSettingsToBackend();
      renderAll();
      await refreshQueueState();
      toast(`Đã chọn thư mục ảnh gốc: ${path}`, { tone: "success" });
      return;
    }
    el.filePicker.click();
  }

  async function chooseOutputFolder() {
    const path = await chooseFolder("output");
    if (!path) return;
    state.settings.output_dir = path;
    await saveSettingsToBackend();
    renderAll();
    toast(`Đã chọn thư mục lưu ảnh: ${path}`, { tone: "success" });
  }

  async function refreshQueueState() {
    if (!hasTauri) return;
    try {
      const queue = await invokeCommand("get_queue_state");
      const jobs = Array.isArray(queue && queue.jobs) ? queue.jobs : [];
      state.queue = {
        jobs,
        done: Number(queue && queue.done) || 0,
        total: Number(queue && queue.total) || jobs.length,
        paused: Boolean(queue && queue.paused),
      };
      if (!jobs.some((job) => job.id === state.selectedJobId)) {
        state.selectedJobId = jobs.length > 0 ? jobs[0].id : null;
        state.preview.jobId = state.selectedJobId;
      }
      if (state.queue.paused) setStatus("Tạm dừng", "paused");
      renderAll();
    } catch (error) {
      showAppError(error, "Không đọc được trạng thái hàng đợi.");
    }
  }

  /* ------------------------------------------------------------------------
     Xử lý hàng đợi (mô phỏng)
     ------------------------------------------------------------------------ */

  function mockOutcome(job, index) {
    const numeric = Number(String(job.id).split("-").pop()) || index + 1;
    if (numeric % 7 === 0) {
      return {
        status: "CAN_CHUP_LAI",
        code: "Q01",
        reason: "Ảnh bị mờ, vui lòng chụp lại",
      };
    }
    if (numeric % 13 === 0) {
      return {
        status: "LOI",
        code: "E003",
        reason: "Ảnh có nhiều người trong khung",
      };
    }
    return { status: "DAT", code: null, reason: null };
  }

  function processNextMockJob() {
    if (!state.mock.running) return;

    if (state.queue.paused) {
      setStatus("Tạm dừng", "paused");
      renderHomeSummary();
      return;
    }

    const job = state.queue.jobs.find((item) => item.status === "CHO");
    if (!job) {
      finishMockBatch();
      return;
    }

    const index = state.queue.jobs.indexOf(job);
    job.status = "DANG_XU_LY";
    job.updated_at = nowIso();
    applyJobUpdate(job);
    setStatus("Đang xử lý…", "busy");
    state.mock.startedAt = performance.now();

    const delay = 720 + Math.random() * 780;
    state.mock.timer = window.setTimeout(() => {
      if (!state.mock.running) return;
      const elapsed = (performance.now() - state.mock.startedAt) / 1000;
      state.mock.durations.push(elapsed);
      if (state.mock.durations.length > 10) state.mock.durations.shift();

      const outcome = mockOutcome(job, index);
      job.status = outcome.status;
      job.reason_code = outcome.code;
      job.reason_text = outcome.reason;
      job.updated_at = nowIso();
      applyJobUpdate(job);

      if (state.queue.paused) {
        state.mock.running = false;
        setStatus("Tạm dừng", "paused");
        renderHomeSummary();
        return;
      }
      processNextMockJob();
    }, delay);
  }

  function finishMockBatch() {
    state.mock.running = false;
    window.clearTimeout(state.mock.timer);
    const jobs = state.queue.jobs;
    const ok = jobs.filter((job) => job.status === "DAT" || job.status === "DA_XUAT").length;
    const needs = jobs.filter((job) => job.status === "CAN_CHUP_LAI").length;
    const failed = jobs.filter((job) => job.status === "LOI").length;
    handleBatchCompleted({ ok, needs_retake: needs, failed });
  }

  async function startQueueAction() {
    if (!ensureConsent()) return;

    if (state.queue.jobs.length === 0) {
      toast("Chưa có ảnh nào trong hàng đợi. Hãy bấm Chọn ảnh trước.", {
        tone: "warn",
        title: "Chưa thể bắt đầu",
      });
      return;
    }

    if (state.queue.paused) {
      state.queue.paused = false;
      if (hasTauri) {
        try {
          await invokeCommand("start_queue");
        } catch (error) {
          showAppError(error, "Không tiếp tục được hàng đợi.");
          return;
        }
      } else {
        state.mock.running = true;
        setStatus("Đang xử lý…", "busy");
        processNextMockJob();
      }
      renderAll();
      return;
    }

    if (!state.queue.jobs.some((job) => job.status === "CHO")) {
      const allDone = state.queue.jobs.every((job) =>
        ["DAT", "CAN_CHUP_LAI", "LOI", "DA_XUAT"].includes(job.status)
      );
      if (allDone) {
        toast("Tất cả ảnh đã được xử lý. Bạn có thể xuất các ảnh đạt.", {
          tone: "info",
          title: "Không còn ảnh chờ",
        });
      }
      return;
    }

    state.queue.paused = false;
    state.mock.durations = [];

    if (hasTauri) {
      try {
        await invokeCommand("start_queue");
        setStatus("Đang xử lý…", "busy");
        renderHomeSummary();
        window.setTimeout(refreshQueueState, 500);
      } catch (error) {
        showAppError(error, "Không bắt đầu được hàng đợi.");
      }
      return;
    }

    state.mock.running = true;
    setStatus("Đang xử lý…", "busy");
    renderHomeSummary();
    processNextMockJob();
  }

  async function pauseQueueAction() {
    if (state.queue.jobs.length === 0) return;

    if (state.queue.paused) {
      await startQueueAction();
      return;
    }

    state.queue.paused = true;
    if (hasTauri) {
      try {
        await invokeCommand("pause_queue");
      } catch (error) {
        showAppError(error, "Không tạm dừng được hàng đợi.");
        state.queue.paused = false;
        return;
      }
    }
    setStatus("Tạm dừng", "paused");
    renderAll();
    toast("Đã tạm dừng. Ảnh đang xử lý dở sẽ được hoàn tất trước khi dừng.", {
      tone: "info",
    });
  }

  /* ------------------------------------------------------------------------
     Xuất ảnh, xử lý lại, loại bỏ
     ------------------------------------------------------------------------ */

  function ensureConsent() {
    if (state.settings.minor_consent_at) return true;
    toast("Bạn cần xác nhận dữ liệu ở bước 1 của hướng dẫn lần đầu trước khi xử lý ảnh.", {
      tone: "warn",
      title: "Chưa xác nhận dữ liệu",
      timeout: 6000,
    });
    if (state.wizard) state.wizard.open(0);
    return false;
  }

  function buildExportPayload() {
    return {
      preset: activePreset().id,
      bg_hex: state.settings.default_bg_hex,
      outfit: currentOutfit(),
    };
  }

  function simulatedOutputPath(job) {
    const preset = activePreset();
    const base = job.file_name.replace(/\.[^.]+$/, "");
    const color = state.settings.default_bg_hex.replace("#", "").toLowerCase();
    const extension = state.ui.export_format === "png" ? "png" : "jpg";
    const folder = state.settings.output_dir || DEMO_FOLDER_OUT;
    return `${folder}\\${base}_${preset.id}_${color}.${extension}`;
  }

  function addHistoryRecord(exportedCount) {
    const jobs = state.queue.jobs;
    const record = {
      id: `batch-${Date.now()}`,
      date: nowIso(),
      count: exportedCount,
      preset: activePreset().id,
      bg_hex: state.settings.default_bg_hex,
      outfit: currentOutfit(),
      output_dir: state.settings.output_dir || DEMO_FOLDER_OUT,
      ok: jobs.filter((job) => ["DAT", "DA_XUAT"].includes(job.status)).length,
      needs_retake: jobs.filter((job) => job.status === "CAN_CHUP_LAI").length,
      failed: jobs.filter((job) => job.status === "LOI").length,
    };
    state.history.unshift(record);
    state.history = state.history.slice(0, 40);
    persistHistory();
    renderHistory();
  }

  async function exportBatch() {
    if (!ensureConsent()) return;

    if (state.queue.jobs.length === 0) {
      toast("Chưa có ảnh nào để xuất. Hãy chọn ảnh và bắt đầu xử lý trước.", {
        tone: "warn",
        title: "Chưa thể xuất",
      });
      return;
    }

    const eligible = state.queue.jobs.filter((job) => job.status === "DAT");
    if (eligible.length === 0) {
      toast("Chưa có ảnh nào ở trạng thái Đạt để xuất. Hãy chụp lại các ảnh cần chụp lại.", {
        tone: "warn",
        title: "Không có ảnh đạt",
        timeout: 5600,
      });
      return;
    }

    const payload = buildExportPayload();
    setStatus("Đang xuất ảnh…", "busy");

    if (hasTauri) {
      try {
        await invokeCommand("export_batch", payload);
        addHistoryRecord(eligible.length);
        setStatus("Sẵn sàng", "ready");
        toast(`Đã gửi yêu cầu xuất ${formatNumber(eligible.length)} ảnh.`, {
          tone: "success",
          title: "Đang xuất ảnh",
        });
      } catch (error) {
        setStatus("Sẵn sàng", "ready");
        showAppError(error, "Không xuất được ảnh.");
      }
      return;
    }

    window.setTimeout(() => {
      for (const job of eligible) {
        job.status = "DA_XUAT";
        job.output_path = simulatedOutputPath(job);
        job.reason_code = null;
        job.reason_text = null;
        job.updated_at = nowIso();
      }
      state.queue.done = computeDone(state.queue.jobs);
      addHistoryRecord(eligible.length);
      setStatus("Sẵn sàng", "ready");
      renderQueue();
      renderPreview();
      renderHomeSummary();
      const folder = state.settings.output_dir || DEMO_FOLDER_OUT;
      toast(`Đã xuất ${formatNumber(eligible.length)} ảnh vào ${folder} (mô phỏng).`, {
        tone: "success",
        title: "Hoàn tất xuất ảnh",
        timeout: 6200,
      });
    }, 680);
  }

  function reprocessSelected() {
    const job = findJob(state.preview.jobId);
    if (!job) return;

    if (hasTauri) {
      toast(
        "Backend chưa có lệnh xử lý lại trong CONTRACT.md. Thay đổi trên khung cắt được giữ trên giao diện.",
        { tone: "warn", title: "Chưa hỗ trợ" }
      );
      return;
    }

    job.status = "DANG_XU_LY";
    job.reason_code = null;
    job.reason_text = null;
    job.updated_at = nowIso();
    applyJobUpdate(job);
    setStatus("Đang xử lý lại…", "busy");

    window.setTimeout(() => {
      job.status = "DAT";
      job.reason_code = null;
      job.reason_text = null;
      job.updated_at = nowIso();
      applyJobUpdate(job);
      setStatus("Sẵn sàng", "ready");
      renderPreview();
      toast(`Đã xử lý lại ${job.file_name} theo thiết lập hiện tại (mô phỏng).`, {
        tone: "success",
      });
    }, 720);
  }

  function removeSelectedJob() {
    const job = findJob(state.preview.jobId);
    if (!job) return;

    state.queue.jobs = state.queue.jobs.filter((item) => item.id !== job.id);
    state.queue.total = state.queue.jobs.length;
    state.queue.done = computeDone(state.queue.jobs);
    const image = state.jobImages.get(job.id);
    if (image && image.objectUrl) URL.revokeObjectURL(image.url);
    state.jobImages.delete(job.id);
    state.crops.delete(job.id);

    const next = state.queue.jobs[0] || null;
    state.selectedJobId = next ? next.id : null;
    state.preview.jobId = next ? next.id : null;
    state.preview.url = null;
    state.preview.imgLoaded = false;

    renderAll();
    if (state.queue.jobs.length === 0) setStatus("Sẵn sàng", "ready");
    toast(`Đã loại ${job.file_name} khỏi lô xuất.`, { tone: "info" });
  }

  function openOutputFolder(path) {
    if (!path) {
      toast("Chưa có thư mục đầu ra để mở.", { tone: "warn" });
      return;
    }
    if (hasTauri) {
      toast(
        `Thư mục đầu ra: ${path}. Backend chưa có lệnh mở thư mục trong CONTRACT.md.`,
        { tone: "info", timeout: 6200 }
      );
      return;
    }
    toast(`Đã mở thư mục (mô phỏng): ${path}`, { tone: "info" });
  }

  /* ------------------------------------------------------------------------
     Cài đặt
     ------------------------------------------------------------------------ */

  function fillPresetSelects() {
    const options = PRESET_ORDER.map((id) => ({
      value: id,
      label: id === "tuy-chinh" ? "Tùy chỉnh (nhập mm)" : PRESET_DEFS[id].name,
    }));
    fillSelect(el.presetSelect, options, state.settings.default_preset);
    fillSelect(el.setDefaultPreset, options, state.settings.default_preset);
  }

  function applyDefaultBg(hexValue, options = {}) {
    const hex = normalizeHex(hexValue);
    if (!hex) {
      toast("Mã màu không hợp lệ. Vui lòng nhập dạng #RRGGBB.", {
        tone: "warn",
        title: "Màu nền chưa đúng",
      });
      return false;
    }
    state.settings.default_bg_hex = hex;
    state.ui.custom_bg_hex = hex;
    if (options.persist !== false) {
      saveSettingsToBackend({ quiet: true });
      persistUi();
    }
    syncHomeForm();
    renderSettingsForm();
    drawPreviewResult();
    return true;
  }

  async function saveSettingsFromForm() {
    const bgHex = normalizeHex(el.setDefaultBgHex.value);
    if (!bgHex) {
      toast("Mã HEX màu nền không hợp lệ. Ví dụ đúng: #1E5AA8.", {
        tone: "error",
        title: "Chưa lưu được",
      });
      el.setDefaultBgHex.focus();
      return;
    }

    const customHex = normalizeHex(el.setCustomBgHex.value) || state.ui.custom_bg_hex;
    const nameTemplate = el.setNameTemplate.value.trim() || DEFAULT_UI.name_template;

    state.settings = normalizeSettings({
      ...state.settings,
      default_preset: el.setDefaultPreset.value,
      default_bg_hex: bgHex,
      cpu_limit_percent: Number(el.setCpuLimit.value),
      poll_interval_secs: Number(el.setPollInterval.value),
      pause_when_busy: el.setPauseBusy.checked,
      pause_on_battery: el.setPauseBattery.checked,
      remove_original_after_export: el.setRemoveOriginal.checked,
      secure_delete: el.setSecureDelete.checked,
      local_log_enabled: el.setLogEnabled.checked,
    });

    state.ui = normalizeUi({
      ...state.ui,
      export_format: el.setExportFormat.value,
      name_template: nameTemplate,
      start_with_windows: el.setStartWithWindows.checked,
      minimize_to_tray: el.setMinimizeToTray.checked,
      thread_limit: Number(el.setThreadLimit.value),
      matte_accuracy: Number(el.setMatteAccuracy.value),
      align_head: el.setAlignHead.checked,
      eye_level_percent: Number(el.setEyeLevel.value),
      custom_bg_hex: customHex,
    });

    const saved = await saveSettingsToBackend();
    if (!saved) return;
    persistUi();
    renderAll();
    toast("Đã lưu cài đặt cho lô ảnh tiếp theo.", { tone: "success" });
  }

  function resetSettings() {
    const confirmed = window.confirm(
      "Khôi phục các tùy chọn về mặc định? Thư mục đã chọn và xác nhận dữ liệu sẽ được giữ lại."
    );
    if (!confirmed) return;

    const keep = {
      input_dir: state.settings.input_dir,
      output_dir: state.settings.output_dir,
      minor_consent_at: state.settings.minor_consent_at,
      minor_consent_version: state.settings.minor_consent_version,
    };
    state.settings = normalizeSettings({ ...DEFAULT_SETTINGS, ...keep });
    state.ui = normalizeUi(DEFAULT_UI);
    saveSettingsToBackend({ quiet: true });
    persistUi();
    renderAll();
    toast("Đã khôi phục tùy chọn mặc định.", { tone: "success" });
  }

  /* ------------------------------------------------------------------------
     Chuyển màn hình
     ------------------------------------------------------------------------ */

  const VIEW_META = {
    home: {
      title: "Trang chủ",
      subtitle: "Chọn thư mục, chọn preset rồi bấm Bắt đầu xử lý.",
    },
    history: {
      title: "Lịch sử",
      subtitle: "Các lô ảnh đã xuất, ngày giờ và thư mục đầu ra.",
    },
    settings: {
      title: "Cài đặt",
      subtitle: "Tùy chỉnh thư mục, preset, hiệu năng và quyền riêng tư.",
    },
    about: {
      title: "Giới thiệu",
      subtitle: "Phiên bản, giấy phép thành phần và cam kết quyền riêng tư.",
    },
  };

  function switchView(name) {
    if (!VIEW_META[name]) return;
    state.view = name;
    qsa(".nav-item").forEach((button) => {
      button.classList.toggle("is-active", button.dataset.view === name);
    });
    qsa(".view").forEach((view) => view.classList.add("hidden"));
    const view = el[`view-${name}`];
    if (view) view.classList.remove("hidden");
    announceTitle(VIEW_META[name].title, VIEW_META[name].subtitle);

    if (name === "history") renderHistory();
    if (name === "settings") renderSettingsForm();
    if (name === "about") renderAbout();
    if (name === "home") {
      syncHomeForm();
      renderQueue();
      renderPreview();
    }
    el.workspace.scrollTop = 0;
  }

  /* ------------------------------------------------------------------------
     Wizard
     ------------------------------------------------------------------------ */

  function createWizardController() {
    if (!window.PhotoKhanWizard || typeof window.PhotoKhanWizard.create !== "function") {
      return {
        open: () =>
          toast("Không tải được hướng dẫn lần đầu. Vui lòng kiểm tra tệp wizard.js.", {
            tone: "error",
          }),
      };
    }

    return window.PhotoKhanWizard.create({
      presets: presetOptions().filter((preset) => preset.id !== "tuy-chinh"),
      consentVersion: CONSENT_VERSION,
      defaults: {
        inputDir: state.settings.input_dir || DEMO_FOLDER_IN,
        outputDir: state.settings.output_dir || DEMO_FOLDER_OUT,
        preset:
          state.settings.default_preset === "tuy-chinh"
            ? "3x4"
            : state.settings.default_preset,
        consentAt: state.settings.minor_consent_at,
        consentVersion: state.settings.minor_consent_version,
      },
      chooseFolder: async (kind) => chooseFolder(kind),
      toast,
      onFinish: async (data) => {
        const at = nowIso();
        const next = {
          ...state.settings,
          input_dir: data.inputDir || state.settings.input_dir,
          output_dir: data.outputDir || state.settings.output_dir,
          default_preset: data.preset || state.settings.default_preset,
          minor_consent_at: at,
          minor_consent_version: CONSENT_VERSION,
        };
        const presetBg = PRESET_DEFS[data.preset] && PRESET_DEFS[data.preset].bg;
        if (presetBg) next.default_bg_hex = presetBg;

        if (hasTauri) {
          await invokeCommand("confirm_minor_consent", { version: CONSENT_VERSION });
          await invokeCommand("set_settings", { settings: clone(next) });
        }

        state.settings = normalizeSettings(next);
        state.ui.wizard_done = true;
        persistUi();
        if (!hasTauri) saveJson(STORAGE_SETTINGS, state.settings);

        renderAll();
        setStatus("Sẵn sàng", "ready");
        toast("Đã hoàn tất thiết lập ban đầu. Bạn có thể chọn ảnh và bắt đầu.", {
          tone: "success",
          title: "Sẵn sàng",
          timeout: 5600,
        });
      },
    });
  }

  function openWizard(step = 0) {
    if (el.toastArea) el.toastArea.replaceChildren();
    if (state.wizard && typeof state.wizard.open === "function") {
      state.wizard.open(step);
    }
  }

  /* ------------------------------------------------------------------------
     Gắn sự kiện
     ------------------------------------------------------------------------ */

  function bindSwatches(container, scope) {
    if (!container) return;
    container.addEventListener("click", (event) => {
      const button = event.target.closest("[data-hex]");
      if (!button) return;
      const hex = button.dataset.hex;
      if (scope === "settings") {
        el.setDefaultBgHex.value = hex;
        applyDefaultBg(hex, { persist: false });
      } else {
        applyDefaultBg(hex);
      }
    });
  }

  function bindEvents() {
    qsa(".nav-item").forEach((button) => {
      button.addEventListener("click", () => switchView(button.dataset.view));
    });

    el.btnChooseImages.addEventListener("click", (event) => {
      event.preventDefault();
      chooseInputFolder();
    });
    el.btnInputDir.addEventListener("click", chooseInputFolder);
    el.btnSetInputDir.addEventListener("click", chooseInputFolder);
    el.btnOutputDir.addEventListener("click", chooseOutputFolder);
    el.btnSetOutputDir.addEventListener("click", chooseOutputFolder);

    el.filePicker.addEventListener("change", (event) => {
      handleLocalFiles(event.target.files);
      event.target.value = "";
    });

    el.btnLoadDemo.addEventListener("click", () => {
      if (
        state.queue.jobs.length > 0 &&
        !window.confirm("Dữ liệu mẫu sẽ thay thế hàng đợi hiện tại. Tiếp tục?")
      ) {
        return;
      }
      createDemoJobs();
      state.preview.jobId = state.selectedJobId;
      state.preview.url = null;
      state.preview.imgLoaded = false;
      renderAll();
      setStatus("Sẵn sàng", "ready");
      toast("Đã nạp 8 ảnh mẫu. Bấm Bắt đầu xử lý để xem mô phỏng.", {
        tone: "info",
        title: "Dữ liệu mẫu",
      });
    });
    el.btnClearDemo.addEventListener("click", () => clearDemoQueue());

    el.presetSelect.addEventListener("change", () => {
      state.settings.default_preset = el.presetSelect.value;
      const preset = PRESET_DEFS[state.settings.default_preset];
      if (preset && preset.id !== "tuy-chinh") {
        state.settings.default_bg_hex = preset.bg;
      }
      if (state.preview.jobId) clampCrop(getCrop(state.preview.jobId));
      renderAll();
      scheduleSettingsSave();
    });

    bindSwatches(el.bgSwatches, "home");
    bindSwatches(el.previewSwatches, "preview");
    bindSwatches(el.setBgSwatches, "settings");

    el.btnApplySetBg.addEventListener("click", () => {
      if (applyDefaultBg(el.setDefaultBgHex.value)) {
        toast("Đã áp dụng màu nền mặc định.", { tone: "success" });
      }
    });
    el.btnUseCustomBg.addEventListener("click", () => {
      if (applyDefaultBg(el.setCustomBgHex.value)) {
        toast("Đã dùng màu nền tùy chỉnh.", { tone: "success" });
      }
    });

    el.outfitToggle.addEventListener("change", () => {
      state.ui.outfit_enabled = el.outfitToggle.checked;
      persistUi();
      renderPreview();
      syncHomeForm();
    });
    el.outfitSelect.addEventListener("change", () => {
      state.ui.selected_outfit = el.outfitSelect.value;
      persistUi();
      renderPreview();
    });
    el.previewOutfit.addEventListener("change", () => {
      state.ui.selected_outfit = el.previewOutfit.value;
      if (!state.ui.outfit_enabled) state.ui.outfit_enabled = true;
      persistUi();
      syncHomeForm();
      renderPreview();
    });

    const updateCustomPreset = () => {
      state.ui.custom_width_mm = clamp(Number(el.customWidth.value) || 35, 10, 150);
      state.ui.custom_height_mm = clamp(Number(el.customHeight.value) || 45, 10, 200);
      state.ui.face_ratio = clamp(Number(el.faceRatio.value) || 75, 50, 85);
      el.faceRatioOut.textContent = `${state.ui.face_ratio}%`;
      const preset = getCustomPreset();
      el.presetHint.textContent = `${preset.size} · ${preset.px[0]} × ${preset.px[1]} px · nền theo màu đã chọn.`;
      el.presetNoteText.textContent = `${preset.note} Tỷ lệ mặt đặt ở ${state.ui.face_ratio}%.`;
      if (state.preview.jobId) {
        clampCrop(getCrop(state.preview.jobId));
        renderCropBox();
        drawPreviewResult();
      }
      persistUi();
    };
    el.customWidth.addEventListener("input", updateCustomPreset);
    el.customHeight.addEventListener("input", updateCustomPreset);
    el.faceRatio.addEventListener("input", updateCustomPreset);

    el.zoomRange.addEventListener("input", () => setCropZoom(el.zoomRange.value));
    el.matteToggle.addEventListener("change", () => {
      state.preview.matte = el.matteToggle.checked;
      el.matteHint.textContent = state.preview.matte ? "Đang bật" : "Đang tắt";
    });

    el.btnReprocess.addEventListener("click", reprocessSelected);
    el.btnRemoveJob.addEventListener("click", removeSelectedJob);

    el.btnStartQueue.addEventListener("click", () => {
      if (isProcessing() && !state.queue.paused) pauseQueueAction();
      else startQueueAction();
    });
    el.btnPauseQueue.addEventListener("click", pauseQueueAction);
    el.btnExportBatch.addEventListener("click", exportBatch);

    el.queueBody.addEventListener("click", (event) => {
      const row = event.target.closest("tr[data-job-id]");
      if (!row) return;
      selectJob(row.dataset.jobId);
    });

    el.originalImg.addEventListener("load", handleOriginalLoaded);
    el.originalImg.addEventListener("error", () => {
      state.preview.imgLoaded = false;
      el.cropWrap.classList.add("hidden");
      el.stageEmpty.classList.remove("hidden");
      el.originalSize.textContent = "";
    });

    el.btnClearHistory.addEventListener("click", () => {
      if (state.history.length === 0) return;
      if (!window.confirm("Xóa toàn bộ lịch sử xuất ảnh trên giao diện?")) return;
      state.history = [];
      persistHistory();
      renderHistory();
      toast("Đã xóa lịch sử xuất ảnh trên giao diện.", { tone: "info" });
    });

    el.historyList.addEventListener("click", (event) => {
      const button = event.target.closest("[data-history-action]");
      if (!button) return;
      const item = button.closest("[data-history-id]");
      if (!item) return;
      const record = state.history.find((entry) => entry.id === item.dataset.historyId);
      if (record) openOutputFolder(record.output_dir);
    });

    qsa("[data-settings-tab]").forEach((button) => {
      button.addEventListener("click", () => {
        const tab = button.dataset.settingsTab;
        qsa("[data-settings-tab]").forEach((item) => {
          const active = item === button;
          item.classList.toggle("is-active", active);
          item.setAttribute("aria-selected", String(active));
        });
        qsa("[data-settings-panel]").forEach((panel) => {
          panel.classList.toggle("hidden", panel.dataset.settingsPanel !== tab);
        });
      });
    });

    el.btnSaveSettings.addEventListener("click", saveSettingsFromForm);
    el.btnResetSettings.addEventListener("click", resetSettings);
    el.btnReopenWizardSettings.addEventListener("click", () => openWizard(0));
    el.btnReopenWizardAbout.addEventListener("click", () => openWizard(0));

    const bindOutput = (input, output, formatter) => {
      const update = () => {
        output.textContent = formatter(Number(input.value));
      };
      input.addEventListener("input", update);
      update();
    };
    bindOutput(el.setCpuLimit, el.setCpuLimitOut, (value) => `${value}%`);
    bindOutput(el.setThreadLimit, el.setThreadLimitOut, (value) =>
      value === 0 ? "Tự động" : `${value} luồng`
    );
    bindOutput(el.setMatteAccuracy, el.setMatteOut, (value) => String(value));
    bindOutput(el.setEyeLevel, el.setEyeLevelOut, (value) => `${value}%`);
    el.setRemoveOriginal.addEventListener("change", () => {
      el.removeOriginalWarning.classList.toggle("is-visible", el.setRemoveOriginal.checked);
    });

    el.btnChooseOutfitTemplate.addEventListener("click", () => {
      const picker = document.createElement("input");
      picker.type = "file";
      picker.accept = "image/png";
      picker.addEventListener("change", () => {
        const file = picker.files && picker.files[0];
        if (!file) return;
        if (!/\.png$/i.test(file.name)) {
          toast("Chỉ nhận file PNG có kênh alpha cho template áo.", {
            tone: "warn",
          });
          return;
        }
        state.ui.outfit_template_name = file.name;
        persistUi();
        renderSettingsForm();
        toast(`Đã ghi nhận template ${file.name} trên giao diện.`, {
          tone: "success",
        });
      });
      picker.click();
    });

    el.btnOpenLogs.addEventListener("click", () => {
      toast("Chức năng mở thư mục log sẽ hoạt động khi backend cung cấp lệnh tương ứng.", {
        tone: "info",
      });
    });
    el.btnDeleteLogs.addEventListener("click", () => {
      if (!window.confirm("Xóa toàn bộ log chẩn đoán cục bộ?")) return;
      toast("Đã ghi nhận yêu cầu xóa log. Backend sẽ xử lý khi có lệnh tương ứng.", {
        tone: "info",
      });
    });

    document.addEventListener("keydown", (event) => {
      if (el.wizardLayer && !el.wizardLayer.classList.contains("hidden")) return;
      const target = event.target;
      if (
        target &&
        (target.matches("input, select, textarea, [contenteditable='true']") ||
          target.tagName === "BUTTON")
      ) {
        return;
      }
      if (!state.preview.jobId) return;

      if (event.key === "ArrowLeft") {
        event.preventDefault();
        moveCropBy(-0.012, 0);
      } else if (event.key === "ArrowRight") {
        event.preventDefault();
        moveCropBy(0.012, 0);
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        moveCropBy(0, -0.012);
      } else if (event.key === "ArrowDown") {
        event.preventDefault();
        moveCropBy(0, 0.012);
      } else if (event.key === "Enter") {
        event.preventDefault();
        exportBatch();
      }
    });

    let resizeTimer = null;
    window.addEventListener("resize", () => {
      window.clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(() => {
        renderCropBox();
      }, 90);
    });

    window.addEventListener("beforeunload", () => {
      revokeJobImages();
    });
  }

  /* ------------------------------------------------------------------------
     Sự kiện Tauri
     ------------------------------------------------------------------------ */

  async function setupTauriListeners() {
    if (!hasTauri) return;
    const eventApi = window.__TAURI__.event;
    if (!eventApi || typeof eventApi.listen !== "function") return;
    try {
      await eventApi.listen("job_updated", (event) => {
        applyJobUpdate(event.payload);
      });
      await eventApi.listen("batch_completed", (event) => {
        handleBatchCompleted(event.payload);
      });
    } catch (error) {
      showAppError(error, "Không đăng ký được sự kiện từ backend.");
    }
  }

  function updateBackendBadge() {
    if (hasTauri) {
      el.backendBadge.textContent = "Đã kết nối backend";
      el.backendBadge.classList.add("is-live");
      el.backendBadge.title = "Đang gọi lệnh Tauri theo CONTRACT.md";
    } else {
      el.backendBadge.textContent = "Chế độ mô phỏng";
      el.backendBadge.classList.remove("is-live");
      el.backendBadge.title = "Chưa có backend: dùng dữ liệu giả lập trên trình duyệt";
    }
  }

  /* ------------------------------------------------------------------------
     Khởi động
     ------------------------------------------------------------------------ */

  async function init() {
    updateBackendBadge();
    await loadSettingsFromBackend();
    fillPresetSelects();

    state.wizard = createWizardController();
    bindEvents();
    bindCropDrag();

    if (hasTauri) {
      await setupTauriListeners();
      await refreshQueueState();
    } else {
      createDemoJobs();
      state.preview.jobId = state.selectedJobId;
      state.preview.url = null;
      state.preview.imgLoaded = false;
    }

    renderAll();
    setStatus("Sẵn sàng", "ready");

    if (!state.settings.minor_consent_at) {
      window.setTimeout(() => openWizard(0), prefersReducedMotion ? 0 : 420);
    }
  }

  init().catch((error) => {
    showAppError(error, "Không khởi động được giao diện.");
  });
})();



