// SCMDB Watcher - Frontend Logic
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { open, save } = window.__TAURI__.dialog;

// --- State ---
let isRunning = false;
let activeMissions = [];
let eventLog = [];
let eventCount = 0;
const MAX_LOG_LINES = 500;
const MAX_RECENT_EVENTS = 50;

// --- DOM refs ---
const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => document.querySelectorAll(sel);

const statusBadge = $('#statusBadge');
const btnToggle = $('#btnToggleWatcher');
const statPort = $('#statPort');
const statMissions = $('#statMissions');
const statEvents = $('#statEvents');
const missionsList = $('#missionsList');
const recentEvents = $('#recentEvents');
const logViewer = $('#logViewer');
const btnClearLogs = $('#btnClearLogs');
const chkAutoScroll = $('#chkAutoScroll');

// Settings
const inputLogPath = $('#inputLogPath');
const btnBrowseLogPath = $('#btnBrowseLogPath');
const inputPort = $('#inputPort');
const chkAutoStart = $('#chkAutoStart');
const chkDevMode = $('#chkDevMode');
const devOriginsGroup = $('#devOriginsGroup');
const inputDevOrigins = $('#inputDevOrigins');
const originsList = $('#originsList');
const inputNewOrigin = $('#inputNewOrigin');
const btnAddOrigin = $('#btnAddOrigin');
const btnSaveSettings = $('#btnSaveSettings');
const saveFeedback = $('#saveFeedback');

// Import
const inputLogbackups = $('#inputLogbackups');
const btnBrowseLogbackups = $('#btnBrowseLogbackups');
const chkIncludeCurrent = $('#chkIncludeCurrent');
const btnRunImport = $('#btnRunImport');
const importResult = $('#importResult');
const importResultContent = $('#importResultContent');

// --- Navigation ---
$$('.nav-item').forEach((item) => {
  item.addEventListener('click', () => {
    $$('.nav-item').forEach((i) => i.classList.remove('active'));
    item.classList.add('active');
    $$('.tab-panel').forEach((p) => p.classList.remove('active'));
    $(`#tab-${item.dataset.tab}`).classList.add('active');
  });
});

// --- Watcher control ---
btnToggle.addEventListener('click', async () => {
  try {
    if (isRunning) {
      await invoke('stop_watcher');
    } else {
      await invoke('start_watcher');
    }
  } catch (e) {
    console.error('Toggle watcher error:', e);
  }
});

function updateStatus(running) {
  isRunning = running;
  btnToggle.textContent = running ? 'Stop Watcher' : 'Start Watcher';
  btnToggle.classList.toggle('btn-danger', running);
  btnToggle.classList.toggle('btn-primary', !running);
  statusBadge.classList.toggle('running', running);
  statusBadge.querySelector('.status-text').textContent = running ? 'Running' : 'Stopped';
}

// --- Missions ---
let missionTimerInterval = null;

function renderMissions() {
  statMissions.textContent = activeMissions.length;
  if (activeMissions.length === 0) {
    missionsList.innerHTML = '<div class="empty-state">No active missions</div>';
    return;
  }
  missionsList.innerHTML = activeMissions
    .map(
      (m) => `
    <div class="mission-card">
      <span class="mission-name">${escapeHtml(m.debugName || '?')}</span>
      <span class="mission-timer" data-start="${m.startTs}">${formatElapsed(m.startTs)}</span>
    </div>`
    )
    .join('');
}

function formatElapsed(startTs) {
  if (!startTs) return '--:--';
  const elapsed = Math.floor(Date.now() / 1000 - startTs);
  if (elapsed < 0) return '00:00';
  const h = Math.floor(elapsed / 3600);
  const m = Math.floor((elapsed % 3600) / 60);
  const s = elapsed % 60;
  if (h > 0) return `${h}:${pad(m)}:${pad(s)}`;
  return `${pad(m)}:${pad(s)}`;
}

function pad(n) {
  return n.toString().padStart(2, '0');
}

function updateTimers() {
  $$('.mission-timer').forEach((el) => {
    const start = parseFloat(el.dataset.start);
    if (start) el.textContent = formatElapsed(start);
  });
}

missionTimerInterval = setInterval(updateTimers, 1000);

