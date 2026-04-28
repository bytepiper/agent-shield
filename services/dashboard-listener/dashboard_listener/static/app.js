const STORAGE_KEY = "agentShieldDashboardState";
const MIN_DETAIL_WIDTH = 320;
const DEFAULT_DETAIL_WIDTH = 560;
const DEFAULT_COLUMN_WIDTHS = {
  time: 120,
  action: 112,
  method: 96,
  domain: 176,
  url: 520,
  bodies: 120,
  size: 104,
  flow: 80,
};
const MIN_COLUMN_WIDTHS = {
  time: 84,
  action: 90,
  method: 80,
  domain: 120,
  url: 220,
  bodies: 92,
  size: 72,
  flow: 64,
};
const COLUMN_FIELDS = ["time", "action", "method", "domain", "url", "bodies", "size", "flow"];
const FILTER_FIELDS = ["q", "domain", "method", "phase"];
const VALID_SORT_FIELDS = new Set(["time", "action", "method", "domain", "url", "bodies", "size", "flow"]);
const VALID_SORT_DIRS = new Set(["asc", "desc"]);
const VALID_TABS = new Set(["overview", "reqh", "resph", "req", "resp"]);

function defaultState() {
  return {
    detailOpen: false,
    detailId: null,
    detailTab: "overview",
    detailWidth: DEFAULT_DETAIL_WIDTH,
    filters: { q: "", domain: "", method: "", phase: "" },
    sort: { field: "time", dir: "desc" },
    columns: {
      visible: Object.fromEntries(COLUMN_FIELDS.map((field) => [field, true])),
      widths: { ...DEFAULT_COLUMN_WIDTHS },
    },
  };
}

function loadState() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return defaultState();
    const parsed = JSON.parse(raw);
    const state = defaultState();

    if (parsed && typeof parsed === "object") {
      state.detailOpen = Boolean(parsed.detailOpen);
      state.detailId = Number.isInteger(parsed.detailId) && parsed.detailId > 0 ? parsed.detailId : null;
      state.detailTab = VALID_TABS.has(parsed.detailTab) ? parsed.detailTab : "overview";
      state.detailWidth =
        Number.isFinite(parsed.detailWidth) && parsed.detailWidth >= MIN_DETAIL_WIDTH
          ? parsed.detailWidth
          : DEFAULT_DETAIL_WIDTH;

      if (parsed.filters && typeof parsed.filters === "object") {
        for (const field of FILTER_FIELDS) {
          state.filters[field] = typeof parsed.filters[field] === "string" ? parsed.filters[field] : "";
        }
      }

      if (parsed.sort && typeof parsed.sort === "object") {
        state.sort.field = VALID_SORT_FIELDS.has(parsed.sort.field) ? parsed.sort.field : "time";
        state.sort.dir = VALID_SORT_DIRS.has(parsed.sort.dir) ? parsed.sort.dir : "desc";
      }

      if (parsed.columns && typeof parsed.columns === "object") {
        if (parsed.columns.visible && typeof parsed.columns.visible === "object") {
          let visibleCount = 0;
          for (const field of COLUMN_FIELDS) {
            const visible = parsed.columns.visible[field];
            state.columns.visible[field] = typeof visible === "boolean" ? visible : true;
            if (state.columns.visible[field]) visibleCount += 1;
          }
          if (visibleCount === 0) {
            state.columns.visible.time = true;
          }
        }
        if (parsed.columns.widths && typeof parsed.columns.widths === "object") {
          for (const field of COLUMN_FIELDS) {
            const width = Number(parsed.columns.widths[field]);
            state.columns.widths[field] =
              Number.isFinite(width) && width >= (MIN_COLUMN_WIDTHS[field] || 64)
                ? width
                : DEFAULT_COLUMN_WIDTHS[field];
          }
        }
      }
    }

    return state;
  } catch {
    return defaultState();
  }
}

function saveState(mutator) {
  const state = loadState();
  if (typeof mutator === "function") {
    mutator(state);
  }
  localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  return state;
}

function getStoredDetailWidth() {
  return loadState().detailWidth;
}

function getFilterForm() {
  return document.getElementById("traffic-filters");
}

function countVisibleColumns(visible) {
  return COLUMN_FIELDS.filter((field) => visible[field]).length;
}

function currentColumnState() {
  return loadState().columns;
}

function setColumnVisible(field, visible) {
  if (!COLUMN_FIELDS.includes(field)) return false;
  const state = saveState((draft) => {
    draft.columns.visible[field] = Boolean(visible);
    if (countVisibleColumns(draft.columns.visible) === 0) {
      draft.columns.visible[field] = true;
    }
  });
  return state.columns.visible[field];
}

function setColumnWidth(field, width) {
  if (!COLUMN_FIELDS.includes(field)) return;
  const min = MIN_COLUMN_WIDTHS[field] || 64;
  const normalized = Math.max(min, Math.floor(width));
  saveState((state) => {
    state.columns.widths[field] = normalized;
  });
}

