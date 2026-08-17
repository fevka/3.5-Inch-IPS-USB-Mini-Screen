const invoke = window.__TAURI__.core.invoke;

// ---------------------------------------------------------------------
// small helpers
// ---------------------------------------------------------------------

function $(id) { return document.getElementById(id); }

function debounce(fn, ms) {
  let t = null;
  return (...args) => {
    clearTimeout(t);
    t = setTimeout(() => fn(...args), ms);
  };
}

function setStatus(el, text, kind) {
  el.textContent = text;
  el.classList.remove("error", "ok");
  if (kind) el.classList.add(kind);
}

// ---------------------------------------------------------------------
// tabs
// ---------------------------------------------------------------------

document.querySelectorAll(".tab-btn").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".tab-btn").forEach((b) => b.classList.remove("active"));
    document.querySelectorAll(".tab-panel").forEach((p) => p.classList.remove("active"));
    btn.classList.add("active");
    $("tab-" + btn.dataset.tab).classList.add("active");
    if (btn.dataset.tab === "theme") {
      // CodeMirror needs a refresh once its container becomes visible,
      // otherwise it renders collapsed to zero height.
      setTimeout(() => editor && editor.refresh(), 0);
    }
  });
});

const globalStatus = $("status-line");

// ---------------------------------------------------------------------
// General tab
// ---------------------------------------------------------------------

let allThemes = [];

async function initGeneralTab() {
  try {
    const [cfg, ports, themes, ifaces] = await Promise.all([
      invoke("load_config"),
      invoke("list_ports"),
      invoke("list_themes"),
      invoke("list_interfaces"),
    ]);
    allThemes = themes;
    const activeTheme = cfg.theme || "NexusMeter";

    fillSelect($("com-port"), ["AUTO", ...ports], cfg.com_port);
    fillSelect($("cfg-theme"), themes, activeTheme);
    fillDatalist($("iface-list"), ifaces);

    $("eth").value = cfg.eth;
    $("wlo").value = cfg.wlo;
    $("ping").value = cfg.ping || "8.8.8.8";
    $("weather-lat").value = cfg.weather_latitude;
    $("weather-lon").value = cfg.weather_longitude;
    $("weather-units").value = cfg.weather_units || "metric";
    $("brightness").value = cfg.brightness;
    $("brightness-val").textContent = cfg.brightness;
    $("display-reverse").checked = !!cfg.display_reverse;
    $("reset-on-startup").checked = !!cfg.reset_on_startup;
    $("show-console").checked = !!cfg.show_console;
    $("lhm-path").value = cfg.lhm_path || "";
    $("sensor-interval").value = cfg.sensor_interval_ms != null ? cfg.sensor_interval_ms : 2000;

    // Check startup task status
    invoke("check_startup").then((enabled) => {
      $("run-on-startup").checked = enabled;
    }).catch(() => {
      $("run-on-startup").checked = false;
    });
    $("startup-delay").value = cfg.startup_delay != null ? cfg.startup_delay : 30;

    // Keep the theme editor's theme selector in sync with the initial config.
    fillSelect($("theme-select"), themes, activeTheme);
    await loadTheme(activeTheme);
    loadGeneralPreview(activeTheme);
  } catch (e) {
    setStatus(globalStatus, "Failed to load config.yaml: " + e, "error");
  }
}

async function loadGeneralPreview(theme) {
  if (!theme) return;
  try {
    const dataUrl = await invoke("render_theme_default_preview", { theme });
    $("general-preview-img").src = dataUrl;
  } catch (e) {
    // Non-fatal - theme might have no saved preview yet.
  }
}

$("cfg-theme").addEventListener("change", () => loadGeneralPreview($("cfg-theme").value));

function fillSelect(select, values, selected) {
  select.innerHTML = "";
  for (const v of values) {
    const opt = document.createElement("option");
    opt.value = v;
    opt.textContent = v;
    if (v === selected) opt.selected = true;
    select.appendChild(opt);
  }
  // If the stored value isn't in the list (e.g. saved port unplugged), keep it visible anyway.
  if (selected && !values.includes(selected)) {
    const opt = document.createElement("option");
    opt.value = selected;
    opt.textContent = selected + " (not found)";
    opt.selected = true;
    select.appendChild(opt);
  }
}

function fillDatalist(datalist, values) {
  datalist.innerHTML = "";
  for (const v of values) {
    const opt = document.createElement("option");
    opt.value = v;
    datalist.appendChild(opt);
  }
}

$("refresh-ports").addEventListener("click", async () => {
  const ports = await invoke("list_ports");
  const current = $("com-port").value;
  fillSelect($("com-port"), ["AUTO", ...ports], current);
});

$("brightness").addEventListener("input", () => {
  $("brightness-val").textContent = $("brightness").value;
});

$("save-config").addEventListener("click", async () => {
  const cfg = {
    com_port: $("com-port").value,
    theme: $("cfg-theme").value,
    eth: $("eth").value,
    wlo: $("wlo").value,
    ping: $("ping").value,
    weather_latitude: parseFloat($("weather-lat").value) || 0,
    weather_longitude: parseFloat($("weather-lon").value) || 0,
    weather_units: $("weather-units").value,
    brightness: parseInt($("brightness").value, 10),
    display_reverse: $("display-reverse").checked,
    reset_on_startup: $("reset-on-startup").checked,
    show_console: $("show-console").checked,
    run_on_startup: $("run-on-startup").checked,
    startup_delay: parseInt($("startup-delay").value) || 30,
    lhm_path: $("lhm-path").value || "",
    sensor_interval_ms: parseInt($("sensor-interval").value) || 2000,
  };
  try {
    await invoke("save_config", { cfg });
    setStatus(globalStatus, "config.yaml saved.", "ok");
  } catch (e) {
    setStatus(globalStatus, "Save error: " + e, "error");
  }
});

$("launch-monitor").addEventListener("click", async () => {
  try {
    await invoke("launch_monitor");
    setStatus(globalStatus, "Monitor started.", "ok");
  } catch (e) {
    setStatus(globalStatus, "Launch error: " + e, "error");
  }
});