// --- Events ---
function addEvent(event) {
  eventCount++;
  statEvents.textContent = eventCount;

  const type = event.type;
  const time = new Date().toLocaleTimeString();
  let text = '';
  let typeClass = '';

  switch (type) {
    case 'mission_start':
      text = `Mission started: ${event.debugName || '?'}`;
      typeClass = 'start';
      activeMissions.push(event);
      renderMissions();
      break;
    case 'mission_complete':
      text = `Mission complete: ${event.debugName || '?'}`;
      typeClass = 'complete';
      activeMissions = activeMissions.filter((m) => (m.guid) !== event.guid);
      renderMissions();
      break;
    case 'mission_ended':
      text = `Mission ${event.completion}: ${event.debugName || '?'}`;
      typeClass = 'ended';
      activeMissions = activeMissions.filter((m) => (m.guid) !== event.guid);
      renderMissions();
      break;
    case 'blueprint_received':
      text = `Blueprint: ${event.productName}`;
      typeClass = 'blueprint';
      break;
    case 'session_reset':
      text = 'Session reset (log rotation)';
      typeClass = 'reset';
      activeMissions = [];
      renderMissions();
      break;
    case 'state_snapshot':
      activeMissions = event.active || [];
      renderMissions();
      return; // Don't show in event list
    default:
      text = JSON.stringify(event);
      typeClass = '';
  }

  // Add to recent events (dashboard)
  const eventEntry = { type, typeClass, text, time };
  eventLog.unshift(eventEntry);
  if (eventLog.length > MAX_RECENT_EVENTS) eventLog.pop();
  renderRecentEvents();

  // Add to log viewer
  addLogLine(type, text, time);
}

function renderRecentEvents() {
  if (eventLog.length === 0) {
    recentEvents.innerHTML = '<div class="empty-state">Waiting for events...</div>';
    return;
  }
  recentEvents.innerHTML = eventLog
    .slice(0, 20)
    .map(
      (e) => `
    <div class="event-item">
      <span class="event-type ${e.typeClass}">${e.typeClass || 'info'}</span>
      <span class="event-text">${escapeHtml(e.text)}</span>
      <span class="event-time">${e.time}</span>
    </div>`
    )
    .join('');
}

function addLogLine(type, text, time) {
  const line = document.createElement('div');
  line.className = `log-line ${type}`;
  line.innerHTML = `<span class="time">[${time}]</span> <span class="type">[${type}]</span> ${escapeHtml(text)}`;
  logViewer.appendChild(line);

  // Trim old lines
  while (logViewer.children.length > MAX_LOG_LINES) {
    logViewer.removeChild(logViewer.firstChild);
  }

  if (chkAutoScroll.checked) {
    logViewer.scrollTop = logViewer.scrollHeight;
  }
}

btnClearLogs.addEventListener('click', () => {
  logViewer.innerHTML = '';
});

// --- Settings ---
const DEFAULT_ORIGINS = ['https://scmdb.net', 'https://www.scmdb.net'];
let customOrigins = [];

function renderOrigins() {
  originsList.innerHTML = '';
  DEFAULT_ORIGINS.forEach((origin) => {
    const item = document.createElement('div');
    item.className = 'origin-item';
    item.innerHTML = `<span class="origin-text default">${origin} (default)</span>`;
    originsList.appendChild(item);
  });
  customOrigins.forEach((origin, idx) => {
    const item = document.createElement('div');
    item.className = 'origin-item';
    item.innerHTML = `
      <span class="origin-text">${origin}</span>
      <button class="btn-icon" data-action="edit" data-idx="${idx}" title="Edit">✎</button>
      <button class="btn-icon btn-danger" data-action="delete" data-idx="${idx}" title="Delete">✕</button>
    `;
    originsList.appendChild(item);
  });
}

originsList.addEventListener('click', (e) => {
  const btn = e.target.closest('[data-action]');
  if (!btn) return;
  const idx = parseInt(btn.dataset.idx);
  if (btn.dataset.action === 'delete') {
    customOrigins.splice(idx, 1);
    renderOrigins();
  } else if (btn.dataset.action === 'edit') {
    const newVal = prompt('Edit origin:', customOrigins[idx]);
    if (newVal !== null && newVal.trim()) {
      customOrigins[idx] = newVal.trim();
      renderOrigins();
    }
  }
});

btnAddOrigin.addEventListener('click', () => {
  const val = inputNewOrigin.value.trim();
  if (val && !customOrigins.includes(val) && !DEFAULT_ORIGINS.includes(val)) {
    customOrigins.push(val);
    inputNewOrigin.value = '';
    renderOrigins();
  }
});

inputNewOrigin.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') btnAddOrigin.click();
});

