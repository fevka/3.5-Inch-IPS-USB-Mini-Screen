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
    const div = document.createElement("div");
    div.className = "el-box" + (key === selectedKey ? " selected" : "");
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
  selectedKey = pathKey(path);
  drawOverlay();
  for (const k in elementDetailsMap) {
    elementDetailsMap[k].classList.toggle("selected", k === selectedKey);
  }
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
  dragState = {
    path: box.path,
    startMouseX: e.clientX,
    startMouseY: e.clientY,
    startX: node.X || 0,
    startY: node.Y || 0,
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
  // Update YAML data
  pathSet(parsedYaml, [...dragState.path, "X"], newX);
  pathSet(parsedYaml, [...dragState.path, "Y"], newY);
  // Update input fields
  const key = pathKey(dragState.path);
  if (elementInputsMap[key + ".X"]) elementInputsMap[key + ".X"].value = newX;
  if (elementInputsMap[key + ".Y"]) elementInputsMap[key + ".Y"].value = newY;
  // Move overlay box locally (no backend call)
  if (dragState.overlayEl) {
    dragState.overlayEl.style.left = (newX * previewImg.clientWidth / currentLayout.width) + "px";
    dragState.overlayEl.style.top = (newY * previewImg.clientHeight / currentLayout.height) + "px";
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

  for (const path of elements) {
    const node = pathGet(parsedYaml, path);
    if (!node) continue;
    list.appendChild(renderElementItem(path, node));
  }

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
  summary.addEventListener("click", () => highlightSelection(path));
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