$("stop-monitor").addEventListener("click", async () => {
  try {
    await invoke("stop_monitor");
    setStatus(globalStatus, "Monitor stopped.", "ok");
  } catch (e) {
    setStatus(globalStatus, "Stop error: " + e, "error");
  }
});

// --- city search (weather) ---

async function runCitySearch() {
  const query = $("city-query").value.trim();
  const resultsEl = $("city-results");
  resultsEl.innerHTML = "";
  if (!query) return;
  try {
    const results = await invoke("search_cities", { query });
    if (results.length === 0) {
      resultsEl.innerHTML = '<div class="city-result">No results found.</div>';
      return;
    }
    for (const r of results) {
      const div = document.createElement("div");
      div.className = "city-result";
      const parts = [r.name, r.admin1, r.country].filter(Boolean);
      div.textContent = parts.join(", ");
      div.addEventListener("click", () => {
        $("weather-lat").value = r.latitude.toFixed(4);
        $("weather-lon").value = r.longitude.toFixed(4);
        $("city-selected").textContent = "Selected: " + parts.join(", ");
        resultsEl.innerHTML = "";
        $("city-query").value = "";
      });
      resultsEl.appendChild(div);
    }
  } catch (e) {
    resultsEl.innerHTML = '<div class="city-result">Search error: ' + e + "</div>";
  }
}

$("city-search-btn").addEventListener("click", runCitySearch);
$("city-query").addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    e.preventDefault();
    runCitySearch();
  }
});

// ---------------------------------------------------------------------
// Theme editor tab
//
// Single source of truth is `yamlText`. Both the CodeMirror editor and
// the visual form read/write through the two sync functions below, so
// they can never drift out of sync with each other the way the old
// app's two independent editor states could.
// ---------------------------------------------------------------------

let currentTheme = null;
let yamlText = "";
let parsedYaml = null; // last successfully parsed object, or null if invalid
let suppressEditorEvent = false;
let currentLayout = null; // {width,height,boxes:[...]}
let selectedKey = null; // joined path string of the selected element
let elementInputsMap = {}; // "path.FIELD" -> <input>
let elementDetailsMap = {}; // "path" -> <details>

function pathKey(path) { return path.join("."); }

const editor = CodeMirror.fromTextArea($("yaml-editor"), {
  mode: "yaml",
  theme: "default",
  lineNumbers: true,
  tabSize: 2,
  indentUnit: 2,
});

const themeStatus = $("theme-status");

async function loadTheme(theme) {
  currentTheme = theme;
  try {
    yamlText = await invoke("load_theme_yaml", { theme });
    suppressEditorEvent = true;
    editor.setValue(yamlText);
    suppressEditorEvent = false;
    syncFromYamlText();
  } catch (e) {
    setStatus(themeStatus, "Load error: " + e, "error");
  }
}

$("theme-select").addEventListener("change", async (e) => {
  await loadTheme(e.target.value);
  loadGeneralPreview(e.target.value);
});

// --- create new theme ---

$("new-theme-btn").addEventListener("click", () => {
  $("new-theme-modal").classList.remove("hidden");
  $("new-theme-name").value = "";
  $("new-theme-bg").value = "";
  $("new-theme-name").focus();
});

$("new-theme-cancel").addEventListener("click", () => {
  $("new-theme-modal").classList.add("hidden");
});

$("new-theme-browse").addEventListener("click", async () => {
  try {
    const path = await invoke("select_image_file");
    if (path) $("new-theme-bg").value = path;
  } catch (e) {
    setStatus(themeStatus, "Image pick error: " + e, "error");
  }
});

$("new-theme-create").addEventListener("click", async () => {
  const name = $("new-theme-name").value.trim();
  const bg = $("new-theme-bg").value.trim();
  if (!name) {
    setStatus(themeStatus, "Theme name is required.", "error");
    return;
  }
  try {
    await invoke("create_theme", { name, backgroundSrc: bg });
    $("new-theme-modal").classList.add("hidden");
    allThemes = await invoke("list_themes");
    populateThemeSelect();
    fillSelect($("cfg-theme"), allThemes, name);
    await loadTheme(name);
    loadGeneralPreview(name);
    setStatus(themeStatus, "Theme '" + name + "' created.", "ok");
  } catch (e) {
    setStatus(themeStatus, "Create error: " + e, "error");
  }
});

function populateThemeSelect() {
  fillSelect($("theme-select"), allThemes, currentTheme);
}

// --- YAML text -> parsed object -> visual form + preview ---

function syncFromYamlText() {
  try {
    parsedYaml = jsyaml.load(yamlText) || {};
    setStatus(themeStatus, "Valid YAML.", "ok");
    renderElementList();
    requestPreview();
  } catch (e) {
    parsedYaml = null;
    setStatus(themeStatus, "YAML error: " + e.message, "error");
    // Deliberately do NOT touch the visual form or preview here - keep
    // showing the last valid state instead of clearing it out from
    // under the user while they're mid-edit.
  }
}

editor.on("change", () => {
  if (suppressEditorEvent) return;
  yamlText = editor.getValue();
  debouncedSyncFromYaml();
});
const debouncedSyncFromYaml = debounce(syncFromYamlText, 250);

// --- visual form edit -> parsed object -> YAML text -> preview ---

function pathGet(obj, path) {
  let cur = obj;
  for (const k of path) {
    if (cur == null) return undefined;
    cur = cur[k];
  }
  return cur;
}

function pathSet(obj, path, value) {
  let cur = obj;
  for (let i = 0; i < path.length - 1; i++) {
    if (cur[path[i]] == null || typeof cur[path[i]] !== "object") cur[path[i]] = {};
    cur = cur[path[i]];
  }
  cur[path[path.length - 1]] = value;
}

function syncFromVisualEdit() {
  if (!parsedYaml) return;
  yamlText = jsyaml.dump(parsedYaml, { lineWidth: -1 });
  suppressEditorEvent = true;
  const cursor = editor.getCursor();
  editor.setValue(yamlText);
  editor.setCursor(cursor);
  suppressEditorEvent = false;
  requestPreview();
}
const debouncedSyncFromVisual = debounce(syncFromVisualEdit, 150);