async function loadConfig() {
  try {
    const config = await invoke('get_config');
    inputLogPath.value = config.log_path;
    inputPort.value = config.port;
    chkAutoStart.checked = config.auto_start_watcher;
    customOrigins = config.custom_origins || [];
    renderOrigins();
    const isDev = await invoke('is_dev_build');
    const devSection = chkDevMode.closest('.form-group');
    if (!isDev) {
      devSection.classList.add('hidden');
      devOriginsGroup.classList.add('hidden');
    } else {
      devSection.classList.remove('hidden');
      chkDevMode.checked = config.dev_mode;
      inputDevOrigins.value = (config.dev_origins || []).join(', ');
      devOriginsGroup.classList.toggle('hidden', !config.dev_mode);
    }
    statPort.textContent = config.port;
  } catch (e) {
    console.error('Load config error:', e);
  }
}

chkDevMode.addEventListener('change', () => {
  devOriginsGroup.classList.toggle('hidden', !chkDevMode.checked);
});

btnSaveSettings.addEventListener('click', async () => {
  try {
    const config = {
      log_path: inputLogPath.value,
      port: parseInt(inputPort.value) || 23456,
      auto_start_watcher: chkAutoStart.checked,
      dev_mode: chkDevMode.checked,
      dev_origins: inputDevOrigins.value
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean),
      custom_origins: [...customOrigins],
    };
    await invoke('save_config', { config });
    statPort.textContent = config.port;
    saveFeedback.classList.remove('hidden');
    setTimeout(() => saveFeedback.classList.add('hidden'), 2000);
  } catch (e) {
    console.error('Save config error:', e);
  }
});

btnBrowseLogPath.addEventListener('click', async () => {
  const path = await open({
    filters: [{ name: 'Log files', extensions: ['log'] }],
  });
  if (path) inputLogPath.value = path;
});

// --- Import ---
btnBrowseLogbackups.addEventListener('click', async () => {
  const path = await open({ directory: true });
  if (path) inputLogbackups.value = path;
});

btnRunImport.addEventListener('click', async () => {
  const dir = inputLogbackups.value;
  if (!dir) return;

  btnRunImport.disabled = true;
  btnRunImport.textContent = 'Scanning...';
  importResult.classList.add('hidden');

  try {
    const result = await invoke('run_import_command', {
      logbackupsDir: dir,
      includeCurrent: chkIncludeCurrent.checked,
    });

    importResult.classList.remove('hidden');
    importResultContent.innerHTML = `
      <p><strong>${result.missions.length}</strong> mission(s), <strong>${result.blueprints.length}</strong> blueprint(s)</p>
      <p>${result.sourceLogs.length} log file(s) scanned</p>
      ${result.duplicatesMerged > 0 ? `<p>${result.duplicatesMerged} duplicate(s) merged</p>` : ''}
      <button class="btn btn-primary" id="btnExportJson" style="margin-top:12px">Export JSON</button>
    `;

    $('#btnExportJson').addEventListener('click', async () => {
      const savePath = await save({
        defaultPath: `scmdb-import-${new Date().toISOString().slice(0, 10)}.json`,
        filters: [{ name: 'JSON', extensions: ['json'] }],
      });
      if (savePath) {
        const payload = {
          exportSchemaVersion: 1,
          watcherVersion: '0.2.0',
          exportedAt: new Date().toISOString(),
          sourceLogs: result.sourceLogs,
          missions: result.missions,
          blueprints: result.blueprints,
        };
        await invoke('export_import_json', { data: payload, outputPath: savePath });
      }
    });
  } catch (e) {
    importResult.classList.remove('hidden');
    importResultContent.innerHTML = `<p style="color:var(--error)">${escapeHtml(String(e))}</p>`;
  } finally {
    btnRunImport.disabled = false;
    btnRunImport.textContent = 'Run Import';
  }
});

// --- Event listeners (Tauri) ---
listen('watcher-event', (e) => {
  addEvent(e.payload);
});

listen('watcher-status-change', (e) => {
  updateStatus(e.payload === 'running');
});

// --- Init ---
async function init() {
  await loadConfig();
  try {
    const status = await invoke('get_watcher_status');
    updateStatus(status.running);
    if (status.activeMissions) {
      activeMissions = status.activeMissions;
      renderMissions();
    }
  } catch (e) {
    console.error('Init status error:', e);
  }
}

function escapeHtml(str) {
  const div = document.createElement('div');
  div.textContent = str;
  return div.innerHTML;
}

init();