function applyColumnState() {
  const { visible, widths } = currentColumnState();
  const trafficTable = document.getElementById("traffic-table");
  const table = trafficTable?.querySelector("table");
  let totalVisibleWidth = 0;

  for (const field of COLUMN_FIELDS) {
    const col = document.querySelector(`col[data-col="${field}"]`);
    const cells = document.querySelectorAll(`[data-col="${field}"]`);
    const isVisible = Boolean(visible[field]);
    const width = widths[field] || DEFAULT_COLUMN_WIDTHS[field];

    if (col) {
      col.style.width = isVisible ? `${width}px` : "0px";
      col.style.display = isVisible ? "" : "none";
      col.hidden = !isVisible;
    }

    cells.forEach((cell) => {
      cell.style.display = isVisible ? "" : "none";
      cell.style.width = isVisible ? `${width}px` : "0px";
      cell.style.minWidth = isVisible ? `${width}px` : "0px";
      cell.style.maxWidth = isVisible ? `${width}px` : "0px";
      cell.hidden = !isVisible;
    });

    if (isVisible) {
      totalVisibleWidth += width;
    }
  }

  if (table && trafficTable) {
    const minWidth = Math.max(totalVisibleWidth, trafficTable.clientWidth || 0);
    table.style.width = `${minWidth}px`;
    table.style.minWidth = `${minWidth}px`;
  }

  const toggles = document.querySelectorAll(".column-toggle");
  toggles.forEach((toggle) => {
    const field = toggle.dataset.column;
    if (!field || !COLUMN_FIELDS.includes(field)) return;
    toggle.checked = Boolean(visible[field]);
    toggle.disabled = countVisibleColumns(visible) === 1 && Boolean(visible[field]);
  });
}

function setSortState(field, dir) {
  const sortField = document.getElementById("sort-field");
  const sortDir = document.getElementById("sort-dir");
  if (sortField) sortField.value = field;
  if (sortDir) sortDir.value = dir;
  saveState((state) => {
    state.sort.field = VALID_SORT_FIELDS.has(field) ? field : "time";
    state.sort.dir = VALID_SORT_DIRS.has(dir) ? dir : "desc";
  });
}

function collectFilterState() {
  const form = getFilterForm();
  const filters = { q: "", domain: "", method: "", phase: "" };
  if (!form) return filters;
  for (const field of FILTER_FIELDS) {
    const input = form.querySelector(`[name="${field}"]`);
    filters[field] = input && typeof input.value === "string" ? input.value : "";
  }
  return filters;
}

function persistFilterState() {
  const filters = collectFilterState();
  saveState((state) => {
    state.filters = filters;
  });
}

function applyStoredControls() {
  const state = loadState();
  const form = getFilterForm();
  if (!form) return false;

  let changed = false;
  for (const field of FILTER_FIELDS) {
    const input = form.querySelector(`[name="${field}"]`);
    if (input && input.value !== state.filters[field]) {
      input.value = state.filters[field];
      changed = true;
    }
  }

  const sortField = document.getElementById("sort-field");
  const sortDir = document.getElementById("sort-dir");
  if (sortField && sortField.value !== state.sort.field) {
    sortField.value = state.sort.field;
    changed = true;
  }
  if (sortDir && sortDir.value !== state.sort.dir) {
    sortDir.value = state.sort.dir;
    changed = true;
  }

  return changed;
}

function applyDetailShell(open) {
  const pane = document.getElementById("detail-pane");
  const resizer = document.getElementById("detail-resizer");
  if (!pane || !resizer) return;
  if (open) {
    pane.style.width = `${getStoredDetailWidth()}px`;
    pane.style.minWidth = `${MIN_DETAIL_WIDTH}px`;
    pane.style.borderLeftWidth = "1px";
    resizer.classList.remove("hidden");
  } else {
    pane.style.width = "0px";
    pane.style.minWidth = "0px";
    pane.style.borderLeftWidth = "0px";
    resizer.classList.add("hidden");
  }
}

function openDetailPane() {
  applyDetailShell(true);
  saveState((state) => {
    state.detailOpen = true;
  });
}

function clearDetailContent() {
  const content = document.getElementById("detail-content");
  if (content) content.innerHTML = "";
}

function closeDetailPane() {
  clearDetailContent();
  applyDetailShell(false);
  saveState((state) => {
    state.detailOpen = false;
    state.detailId = null;
    state.detailTab = "overview";
  });
}

function rememberDetailSelection(listenerId, tab) {
  const id = Number(listenerId);
  if (!Number.isInteger(id) || id <= 0) return;
  saveState((state) => {
    state.detailOpen = true;
    state.detailId = id;
    state.detailTab = VALID_TABS.has(tab) ? tab : "overview";
  });
}

function selectedDetailUrl() {
  const state = loadState();
  if (!state.detailOpen || !state.detailId) return null;
  const tab = VALID_TABS.has(state.detailTab) ? state.detailTab : "overview";
  return `/fragments/detail/${state.detailId}?tab=${encodeURIComponent(tab)}`;
}