// --- live preview + click/drag overlay ---

const previewImg = $("preview-img");

async function doPreview() {
  if (!currentTheme) return;
  try {
    const [dataUrl, layout] = await Promise.all([
      invoke("render_preview", { theme: currentTheme, yamlText }),
      invoke("theme_layout", { theme: currentTheme, yamlText }),
    ]);
    currentLayout = layout;
    previewImg.onload = () => { previewImg.onload = null; requestAnimationFrame(drawOverlay); };
    previewImg.src = dataUrl;
    $("general-preview-img").src = dataUrl;
    if (previewImg.complete) requestAnimationFrame(drawOverlay);
  } catch (e) {
    setStatus(themeStatus, "Preview error: " + e, "error");
  }
}

const requestPreview = debounce(doPreview, 200);

// Immediate (non-debounced) preview – used during drag
let previewPending = false;
async function requestPreviewImmediate() {
  if (previewPending) return;
  previewPending = true;
  await doPreview();
  previewPending = false;
}

window.addEventListener("resize", () => { if (currentLayout) drawOverlay(); });

function drawOverlay() {
  const container = $("preview-overlay");
  container.innerHTML = "";
  if (!currentLayout) return;
  if (!previewImg.clientWidth) {
    requestAnimationFrame(drawOverlay);
    return;
  }
  const scaleX = previewImg.clientWidth / currentLayout.width;
  const scaleY = previewImg.clientHeight / currentLayout.height;

  for (const box of currentLayout.boxes) {
    if (box.hidden) continue;
    const key = pathKey(box.path);
    const isSingle = key === selectedKey;
    const div = document.createElement("div");
    div.className = "el-box" + (isSingle ? " selected" : "");
    div.style.left = box.x * scaleX + "px";
    div.style.top = box.y * scaleY + "px";
    div.style.width = Math.max(6, box.w * scaleX) + "px";
    div.style.height = Math.max(6, box.h * scaleY) + "px";
    div.title = key;
    div.addEventListener("mousedown", (e) => startDrag(e, box));
    div.addEventListener("click", (e) => {
      e.stopPropagation();
      selectElement(box.path);
    });
    container.appendChild(div);
  }
}

function highlightSelection(path) {
  const key = pathKey(path);
  selectedKey = key;
  drawOverlay();
  for (const k in elementDetailsMap) {
    elementDetailsMap[k].classList.toggle("selected", k === selectedKey);
  }
  updateAlignToolbarState();
}

function selectElement(path) {
  highlightSelection(path);
  for (const k in elementDetailsMap) {
    elementDetailsMap[k].open = k === selectedKey;
  }
  const details = elementDetailsMap[selectedKey];
  if (details) {
    details.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }
}

// --- dragging an element directly on the preview ---

let dragState = null;

function startDrag(e, box) {
  e.preventDefault();
  e.stopPropagation();
  if (!parsedYaml) return;
  selectElement(box.path);
  const node = pathGet(parsedYaml, box.path);
  if (!node) return;
  const overlay = $("preview-overlay");
  const el = findOverlayBox(box.path);
  // The YAML X/Y is anchor-dependent (e.g. "rt" means right edge), while the
  // overlay box sits at the text's top-left corner. Keep both: YAML values
  // for writing back, box.x/y (top-left) for visually tracking the box.
  dragState = {
    path: box.path,
    startMouseX: e.clientX,
    startMouseY: e.clientY,
    startX: node.X || 0,
    startY: node.Y || 0,
    startBoxX: box.x,
    startBoxY: box.y,
    scaleX: currentLayout.width / previewImg.clientWidth,
    scaleY: currentLayout.height / previewImg.clientHeight,
    overlayEl: el,
  };
  if (el) el.style.cursor = "grabbing";
  window.addEventListener("mousemove", onDragMove);
  window.addEventListener("mouseup", onDragEnd);
}

function findOverlayBox(path) {
  const key = pathKey(path);
  return $("preview-overlay").querySelector(`[title="${key}"]`);
}

function onDragMove(e) {
  if (!dragState) return;
  const dx = Math.round((e.clientX - dragState.startMouseX) * dragState.scaleX);
  const dy = Math.round((e.clientY - dragState.startMouseY) * dragState.scaleY);
  const newX = dragState.startX + dx;
  const newY = dragState.startY + dy;
  const newBoxX = dragState.startBoxX + dx;
  const newBoxY = dragState.startBoxY + dy;
  // Update YAML data for the dragged element.
  const p = dragState.path;
  const node = pathGet(parsedYaml, p);
  if (node) {
    pathSet(parsedYaml, [...p, "X"], newX);
    pathSet(parsedYaml, [...p, "Y"], newY);
    const key = pathKey(p);
    if (elementInputsMap[key + ".X"]) elementInputsMap[key + ".X"].value = newX;
    if (elementInputsMap[key + ".Y"]) elementInputsMap[key + ".Y"].value = newY;
  }
  // Move overlay box locally (no backend call). The box tracks its top-left
  // corner; the YAML X/Y keeps its anchor semantics.
  if (dragState.overlayEl) {
    dragState.overlayEl.style.left = (newBoxX * previewImg.clientWidth / currentLayout.width) + "px";
    dragState.overlayEl.style.top = (newBoxY * previewImg.clientHeight / currentLayout.height) + "px";
  }
}

function onDragEnd() {
  if (dragState && dragState.overlayEl) {
    dragState.overlayEl.style.cursor = "grab";
  }
  dragState = null;
  window.removeEventListener("mousemove", onDragMove);
  window.removeEventListener("mouseup", onDragEnd);
  // Sync preview after drag ends
  syncFromVisualEdit();
}

// --- visual element list ---

// --- element palette (adding NEW elements to a theme) ---

