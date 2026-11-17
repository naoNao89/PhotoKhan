/* ==========================================================================
   PhotoKhan – wizard lần đầu (4 bước)
   Phụ thuộc: vendor/lottie.min.js (có fallback SVG nếu không tải được JSON).
   ========================================================================== */

(() => {
  "use strict";

  const STEP_COUNT = 4;

  const dom = {};
  let options = null;
  let currentStep = 0;
  let data = {
    consent: false,
    inputDir: "",
    outputDir: "",
    preset: "3x4",
  };
  let guideAnimation = null;
  let guideTimer = null;
  let lastFocused = null;
  let finishing = false;

  function byId(id) {
    return document.getElementById(id);
  }

  function query(selector, root = document) {
    return root.querySelector(selector);
  }

  function queryAll(selector, root = document) {
    return Array.from(root.querySelectorAll(selector));
  }

  function escapeHtml(value) {
    return String(value == null ? "" : value)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#039;");
  }

  function formatDateTime(iso) {
    if (!iso) return "";
    const date = new Date(iso);
    if (Number.isNaN(date.getTime())) return iso;
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

  function notify(message, optionsToast = {}) {
    if (dom.wizardLayer && !dom.wizardLayer.classList.contains("hidden")) {
      showWizardAlert(message, optionsToast.tone);
      return;
    }
    if (options && typeof options.toast === "function") {
      options.toast(message, optionsToast);
    }
  }

  function showWizardAlert(message, tone = "warn") {
    if (!dom.wizardAlert) return;
    dom.wizardAlert.textContent = String(message || "");
    dom.wizardAlert.dataset.tone = tone === "error" ? "error" : "warn";
    dom.wizardAlert.classList.remove("hidden");
  }

  function hideWizardAlert() {
    if (!dom.wizardAlert) return;
    dom.wizardAlert.classList.add("hidden");
    dom.wizardAlert.textContent = "";
  }

  /* ------------------------------------------------------------------------
     Khởi tạo
     ------------------------------------------------------------------------ */

  function create(createOptions) {
    options = createOptions || {};

    for (const id of [
      "wizardLayer",
      "wizardClose",
      "wizardSteps",
      "wizardContent",
      "consentCheck",
      "consentError",
      "consentMeta",
      "wizInputDir",
      "wizOutputDir",
      "wizChooseInput",
      "wizChooseOutput",
      "folderError",
      "wizPresetList",
      "wizardPresetError",
      "wizardAlert",
      "wizardLottie",
      "guideFallback",
      "wizardStepLabel",
      "wizardBack",
      "wizardNext",
      "wizardNextLabel",
    ]) {
      dom[id] = byId(id);
    }

    buildPresetCards();
    bindEvents();
    resetData();

    return {
      open,
      close,
      isOpen: () => dom.wizardLayer && !dom.wizardLayer.classList.contains("hidden"),
    };
  }

  function resetData() {
    const defaults = (options && options.defaults) || {};
    data = {
      consent: Boolean(defaults.consentAt),
      inputDir: defaults.inputDir || "",
      outputDir: defaults.outputDir || "",
      preset: defaults.preset || "3x4",
    };

    if (dom.wizInputDir) dom.wizInputDir.value = data.inputDir;
    if (dom.wizOutputDir) dom.wizOutputDir.value = data.outputDir;
    if (dom.consentCheck) dom.consentCheck.checked = data.consent;

    const consentBox = dom.consentCheck && dom.consentCheck.closest(".consent-box");
    if (consentBox) consentBox.classList.toggle("is-checked", data.consent);

    if (dom.consentMeta) {
      if (defaults.consentAt) {
        dom.consentMeta.classList.remove("hidden");
        dom.consentMeta.textContent = `Đã xác nhận trước đó lúc ${formatDateTime(
          defaults.consentAt
        )} · Phiên bản nội dung ${defaults.consentVersion || "—"}.`;
      } else {
        dom.consentMeta.classList.add("hidden");
        dom.consentMeta.textContent = "";
      }
    }

    hideError(dom.consentError);
    hideError(dom.folderError);
    hideError(dom.wizardPresetError);
    hideWizardAlert();
    selectPreset(data.preset);
  }

  /* ------------------------------------------------------------------------
     Preset
     ------------------------------------------------------------------------ */

  function buildPresetCards() {
    if (!dom.wizPresetList) return;
    const presets =
      options && Array.isArray(options.presets) && options.presets.length > 0
        ? options.presets
        : [
            {
              id: "3x4",
              name: "Ảnh 3×4 cm",
              size: "3 × 4 cm",
              px: [354, 472],
              face: "70–80%",
              desc: "Học sinh, sinh viên, bằng lái xe.",
            },
          ];

    dom.wizPresetList.innerHTML = presets
      .map(
        (preset) => `
        <label class="preset-card" data-preset-id="${escapeHtml(preset.id)}">
          <input type="radio" name="wizardPreset" value="${escapeHtml(preset.id)}" />
          <span class="preset-radio" aria-hidden="true"></span>
          <span class="preset-card-body">
            <strong>${escapeHtml(preset.name)}</strong>
            <span>${escapeHtml(preset.size)} · ${preset.px[0]} × ${preset.px[1]} px</span>
            <span>${escapeHtml(preset.desc || "")}</span>
            <em>Mặt chiếm ${escapeHtml(preset.face || "70–80%")}</em>
          </span>
        </label>
      `
      )
      .join("");

    dom.wizPresetList.addEventListener("change", (event) => {
      if (event.target.name !== "wizardPreset") return;
      data.preset = event.target.value;
      hideError(dom.wizardPresetError);
      hideWizardAlert();
      updatePresetSelection();
    });
  }

  function selectPreset(id) {
    if (!dom.wizPresetList) return;
    if (!id) return;
    const input = qsInput(id);
    if (input) input.checked = true;
    data.preset = input ? id : data.preset;
    updatePresetSelection();
  }

  function qsInput(id) {
    return query(`input[name="wizardPreset"][value="${cssEscape(id)}"]`, dom.wizPresetList);
  }

  function cssEscape(value) {
    if (window.CSS && typeof window.CSS.escape === "function") {
      return window.CSS.escape(String(value));
    }
    return String(value).replace(/["\\]/g, "\\$&");
  }

  function updatePresetSelection() {
    queryAll(".preset-card", dom.wizPresetList).forEach((card) => {
      const input = query("input", card);
      card.classList.toggle("is-selected", Boolean(input && input.checked));
    });
  }

  /* ------------------------------------------------------------------------
     Điều hướng
     ------------------------------------------------------------------------ */

  function bindEvents() {
    dom.wizardClose.addEventListener("click", close);
    dom.wizardBack.addEventListener("click", goBack);
    dom.wizardNext.addEventListener("click", goNext);

    dom.consentCheck.addEventListener("change", () => {
      data.consent = dom.consentCheck.checked;
      const box = dom.consentCheck.closest(".consent-box");
      if (box) box.classList.toggle("is-checked", data.consent);
      if (data.consent) hideError(dom.consentError);
      updateFooter();
    });

    dom.wizChooseInput.addEventListener("click", async () => {
      const path = await chooseFolder("input");
      if (!path) return;
      data.inputDir = path;
      dom.wizInputDir.value = path;
      hideError(dom.folderError);
    });

    dom.wizChooseOutput.addEventListener("click", async () => {
      const path = await chooseFolder("output");
      if (!path) return;
      data.outputDir = path;
      dom.wizOutputDir.value = path;
      hideError(dom.folderError);
    });

    dom.wizardLayer.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        close();
      }
      if (event.key === "Tab") {
        trapFocus(event);
      }
    });

    // Nhấn Enter trong wizard không làm xuất ảnh ở lớp phía sau.
    dom.wizardLayer.addEventListener("keypress", (event) => {
      if (event.key === "Enter") event.stopPropagation();
    });
  }

  async function chooseFolder(kind) {
    if (!options || typeof options.chooseFolder !== "function") return "";
    try {
      const path = await options.chooseFolder(kind);
      return typeof path === "string" ? path : "";
    } catch (error) {
      notify(error && error.message ? error.message : "Không chọn được thư mục.", {
        tone: "error",
      });
      return "";
    }
  }

  function open(step = 0) {
    if (!dom.wizardLayer) return;
    lastFocused = document.activeElement;
    resetData();
    currentStep = Math.max(0, Math.min(STEP_COUNT - 1, Number(step) || 0));
    renderStep();
    dom.wizardLayer.classList.remove("hidden");
    dom.wizardLayer.setAttribute("aria-hidden", "false");
    document.body.classList.add("wizard-open");

    window.setTimeout(() => {
      if (currentStep === 0) dom.consentCheck.focus();
      else dom.wizardClose.focus();
    }, 60);
  }

  function close() {
    if (!dom.wizardLayer) return;
    destroyGuideAnimation();
    hideWizardAlert();
    hideError(dom.wizardPresetError);
    dom.wizardLayer.classList.add("hidden");
    dom.wizardLayer.setAttribute("aria-hidden", "true");
    document.body.classList.remove("wizard-open");
    if (lastFocused && typeof lastFocused.focus === "function") {
      lastFocused.focus({ preventScroll: true });
    }
  }

  function goBack() {
    if (currentStep <= 0) return;
    currentStep -= 1;
    renderStep();
  }

  async function goNext() {
    if (finishing) return;

    if (currentStep === 0) {
      if (!dom.consentCheck.checked) {
        showError(dom.consentError);
        dom.consentCheck.focus();
        return;
      }
      data.consent = true;
    }

    if (currentStep === 1) {
      data.inputDir = dom.wizInputDir.value.trim();
      data.outputDir = dom.wizOutputDir.value.trim();
      if (!data.inputDir || !data.outputDir) {
        showError(dom.folderError);
        if (!data.inputDir) dom.wizChooseInput.focus();
        else dom.wizChooseOutput.focus();
        return;
      }
      hideError(dom.folderError);
    }

    if (currentStep === 2) {
      const checked = query('input[name="wizardPreset"]:checked', dom.wizPresetList);
      if (!checked) {
        showError(dom.wizardPresetError);
        const firstInput = query('input[name="wizardPreset"]', dom.wizPresetList);
        if (firstInput) firstInput.focus();
        return;
      }
      data.preset = checked.value;
    }

    if (currentStep >= STEP_COUNT - 1) {
      await finish();
      return;
    }

    currentStep += 1;
    renderStep();
  }

  async function finish() {
    if (!options || typeof options.onFinish !== "function") {
      close();
      return;
    }

    finishing = true;
    const previousLabel = dom.wizardNextLabel.textContent;
    dom.wizardNext.disabled = true;
    dom.wizardNextLabel.textContent = "Đang lưu…";

    try {
      await options.onFinish({
        consent: true,
        inputDir: data.inputDir,
        outputDir: data.outputDir,
        preset: data.preset,
        consentVersion: options.consentVersion || "1.0",
      });
      finishing = false;
      close();
    } catch (error) {
      finishing = false;
      const message =
        (error && (error.message_vi || error.message)) ||
        "Không lưu được thiết lập. Vui lòng thử lại.";
      notify(message, { tone: "error", title: "Chưa hoàn tất" });
    } finally {
      finishing = false;
      dom.wizardNext.disabled = false;
      dom.wizardNextLabel.textContent = previousLabel;
    }
  }

  /* ------------------------------------------------------------------------
     Hiển thị bước
     ------------------------------------------------------------------------ */

  function renderStep() {
    queryAll(".wizard-step", dom.wizardContent).forEach((section) => {
      const index = Number(section.dataset.step);
      section.classList.toggle("hidden", index !== currentStep);
    });

    queryAll("[data-step-chip]", dom.wizardSteps).forEach((chip) => {
      const index = Number(chip.dataset.stepChip);
      chip.classList.toggle("is-active", index === currentStep);
      chip.classList.toggle("is-done", index < currentStep);
    });

    dom.wizardStepLabel.textContent = `Bước ${currentStep + 1}/${STEP_COUNT}`;
    dom.wizardBack.disabled = currentStep === 0;
    hideError(dom.wizardPresetError);
    hideWizardAlert();
    updateFooter();

    if (currentStep === 3) {
      initGuideAnimation();
    } else {
      destroyGuideAnimation();
    }

    dom.wizardContent.scrollTop = 0;
  }

  function updateFooter() {
    const isLast = currentStep === STEP_COUNT - 1;
    dom.wizardNextLabel.textContent = isLast ? "Hoàn tất" : "Tiếp tục";
    const nextIcon = query("use", dom.wizardNext);
    if (nextIcon) nextIcon.setAttribute("href", isLast ? "#i-check" : "#i-arrow-right");
    dom.wizardNext.disabled = finishing || (currentStep === 0 && !dom.consentCheck.checked);
  }

  function showError(node) {
    if (!node) return;
    node.classList.remove("hidden");
  }

  function hideError(node) {
    if (!node) return;
    node.classList.add("hidden");
  }

  function trapFocus(event) {
    const focusable = queryAll(
      'button:not(:disabled), input:not(:disabled), select:not(:disabled), [tabindex]:not([tabindex="-1"])',
      dom.wizardLayer
    ).filter((node) => node.offsetParent !== null);

    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];

    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  /* ------------------------------------------------------------------------
     Lottie hướng dẫn
     ------------------------------------------------------------------------ */

  function initGuideAnimation() {
    if (!dom.wizardLottie) return;
    if (guideAnimation) return;

    showGuideFallback(!window.lottie);

    if (!window.lottie) return;

    try {
      guideAnimation = window.lottie.loadAnimation({
        container: dom.wizardLottie,
        renderer: "svg",
        loop: true,
        autoplay: true,
        path: "lottie/huong-dan-chup-anh.json",
        rendererSettings: {
          preserveAspectRatio: "xMidYMid meet",
          progressiveLoad: false,
        },
      });

      const fallback = () => showGuideFallback(true);
      guideAnimation.addEventListener("data_failed", fallback);
      guideAnimation.addEventListener("error", fallback);

      guideTimer = window.setTimeout(() => {
        if (!dom.wizardLottie.querySelector("svg")) fallback();
      }, 2200);
    } catch (error) {
      showGuideFallback(true);
    }
  }

  function showGuideFallback(show) {
    if (!dom.guideFallback) return;
    dom.guideFallback.classList.toggle("is-visible", Boolean(show));
  }

  function destroyGuideAnimation() {
    window.clearTimeout(guideTimer);
    guideTimer = null;
    if (guideAnimation) {
      try {
        guideAnimation.destroy();
      } catch (error) {
        /* Không cần xử lý khi animation đã bị hủy. */
      }
      guideAnimation = null;
    }
    if (dom.wizardLottie) dom.wizardLottie.innerHTML = "";
  }

  window.PhotoKhanWizard = { create };
})();