function hasVisibleRow(listenerId) {
  return Boolean(document.querySelector(`[data-listener-id="${listenerId}"]`));
}

function restoreSelectedDetail() {
  const state = loadState();
  if (!state.detailOpen || !state.detailId) {
    applyDetailShell(false);
    clearDetailContent();
    return;
  }

  if (!hasVisibleRow(state.detailId)) {
    closeDetailPane();
    return;
  }

  const url = selectedDetailUrl();
  if (!url) {
    closeDetailPane();
    return;
  }

  openDetailPane();
  htmx.ajax("GET", url, { target: "#detail-content", swap: "innerHTML" });
}

function validateDetailSelection() {
  const state = loadState();
  if (!state.detailOpen || !state.detailId) return;
  if (!hasVisibleRow(state.detailId)) {
    closeDetailPane();
  }
}

function initDetailResizer() {
  const resizer = document.getElementById("detail-resizer");
  if (!resizer || resizer.dataset.bound === "1") return;
  resizer.dataset.bound = "1";

  let dragging = false;
  const onMove = (event) => {
    if (!dragging) return;
    const pane = document.getElementById("detail-pane");
    if (!pane) return;
    const min = MIN_DETAIL_WIDTH;
    const max = Math.max(min, Math.floor(window.innerWidth * 0.72));
    const width = Math.min(max, Math.max(min, window.innerWidth - event.clientX));
    pane.style.width = `${width}px`;
    saveState((state) => {
      state.detailWidth = width;
    });
  };
  const stop = () => {
    dragging = false;
    document.body.style.userSelect = "";
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", stop);
  };

  resizer.addEventListener("mousedown", (event) => {
    event.preventDefault();
    dragging = true;
    document.body.style.userSelect = "none";
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", stop);
  });
}

function bindFilterPersistence() {
  const form = getFilterForm();
  if (!form || form.dataset.bound === "1") return;
  form.dataset.bound = "1";
  for (const field of FILTER_FIELDS) {
    const input = form.querySelector(`[name="${field}"]`);
    if (!input) continue;
    const handler = () => persistFilterState();
    input.addEventListener("input", handler);
    input.addEventListener("change", handler);
  }
}

function bindColumnControls() {
  const toggles = document.querySelectorAll(".column-toggle");
  toggles.forEach((toggle) => {
    if (toggle.dataset.bound === "1") return;
    toggle.dataset.bound = "1";
    toggle.addEventListener("change", () => {
      const field = toggle.dataset.column;
      if (!field) return;
      const finalVisible = setColumnVisible(field, toggle.checked);
      toggle.checked = finalVisible;
      applyColumnState();
    });
  });
}

function initColumnResizers() {
  const resizers = document.querySelectorAll(".column-resizer");
  resizers.forEach((resizer) => {
    if (resizer.dataset.bound === "1") return;
    resizer.dataset.bound = "1";
    const field = resizer.dataset.column;
    if (!field || !COLUMN_FIELDS.includes(field)) return;

    let dragging = false;
    let startX = 0;
    let startWidth = 0;

    const onMove = (event) => {
      if (!dragging) return;
      const delta = event.clientX - startX;
      const nextWidth = startWidth + delta;
      setColumnWidth(field, nextWidth);
      applyColumnState();
    };

    const stop = () => {
      dragging = false;
      document.body.style.userSelect = "";
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", stop);
    };

    resizer.addEventListener("mousedown", (event) => {
      event.preventDefault();
      event.stopPropagation();
      const cell = document.querySelector(`th[data-col="${field}"]`);
      const rect = cell?.getBoundingClientRect();
      startX = event.clientX;
      startWidth = rect?.width || currentColumnState().widths[field] || DEFAULT_COLUMN_WIDTHS[field];
      dragging = true;
      document.body.style.userSelect = "none";
      window.addEventListener("mousemove", onMove);
      window.addEventListener("mouseup", stop);
    });
  });
}

function refreshTrafficIfNeeded() {
  const changed = applyStoredControls();
  bindFilterPersistence();
  bindColumnControls();
  if (changed) {
    htmx.ajax("GET", "/fragments/traffic", {
      target: "#traffic-table",
      swap: "outerHTML",
      values: {
        ...collectFilterState(),
        sort: document.getElementById("sort-field")?.value || "time",
        dir: document.getElementById("sort-dir")?.value || "desc",
      },
    });
    return;
  }
  applyColumnState();
  validateDetailSelection();
}

document.addEventListener("htmx:afterSwap", (event) => {
  initDetailResizer();
  bindFilterPersistence();
  bindColumnControls();
  initColumnResizers();
  applyColumnState();

  const target = event.detail?.target;
  if (target && target.id === "traffic-table") {
    validateDetailSelection();
  }
});

document.addEventListener("DOMContentLoaded", () => {
  refreshTrafficIfNeeded();
  initDetailResizer();
  const state = loadState();
  applyDetailShell(state.detailOpen);
  bindFilterPersistence();
  bindColumnControls();
  initColumnResizers();
  applyColumnState();
  restoreSelectedDetail();
});