/// Shared field defaults injected into every element added from the
/// palette, so it shows up immediately and is draggable.
function defaultFontFor() {
  // Use the theme's first available font if any, else a safe generic path.
  if (parsedYaml && parsedYaml.static_text) {
    for (const k of Object.keys(parsedYaml.static_text)) {
      const n = parsedYaml.static_text[k];
      if (n && n.FONT) return n.FONT;
    }
  }
  return "generale-mono/GeneraleMonoA.ttf";
}

function themeBgPath() {
  return (parsedYaml &&
    parsedYaml.static_images &&
    parsedYaml.static_images.BACKGROUND &&
    parsedYaml.static_images.BACKGROUND.PATH) || "background.png";
}

function ensureTree(obj, path) {
  let cur = obj;
  for (let i = 0; i < path.length - 1; i++) {
    if (cur[path[i]] == null || typeof cur[path[i]] !== "object") cur[path[i]] = {};
    cur = cur[path[i]];
  }
  if (cur[path[path.length - 1]] == null) cur[path[path.length - 1]] = {};
  return cur[path[path.length - 1]];
}

function uniqueKey(obj, base) {
  if (obj[base] == null) return base;
  let i = 2;
  while (obj[base + "_" + i] != null) i++;
  return base + "_" + i;
}

function addTextElement(category, name, text, x, y, fontSize, color) {
  if (!parsedYaml) return;
  if (!parsedYaml.static_text) parsedYaml.static_text = {};
  const key = uniqueKey(parsedYaml.static_text, name);
  parsedYaml.static_text[key] = {
    TEXT: text,
    X: x,
    Y: y,
    FONT: defaultFontFor(),
    FONT_SIZE: fontSize,
    FONT_COLOR: color,
    BACKGROUND_IMAGE: themeBgPath(),
    ALIGN: "left",
    ANCHOR: "lt",
  };
  syncFromVisualEdit();
  return ["static_text", key];
}

function addTextElementAt(name, text, x, y, fontSize, color) {
  const path = addTextElement(name, name, text, x, y, fontSize, color);
  renderElementList();
  return path;
}

function addStatElement(cat, name) {
  if (!parsedYaml) return;
  if (!parsedYaml.STATS) parsedYaml.STATS = {};
  if (!parsedYaml.STATS[cat]) parsedYaml.STATS[cat] = {};
  const group = parsedYaml.STATS[cat];
  const key = uniqueKey(group, name);
  group[key] = buildStatTemplate(name);
  syncFromVisualEdit();
  return ["STATS", cat, key];
}

function addStatElementAt(cat, name, x, y, fontSize, color, barColor, w, h) {
  if (!parsedYaml) return;
  if (!parsedYaml.STATS) parsedYaml.STATS = {};
  if (!parsedYaml.STATS[cat]) parsedYaml.STATS[cat] = {};
  const group = parsedYaml.STATS[cat];
  const key = uniqueKey(group, name);
  group[key] = buildStatTemplate(name, x, y, fontSize, color, barColor, w, h);
  syncFromVisualEdit();
  renderElementList();
  return ["STATS", cat, key];
}

/// Returns a realistic per-stat template node (with TEXT + GRAPH where
/// appropriate) so newly-added stats show real geometry in the preview.
function buildStatTemplate(name, x, y, fontSize, color, barColor, w, h) {
  x = x != null ? x : 310;
  y = y != null ? y : 20;
  fontSize = fontSize != null ? fontSize : 16;
  color = color != null ? color : "255, 255, 255";
  barColor = barColor != null ? barColor : "107, 203, 255";
  w = w != null ? w : 300;
  h = h != null ? h : 8;
  const bg = themeBgPath();
  const t = { SHOW: true, SHOW_UNIT: true, X: x, Y: y, FONT: defaultFontFor(), FONT_SIZE: fontSize, FONT_COLOR: color, BACKGROUND_IMAGE: bg, ALIGN: "right", ANCHOR: "rt" };
  const g = { SHOW: true, X: x, Y: y + 34, WIDTH: w, HEIGHT: h, MIN_VALUE: 0, MAX_VALUE: 100, BAR_COLOR: barColor, BAR_OUTLINE: false, BACKGROUND_IMAGE: bg };
  switch (name) {
    case "PERCENTAGE":
      return { INTERVAL: 1, TEXT: { ...t }, GRAPH: { ...g } };
    case "FREQUENCY":
      return { INTERVAL: 5, TEXT: { ...t } };
    case "TEMPERATURE":
      return { INTERVAL: 5, TEXT: { ...t } };
    case "MEMORY":
      return { INTERVAL: 5, TEXT: { ...t } };
    case "VIRTUAL":
      return { INTERVAL: 5, GRAPH: { ...g }, PERCENT_TEXT: { ...t }, USED: { ...t } };
    case "UPLOAD":
      return { INTERVAL: 1, TEXT: { ...t } };
    case "DOWNLOAD":
      return { INTERVAL: 1, TEXT: { ...t } };
    case "TEMPERATURE_FELT":
      return { INTERVAL: 300, TEXT: { ...t } };
    case "HUMIDITY":
      return { INTERVAL: 300, TEXT: { ...t } };
    case "WEATHER_DESCRIPTION":
      return { INTERVAL: 300, TEXT: { ...t }, ICON: { SHOW: true, X: x, Y: y + 34, WIDTH: w, HEIGHT: h } };
    default:
      return { INTERVAL: 1, TEXT: { ...t } };
  }
}

function addDateElement(name, fmt, x, y, fontSize, color) {
  if (!parsedYaml) return;
  if (!parsedYaml.DATE) parsedYaml.DATE = {};
  const key = uniqueKey(parsedYaml.DATE, name);
  parsedYaml.DATE[key] = {
    INTERVAL: 1,
    TEXT: {
      FORMAT: fmt,
      SHOW: true,
      X: x != null ? x : 160,
      Y: y != null ? y : 400,
      FONT: defaultFontFor(),
      FONT_SIZE: fontSize != null ? fontSize : 24,
      FONT_COLOR: color != null ? color : "255, 255, 255",
      BACKGROUND_COLOR: "8, 8, 12",
      ALIGN: "center",
      ANCHOR: "mt",
    },
  };
  syncFromVisualEdit();
  return ["DATE", key];
}

function addDateElementAt(name, fmt, x, y, fontSize, color) {
  const path = addDateElement(name, fmt, x, y, fontSize, color);
  renderElementList();
  return path;
}

const PALETTE = [
  { group: "CPU", items: [
    { name: "PERCENTAGE", label: "CPU % (text+bar)", kind: "stat", cat: "CPU", hasBar: true },
    { name: "FREQUENCY", label: "CPU Frequency", kind: "stat", cat: "CPU" },
    { name: "TEMPERATURE", label: "CPU Temp", kind: "stat", cat: "CPU" },
  ]},
  { group: "GPU", items: [
    { name: "PERCENTAGE", label: "GPU % (text+bar)", kind: "stat", cat: "GPU", hasBar: true },
    { name: "MEMORY", label: "GPU Memory", kind: "stat", cat: "GPU" },
    { name: "TEMPERATURE", label: "GPU Temp", kind: "stat", cat: "GPU" },
  ]},
  { group: "MEMORY", items: [
    { name: "VIRTUAL", label: "Memory (bar+text)", kind: "stat", cat: "MEMORY", hasBar: true },
  ]},
  { group: "NET", items: [
    { name: "UPLOAD", label: "Upload speed", kind: "stat", cat: "NET" },
    { name: "DOWNLOAD", label: "Download speed", kind: "stat", cat: "NET" },
  ]},
  { group: "WEATHER", items: [
    { name: "TEMPERATURE", label: "Temp", kind: "stat", cat: "WEATHER" },
    { name: "HUMIDITY", label: "Humidity", kind: "stat", cat: "WEATHER" },
    { name: "TEMPERATURE_FELT", label: "Feels like", kind: "stat", cat: "WEATHER" },
    { name: "WEATHER_DESCRIPTION", label: "Desc + icon", kind: "stat", cat: "WEATHER", hasIcon: true },
  ]},
  { group: "DATE", items: [
    { name: "HOUR", label: "Clock", kind: "date", fmt: "short" },
    { name: "DAY", label: "Day / date", kind: "date", fmt: "short" },
  ]},
  { group: "STATIC TEXT", items: [
    { name: "LABEL", label: "Label (text)", kind: "text", text: "CPU Label" },
    { name: "TITLE", label: "Title (big)", kind: "text", text: "My Title" },
  ]},
];

// ---- add-element popup ----

let addModalType = null; // currently selected PALETTE item

function openAddModal(item) {
  try {
    addModalType = item;
    // Populate type select only once (static), but keep it in sync.
    if ($("add-el-type").options.length === 0) {
      for (const group of PALETTE) {
        const og = document.createElement("optgroup");
        og.label = group.group;
        for (const it of group.items) {
        const opt = document.createElement("option");
        opt.value = JSON.stringify({ name: it.name, cat: it.cat || "", kind: it.kind, hasBar: !!it.hasBar, hasIcon: !!it.hasIcon, fmt: it.fmt || "", text: it.text || "" });
        opt.textContent = it.label;
        og.appendChild(opt);
      }
      $("add-el-type").appendChild(og);
    }
  }
  // Pre-select the clicked item
  const wanted = JSON.stringify({ name: item.name, cat: item.cat || "", kind: item.kind, hasBar: !!item.hasBar, hasIcon: !!item.hasIcon, fmt: item.fmt || "", text: item.text || "" });
  const opts = Array.from($("add-el-type").options);
  const match = opts.find((o) => o.value === wanted);
  if (match) $("add-el-type").value = match.value;

  // Suggest a position that doesn't overlap: place below the lowest
  // existing element, or top-left on empty themes.
  const suggested = suggestFreePosition();
  $("add-el-x").value = suggested.x;
  $("add-el-y").value = suggested.y;
  $("add-el-text").value = item.text || "";
  $("add-el-font-size").value = 16;
  $("add-el-color").value = "#ffffff";
  $("add-el-bar-color").value = "#6bcbff";
  $("add-el-w").value = 300;
  $("add-el-h").value = 8;
  updateAddModalFields();
  $("add-element-modal").classList.remove("hidden");
  } catch (e) {
    setStatus(themeStatus, "Palette error: " + e.message, "error");
    console.error("openAddModal failed", e);
  }
}

function updateAddModalFields() {
  if (!addModalType) return;
  const kind = addModalType.kind;
  const hasBar = addModalType.hasBar;
  const hasIcon = addModalType.hasIcon;
  setFieldVis("add-el-text", kind === "text");
  setFieldVis("add-el-font-size", kind !== "date");
  setFieldVis("add-el-color", kind !== "date");
  setFieldVis("add-el-bar-color", hasBar);
  setFieldVis("add-el-w", hasBar || hasIcon);
  setFieldVis("add-el-h", hasBar || hasIcon);
}

function setFieldVis(id, show) {
  const el = $(id);
  el.closest(".field").style.display = show ? "" : "none";
}

/// Returns a free position that never overlaps existing elements, by
/// scanning parsedYaml directly (not currentLayout, which is debounced
/// and can be stale when the popup opens). Each new element lands
/// below the lowest existing one, so repeated adds form a clean column.
function suggestFreePosition() {
  const SCREEN_H = 480;
  let maxBottom = 20;
  const consider = (x, y, w, h) => {
    maxBottom = Math.max(maxBottom, y + h + 12);
  };
  if (parsedYaml) {
    // STATS: per-group, per-item text/bar/icon nodes.
    if (parsedYaml.STATS && typeof parsedYaml.STATS === "object") {
      for (const cat of Object.values(parsedYaml.STATS)) {
        if (!cat || typeof cat !== "object") continue;
        for (const item of Object.values(cat)) {
          if (!item || typeof item !== "object") continue;
          for (const sub of Object.values(item)) {
            if (!sub || typeof sub !== "object") continue;
            const x = sub.X != null ? sub.X : 0;
            const y = sub.Y != null ? sub.Y : 0;
            const w = sub.WIDTH != null ? sub.WIDTH : 100;
            const h = sub.HEIGHT != null ? sub.HEIGHT : (sub.FONT_SIZE != null ? sub.FONT_SIZE * 1.4 : 20);
            consider(x, y, w, h);
          }
        }
      }
    }
    // static_text and DATE.TEXT and static_images.
    for (const [section, isStat] of [["static_text", false], ["DATE", false], ["static_images", false]]) {
      const root = parsedYaml[section];
      if (!root || typeof root !== "object") continue;
      const items = section === "DATE" ? [root] : Object.values(root);
      for (const item of items) {
        if (!item || typeof item !== "object") continue;
        const subs = section === "DATE" ? Object.values(item) : [item];
        for (const sub of subs) {
          if (!sub || typeof sub !== "object") continue;
          const x = sub.X != null ? sub.X : 0;
          const y = sub.Y != null ? sub.Y : 0;
          const w = sub.WIDTH != null ? sub.WIDTH : 100;
          const h = sub.HEIGHT != null ? sub.HEIGHT : (sub.FONT_SIZE != null ? sub.FONT_SIZE * 1.4 : 20);
          consider(x, y, w, h);
        }
      }
    }
  }
  const y = Math.min(maxBottom, SCREEN_H - 60);
  return { x: 20, y };
}

$("add-el-type").addEventListener("change", () => {
  const raw = $("add-el-type").value;
  try {
    addModalType = JSON.parse(raw);
    updateAddModalFields();
  } catch (e) { /* ignore */ }
});

$("add-el-cancel").addEventListener("click", () => {
  $("add-element-modal").classList.add("hidden");
  addModalType = null;
});

$("add-el-confirm").addEventListener("click", () => {
  if (!addModalType) {
    setStatus(themeStatus, "Add error: no element type selected.", "error");
    return;
  }
  const x = parseInt($("add-el-x").value, 10) || 0;
  const y = parseInt($("add-el-y").value, 10) || 0;
  const fontSize = parseInt($("add-el-font-size").value, 10) || 16;
  const color = $("add-el-color").value;
  const barColor = $("add-el-bar-color").value;
  const w = parseInt($("add-el-w").value, 10) || 300;
  const h = parseInt($("add-el-h").value, 10) || 8;
  const text = $("add-el-text").value.trim();

  const t = addModalType;
  let path = null;
  try {
    if (t.kind === "text") {
      path = addTextElementAt(t.name, text || "Label", x, y, fontSize, color);
    } else if (t.kind === "date") {
      path = addDateElementAt(t.name, t.fmt, x, y, fontSize, color);
    } else {
      path = addStatElementAt(t.cat, t.name, x, y, fontSize, color, barColor, w, h);
    }
  } catch (e) {
    setStatus(themeStatus, "Add error: " + e.message, "error");
    return;
  }
  $("add-element-modal").classList.add("hidden");
  addModalType = null;
  if (path) selectElement(path);
  setStatus(themeStatus, "Element added.", "ok");
});

function renderPalette() {
  const host = $("element-palette");
  host.innerHTML = "";
  for (const group of PALETTE) {
    const g = document.createElement("div");
    g.className = "palette-group";
    const title = document.createElement("span");
    title.className = "palette-title";
    title.textContent = group.group;
    g.appendChild(title);
    for (const item of group.items) {
      const btn = document.createElement("button");
      btn.className = "palette-btn";
      btn.textContent = item.label;
      btn.title = "Add " + item.name + " element";
      btn.addEventListener("click", () => openAddModal(item));
      g.appendChild(btn);
    }
    host.appendChild(g);
  }
}
renderPalette();

// --- align / arrange toolbar ---

function buildAlignToolbar() {
  const host = $("align-toolbar");
  host.innerHTML = "";
  const actions = [
    { id: "align-left", label: "⬅ Align L", hint: "Align to screen left edge" },
    { id: "align-hcenter", label: "⇌ Center X", hint: "Center horizontally on screen" },
    { id: "align-right", label: "Align R ➡", hint: "Align to screen right edge" },
    { id: "align-top", label: "⬆ Align T", hint: "Align to screen top edge" },
    { id: "align-vcenter", label: "⇅ Center Y", hint: "Center vertically on screen" },
    { id: "align-bottom", label: "Align B ⬇", hint: "Align to screen bottom edge" },
  ];
  for (const a of actions) {
    const btn = document.createElement("button");
    btn.className = "align-btn";
    btn.textContent = a.label;
    btn.title = a.hint;
    btn.dataset.action = a.id;
    btn.disabled = true;
    btn.addEventListener("click", () => applyAlignAction(a.id));
    host.appendChild(btn);
  }
}

function selectedElementPaths() {
  if (!selectedKey) return [];
  return [selectedKey.split(".").filter((s) => s.length > 0)];
}

function updateAlignToolbarState() {
  const btns = document.querySelectorAll(".align-btn");
  btns.forEach((b) => {
    b.disabled = !selectedKey;
  });
}

function elementBounds(path) {
  const node = pathGet(parsedYaml, path);
  if (!node) return null;
  const isWide = node.WIDTH !== undefined && node.HEIGHT !== undefined;
  const w = isWide ? (node.WIDTH || 0) : 40;
  const h = isWide ? (node.HEIGHT || 0) : 16;
  const x = node.X || 0;
  const y = node.Y || 0;
  // For right/center anchored text the X is not the left edge; use the
  // overlay geometry from compute_layout for real bounds.
  const box = currentLayout && currentLayout.boxes.find((b) => pathKey(b.path) === pathKey(path));
  if (box) {
    return { x: box.x, y: box.y, w: box.w, h: box.h };
  }
  // Fallback: apply the anchor offsets ourselves so a text element whose box
  // has not been computed yet still reports its true top-left corner.
  if (!isWide) {
    const anchor = (node.ANCHOR || "lt").split("");
    const ha = anchor[0] || "l";
    const va = anchor[1] || "t";
    const estW = measureTextWidth(node);
    const estH = node.FONT_SIZE || 14;
    const fx = ha === "r" ? x - estW : ha === "m" ? x - Math.floor(estW / 2) : x;
    const fy = va === "b" ? y - estH : va === "m" ? y - Math.floor(estH / 2) : y;
    return { x: fx, y: fy, w: estW, h: estH };
  }
  return { x, y, w, h };
}

// Rough width estimate for a text node when the backend layout is not
// available yet. Falls back to a per-character guess.
let textMeasureCtx = null;
function measureTextWidth(node) {
  const s = String(node.TEXT || "");
  const fs = node.FONT_SIZE || 14;
  try {
    if (!textMeasureCtx) {
      const c = document.createElement("canvas");
      textMeasureCtx = c.getContext("2d");
    }
    textMeasureCtx.font = `${fs}px sans-serif`;
    return Math.max(4, Math.ceil(textMeasureCtx.measureText(s || "123").width));
  } catch (e) {
    return Math.max(4, Math.ceil(fs * 0.6 * (s.length || 3)));
  }
}

function applyAlignAction(action) {
  const paths = selectedElementPaths();
  if (paths.length < 1) return;
  const bounds = paths.map((p) => ({ p, b: elementBounds(p) })).filter((x) => x.b);
  if (bounds.length < 1) return;

  const screenW = (currentLayout && currentLayout.width) || previewImg.naturalWidth || 320;
  const screenH = (currentLayout && currentLayout.height) || previewImg.naturalHeight || 240;

  // Align actions snap the element to the screen edges/center.
  for (const { p, b } of bounds) {
    const node = pathGet(parsedYaml, p);
    if (!node) continue;
    const anchor = (node.ANCHOR || "lt").split("");
    const ha = anchor[0] || "l";
    const va = anchor[1] || "t";
    // Recompute the X/Y the element needs to hold its edge/center at the target.
    if (action === "align-left") {
      node.X = 0 + ((ha === "l") ? 0 : (ha === "m" ? b.w / 2 : b.w));
    } else if (action === "align-right") {
      node.X = (screenW - b.w) + ((ha === "l") ? 0 : (ha === "m" ? b.w / 2 : b.w));
    } else if (action === "align-hcenter") {
      const cx = screenW / 2;
      node.X = cx - b.w / 2 + ((ha === "l") ? 0 : (ha === "m" ? b.w / 2 : b.w));
    } else if (action === "align-top") {
      node.Y = 0 + ((va === "t") ? 0 : (va === "m" ? b.h / 2 : b.h));
    } else if (action === "align-bottom") {
      node.Y = (screenH - b.h) + ((va === "t") ? 0 : (va === "m" ? b.h / 2 : b.h));
    } else if (action === "align-vcenter") {
      const cy = screenH / 2;
      node.Y = cy - b.h / 2 + ((va === "t") ? 0 : (va === "m" ? b.h / 2 : b.h));
    }
  }
  syncFromVisualEdit();
}

buildAlignToolbar();
updateAlignToolbarState();

function removeElement(path) {
  if (!parsedYaml) return;
  // Walk to the parent, then delete the key. For STATS.X.NAME paths the
  // parent may itself become empty - leave it, harmless.
  if (path.length < 2) return;
  const parentPath = path.slice(0, path.length - 1);
  const key = path[path.length - 1];
  const parent = pathGet(parsedYaml, parentPath);
  if (parent && typeof parent === "object") {
    delete parent[key];
  }
  selectedKey = null;
  syncFromVisualEdit();
  renderElementList();
}

/// Mirrors the Rust `scan_elements`: finds every node that has both X
/// and Y, anywhere in the tree, regardless of depth.
function scanElements(value, path, out) {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    const hasXY = Object.prototype.hasOwnProperty.call(value, "X") && Object.prototype.hasOwnProperty.call(value, "Y");
    if (hasXY) out.push([...path]);
    for (const key of Object.keys(value)) {
      path.push(key);
      scanElements(value[key], path, out);
      path.pop();
    }
  }
}

function renderElementList() {
  const list = $("element-list");
  list.innerHTML = "";
  elementInputsMap = {};
  elementDetailsMap = {};
  if (!parsedYaml) return;

  const elements = [];
  scanElements(parsedYaml, [], elements);

  // Drop selections that no longer exist in the tree.
  if (selectedKey && !elements.some((p) => pathKey(p) === selectedKey)) selectedKey = null;

  for (const path of elements) {
    const node = pathGet(parsedYaml, path);
    if (!node) continue;
    list.appendChild(renderElementItem(path, node));
  }

  // Re-apply open state: whichever element is currently selected stays
  // open after a re-render (e.g. after the field edit re-renders).
  for (const k in elementDetailsMap) {
    elementDetailsMap[k].open = k === selectedKey;
  }

  updateAlignToolbarState();

  if (elements.length === 0) {
    const p = document.createElement("p");
    p.className = "status";
    p.textContent = "No elements with X/Y found.";
    list.appendChild(p);
  }
}

function renderElementItem(path, node) {
  const key = pathKey(path);
  const details = document.createElement("details");
  details.className = "el-item" + (key === selectedKey ? " selected" : "");

  const summary = document.createElement("summary");
  const pathSpan = document.createElement("span");
  pathSpan.className = "path";
  pathSpan.textContent = key;
  summary.appendChild(pathSpan);
  if (node.SHOW === false) {
    const tag = document.createElement("span");
    tag.className = "hidden-tag";
    tag.textContent = "hidden";
    summary.appendChild(tag);
  }
  const delBtn = document.createElement("button");
  delBtn.className = "del-btn";
  delBtn.textContent = "✕";
  delBtn.title = "Remove this element";
  delBtn.addEventListener("click", (ev) => {
    ev.stopPropagation();
    removeElement(path);
  });
  summary.appendChild(delBtn);
  summary.addEventListener("click", (ev) => {
    ev.preventDefault();
    selectElement(path);
  });
  details.appendChild(summary);

  elementDetailsMap[key] = details;

  const body = document.createElement("div");
  body.className = "el-body";

  const isBar = node.WIDTH !== undefined && node.HEIGHT !== undefined && node.PATH === undefined && node.BAR_COLOR !== undefined;
  const isIcon = node.WIDTH !== undefined && node.HEIGHT !== undefined && node.PATH === undefined && node.BAR_COLOR === undefined;

  body.appendChild(numField(path, "X", node.X, 0));
  body.appendChild(numField(path, "Y", node.Y, 0));

  if (isBar) {
    body.appendChild(numField(path, "WIDTH", node.WIDTH, 80));
    body.appendChild(numField(path, "HEIGHT", node.HEIGHT, 10));
    body.appendChild(colorField(path, "BAR_COLOR", node.BAR_COLOR));
  } else if (isIcon) {
    body.appendChild(numField(path, "WIDTH", node.WIDTH, 24));
    body.appendChild(numField(path, "HEIGHT", node.HEIGHT, 24));
    const note = document.createElement("p");
    note.className = "status";
    note.textContent = "Weather icon (auto)";
    body.appendChild(note);
  } else {
    if (node.TEXT !== undefined) {
      body.appendChild(textFieldFull(path, "TEXT", node.TEXT));
    }
    body.appendChild(numField(path, "FONT_SIZE", node.FONT_SIZE, 14));
    body.appendChild(colorField(path, "FONT_COLOR", node.FONT_COLOR));
    body.appendChild(anchorField(path, node.ANCHOR));
  }

  body.appendChild(checkboxField(path, "SHOW", node.SHOW !== false));

  details.appendChild(body);
  details.open = false;
  return details;
}

function labeledWrap(labelText, inputEl) {
  const wrap = document.createElement("div");
  wrap.className = "field";
  const label = document.createElement("label");
  label.textContent = labelText;
  wrap.appendChild(label);
  wrap.appendChild(inputEl);
  return wrap;
}

function numField(path, field, value, fallback) {
  const input = document.createElement("input");
  input.type = "number";
  input.value = value !== undefined ? value : fallback;
  input.addEventListener("change", () => {
    pathSet(parsedYaml, [...path, field], parseInt(input.value, 10) || 0);
    debouncedSyncFromVisual();
  });
  elementInputsMap[pathKey(path) + "." + field] = input;
  return labeledWrap(field, input);
}

function textFieldFull(path, field, value) {
  const input = document.createElement("input");
  input.type = "text";
  input.value = value !== undefined ? value : "";
  input.addEventListener("change", () => {
    pathSet(parsedYaml, [...path, field], input.value);
    debouncedSyncFromVisual();
  });
  const wrap = labeledWrap(field, input);
  wrap.classList.add("full");
  return wrap;
}

function colorField(path, field, csv) {
  const input = document.createElement("input");
  input.type = "color";
  input.value = csvToHex(csv || "255, 255, 255");
  input.addEventListener("change", () => {
    pathSet(parsedYaml, [...path, field], hexToCsv(input.value));
    debouncedSyncFromVisual();
  });
  return labeledWrap(field, input);
}

function anchorField(path, value) {
  const select = document.createElement("select");
  const options = ["lt", "mt", "rt", "lm", "mm", "rm", "lb", "mb", "rb"];
  for (const o of options) {
    const opt = document.createElement("option");
    opt.value = o;
    opt.textContent = o;
    if (o === (value || "lt")) opt.selected = true;
    select.appendChild(opt);
  }
  select.addEventListener("change", () => {
    pathSet(parsedYaml, [...path, "ANCHOR"], select.value);
    debouncedSyncFromVisual();
  });
  return labeledWrap("ANCHOR", select);
}

function checkboxField(path, field, checked) {
  const wrap = document.createElement("div");
  wrap.className = "field full";
  const label = document.createElement("label");
  label.className = "checkbox";
  const input = document.createElement("input");
  input.type = "checkbox";
  input.checked = checked;
  input.addEventListener("change", () => {
    pathSet(parsedYaml, [...path, field], input.checked);
    debouncedSyncFromVisual();
  });
  label.appendChild(input);
  label.appendChild(document.createTextNode(field));
  wrap.appendChild(label);
  return wrap;
}

function csvToHex(csv) {
  const parts = String(csv).split(",").map((s) => parseInt(s.trim(), 10) || 0);
  const [r, g, b] = parts.length === 3 ? parts : [255, 255, 255];
  return "#" + [r, g, b].map((n) => n.toString(16).padStart(2, "0")).join("");
}

function hexToCsv(hex) {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `${r}, ${g}, ${b}`;
}

$("save-theme").addEventListener("click", async () => {
  try {
    await invoke("save_theme_yaml", { theme: currentTheme, yamlText });
    setStatus(themeStatus, "Saved.", "ok");
    loadGeneralPreview(currentTheme);
  } catch (e) {
    setStatus(themeStatus, "Save error: " + e, "error");
  }
});

// --- startup checkbox ---

$("run-on-startup").addEventListener("change", async () => {
  const enabled = $("run-on-startup").checked;
  const delay = parseInt($("startup-delay").value) || 30;
  try {
    await invoke("set_startup", { enabled, delaySeconds: delay });
    setStatus(globalStatus, enabled ? "Startup task created (admin, delay " + delay + "s)." : "Startup task removed.", "ok");
  } catch (e) {
    setStatus(globalStatus, "Startup error: " + e + " (try running as Administrator)", "error");
    $("run-on-startup").checked = !enabled;
  }
});

// --- browse LHM folder ---

$("browse-lhm").addEventListener("click", async () => {
  try {
    const path = await invoke("select_folder");
    if (path) {
      $("lhm-path").value = path;
    }
  } catch (e) {
    setStatus(globalStatus, "Browse error: " + e, "error");
  }
});

// --- custom number spinner buttons ---

document.addEventListener("click", (e) => {
  const btn = e.target.closest(".num-btn button");
  if (!btn) return;
  const id = btn.dataset.target;
  const input = $(id);
  if (!input) return;
  const step = parseFloat(input.step) || 1;
  const dir = btn.dataset.dir === "up" ? 1 : -1;
  const val = parseFloat(input.value) || 0;
  const newVal = parseFloat((val + step * dir).toFixed(4));
  input.value = newVal;
  input.dispatchEvent(new Event("change", { bubbles: true }));
});

// ---------------------------------------------------------------------

initGeneralTab().then(populateThemeSelect);
