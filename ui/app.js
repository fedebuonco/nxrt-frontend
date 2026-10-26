/* nxrt-frontend — the window's side. Talks to the Rust commands, renders the four views. */
(() => {
  'use strict';

  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;
  const main = document.getElementById('main');
  const nav = document.getElementById('nav');
  const keysEl = document.getElementById('keys');
  const toasts = document.getElementById('toasts');
  const sheet = document.getElementById('sheet');

  const S = {
    snap: null,
    view: 'library',
    key: null,
    logs: null,
    logsBusy: false,
    manager: null,
    managerBusy: false,
    managerLevel: 'all',
    managerSearch: '',
    managerFollow: true,
    managerTimer: 0,
    update: null,
    runs: {},
    icons: new Map(),
    disc: new Set(),
    lastView: null,
    lastKey: null,
    loading: false,
    queued: false,
    loadTimer: 0,
    pad: { connected: false, name: null, error: null },
    padMode: false,
    padTest: false,
    padHeld: new Set(),
    padLast: null,
    focusSel: null,
    pendingFocus: null,
    wantFocusFirst: false,
  };

  // ---- text, paths, time ----------------------------------------------------------------------------------------

  const esc = (v) =>
    String(v ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

  const baseName = (p) => String(p || '').split('/').filter(Boolean).pop() || String(p || '');
  const dirName = (p) => String(p || '').split('/').slice(0, -1).join('/') || '/';
  const errText = (e) => (typeof e === 'string' ? e : e && e.message ? e.message : String(e));
  const samePath = (a, b) => String(a || '') === String(b || '');

  function initials(name) {
    const words = String(name || '').split(/[^A-Za-z0-9]+/).filter((w) => w.length > 2 || /^[A-Z]+$/.test(w));
    const s = words.map((w) => w[0]).slice(0, 3).join('');
    return (s || '?').toUpperCase();
  }

  function ago(secs) {
    const now = Math.floor(Date.now() / 1000);
    const d = Math.max(0, now - Number(secs || 0));
    if (d < 60) return 'just now';
    if (d < 3600) return `${Math.floor(d / 60)} min ago`;
    if (d < 86400) return `${Math.floor(d / 3600)} h ago`;
    return `${Math.floor(d / 86400)} d ago`;
  }

  function bytes(n) {
    n = Number(n || 0);
    if (n >= 1e6) return `${(n / 1e6).toFixed(1)} MB`;
    return `${Math.max(1, Math.round(n / 1e3))} KB`;
  }

  // ---- icons ----------------------------------------------------------------------------------------------------

  const svg = (body, w = 16) => `<svg class="ico" width="${w}" height="${w}" viewBox="0 0 16 16" fill="none" aria-hidden="true">${body}</svg>`;
  const ICONS = {
    grid: svg('<rect x="1.7" y="1.7" width="5.1" height="5.1" rx="1.3" fill="currentColor"/><rect x="9.2" y="1.7" width="5.1" height="5.1" rx="1.3" fill="currentColor"/><rect x="1.7" y="9.2" width="5.1" height="5.1" rx="1.3" fill="currentColor"/><rect x="9.2" y="9.2" width="5.1" height="5.1" rx="1.3" fill="currentColor"/>'),
    doc: svg('<rect x="3.1" y="1.6" width="9.8" height="12.8" rx="2" stroke="currentColor" stroke-width="1.4"/><path d="M5.4 5.3h5.2M5.4 8h5.2M5.4 10.7h3.4" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>'),
    sliders: svg('<path d="M2 4.4h12M2 11.6h12" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/><circle cx="5.8" cy="4.4" r="1.9" fill="currentColor"/><circle cx="10.2" cy="11.6" r="1.9" fill="currentColor"/>'),
    play: svg('<path d="M4.4 2.6v10.8L13 8z" fill="currentColor"/>', 12),
    plus: svg('<path d="M8 2.4v11.2M2.4 8h11.2" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/>', 14),
    check: svg('<path d="M3.2 8.4l3 3 6.6-6.8" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>', 14),
    back: svg('<path d="M9.8 3.4L5.2 8l4.6 4.6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>', 14),
    chevron: svg('<path d="M4.4 6.2L8 9.8l3.6-3.6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>', 12),
    x: svg('<path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>', 14),
    stop: svg('<rect x="4.2" y="4.2" width="7.6" height="7.6" rx="1.5" fill="currentColor"/>', 12),
    folder: svg('<path d="M2 4.4c0-.7.6-1.2 1.2-1.2h2.4l1.3 1.5h5.9c.7 0 1.2.5 1.2 1.2v5.7c0 .7-.5 1.2-1.2 1.2H3.2c-.6 0-1.2-.5-1.2-1.2V4.4z" stroke="currentColor" stroke-width="1.3"/>'),
    key: svg('<circle cx="5.6" cy="5.6" r="2.9" stroke="currentColor" stroke-width="1.4"/><path d="M7.7 7.7l4.6 4.6M10.2 10.2l1.5-1.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>'),
    cpu: svg('<rect x="3.4" y="3.4" width="9.2" height="9.2" rx="1.6" stroke="currentColor" stroke-width="1.4"/><rect x="6.4" y="6.4" width="3.2" height="3.2" rx=".8" fill="currentColor"/>'),
    spark: svg('<path d="M8 1.8l1.4 4.8L14.2 8l-4.8 1.4L8 14.2 6.6 9.4 1.8 8l4.8-1.4z" fill="currentColor"/>', 14),
    screen: svg('<rect x="2.2" y="3.2" width="11.6" height="8" rx="1.4" stroke="currentColor" stroke-width="1.4"/><path d="M5.4 13.4h5.2" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>', 14),
    bug: svg('<ellipse cx="8" cy="8.8" rx="3.3" ry="4.2" stroke="currentColor" stroke-width="1.3"/><path d="M8 4.6V2.9M5.3 6L3.7 4.4M10.7 6l1.6-1.6M4.7 8.8H2.4M13.6 8.8h-2.3M5.4 11.4l-1.7 1.5M10.6 11.4l1.7 1.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>', 14),
    pad: svg('<path d="M4.5 5.3h7c1.3 0 2.2.9 2.5 2.2l.6 2.6c.3 1.3-.8 2.3-2 1.9l-1.5-.6c-.5-.2-1-.6-1.6-.6H6.5c-.6 0-1 .4-1.6.6l-1.5.6c-1.2.4-2.3-.6-2-1.9l.6-2.6c.3-1.3 1.2-2.2 2.5-2.2z" stroke="currentColor" stroke-width="1.25"/><path d="M5.1 8.2v2.1M4.1 9.25h2.1" stroke="currentColor" stroke-width="1.25" stroke-linecap="round"/><circle cx="10.8" cy="8.7" r=".8" fill="currentColor"/><circle cx="11.9" cy="10" r=".8" fill="currentColor"/>', 16),
  };

  const ABOUT = {
    package: "Your copy of the game (.nsp / .xci). Never changed: the game's data is read from it in place, so keep it where it is.",
    update: "The game's update (.nsp): its new code, and its changes to the game's data, read in place with the base package. Keep it where it is.",
    keys: 'The keys from your own Switch (prod.keys). Needed to read the package; nothing is decrypted to disk.',
    app: "The recomp's program: it runs this game (and does its setup steps). The manager starts it.",
    recomp: "The recomp the manager created for this game: its source (Makefile, runtime/<id>_game.c, tools/patches.py). Edit it to work on the port; build.sh rebuilds the app.",
    data: "Yours: the game's settings, saves and their backups, mods, and the shader cache. Back this folder up; deleting it loses your saves.",
    game: "Made from the package: the game's code files (exefs), a pointer to its data in the package (romfs.src), and the built game code (.gamebuild, the game's library). Safe to delete: the steps make it again.",
    logs: 'A log of every run and step the manager started: what to send when something goes wrong.',
  };

  // ---- controller settings passed to games --------------------------------------------------------

  const CTRL_INPUTS = [
    ['a', 'A'], ['b', 'B'], ['x', 'X'], ['y', 'Y'],
    ['l', 'L'], ['r', 'R'], ['zl', 'ZL'], ['zr', 'ZR'],
    ['plus', 'Plus'], ['minus', 'Minus'],
    ['lstick', 'Left stick press'], ['rstick', 'Right stick press'],
    ['left', 'D-pad Left'], ['up', 'D-pad Up'], ['right', 'D-pad Right'], ['down', 'D-pad Down'],
  ];
  const CTRL_BUTTONS = [
    ['south', 'South (bottom)'], ['east', 'East (right)'], ['west', 'West (left)'], ['north', 'North (top)'],
    ['lshoulder', 'L1 / L'], ['rshoulder', 'R1 / R'], ['ltrigger', 'L2 / ZL'], ['rtrigger', 'R2 / ZR'],
    ['back', 'Back / Minus'], ['start', 'Start / Plus'], ['guide', 'Guide'],
    ['lstick', 'L stick press'], ['rstick', 'R stick press'],
    ['dpad_up', 'D-pad Up'], ['dpad_down', 'D-pad Down'], ['dpad_left', 'D-pad Left'], ['dpad_right', 'D-pad Right'],
    ['touchpad', 'Touchpad'], ['misc1', 'Misc 1'], ['rpaddle1', 'Paddle 1'], ['lpaddle1', 'Paddle 3'],
    ['rpaddle2', 'Paddle 2'], ['lpaddle2', 'Paddle 4'],
    ['misc2', 'Misc 2'], ['misc3', 'Misc 3'], ['misc4', 'Misc 4'], ['misc5', 'Misc 5'], ['misc6', 'Misc 6'],
  ];
  const CTRL_PROMPTS = [['auto', 'Auto'], ['nintendo', 'Nintendo'], ['xbox', 'Xbox'], ['playstation', 'PlayStation'], ['keyboard', 'Keyboard']];

  const ctrlBool = (v) => (v === true ? '1' : v === false ? '0' : '');
  const ctrlButtonLabel = (v) => (CTRL_BUTTONS.find(([x]) => x === v) || ['', v || ''])[1];
  const ctrlPromptLabel = (v) => (CTRL_PROMPTS.find(([x]) => x === v) || ['', v || ''])[1];
  function ctrlCount(c) {
    if (!c) return 0;
    return (c.prompts ? 1 : 0) + (c.dpad_move != null ? 1 : 0) + (c.rumble != null ? 1 : 0) +
      (c.rumble_strength != null ? 1 : 0) + (c.led != null ? 1 : 0) + Object.keys(c.bind_pad || {}).length;
  }
  function ctrlInherit(scope, has, label) {
    if (scope === 'defaults') return "Game's own";
    return has ? `Use defaults (${label})` : 'Use defaults';
  }
  function ctrlSelect(name, options, value, inherit) {
    const opts = [`<option value="">${esc(inherit)}</option>`];
    for (const [v, label] of options) opts.push(`<option value="${esc(v)}"${v === value ? ' selected' : ''}>${esc(label)}</option>`);
    return `<select data-c="${esc(name)}">${opts.join('')}</select>`;
  }
  function ctrlForm(scope, c, global, key) {
    c = c || {}; global = global || {};
    const gp = global.bind_pad || {}, bp = c.bind_pad || {};
    const inheritBool = (name) => ctrlInherit(scope, global[name] != null, global[name] ? 'On' : 'Off');
    const strength = [['0', '0%']];
    for (let i = 5; i <= 100; i += 5) strength.push([String(i), `${i}%`]);
    const prefs = [
      ['Button prompts', 'prompts', CTRL_PROMPTS, c.prompts || '', ctrlInherit(scope, !!global.prompts, ctrlPromptLabel(global.prompts))],
      ['D-pad moves the stick', 'dpad_move', [['1', 'On'], ['0', 'Off']], ctrlBool(c.dpad_move), inheritBool('dpad_move')],
      ['Rumble', 'rumble', [['1', 'On'], ['0', 'Off']], ctrlBool(c.rumble), inheritBool('rumble')],
      ['Rumble strength', 'rumble_strength', strength, c.rumble_strength != null ? String(c.rumble_strength) : '', ctrlInherit(scope, global.rumble_strength != null, global.rumble_strength != null ? `${global.rumble_strength}%` : '')],
      ['Controller light', 'led', [['1', 'On'], ['0', 'Off']], ctrlBool(c.led), inheritBool('led')],
    ].map(([label, name, options, value, inherit]) =>
      `<label class="ctl-row"><span class="ctl-label">${esc(label)}</span>${ctrlSelect(name, options, value, inherit)}</label>`).join('');
    const binds = CTRL_INPUTS.map(([input, label]) => {
      const inherit = ctrlInherit(scope, !!gp[input], ctrlButtonLabel(gp[input]));
      return `<label class="ctl-row"><span class="ctl-label">${esc(label)}</span>${ctrlSelect(`bind_pad.${input}`, CTRL_BUTTONS, bp[input] || '', inherit)}</label>`;
    }).join('');
    const slug = `controls-${scope}${key ? `-${key.replace(/[^a-z0-9]/gi, '-')}` : ''}`;
    const attrs = `data-scope="${scope}"${key ? ` data-key="${esc(key)}"` : ''}`;
    return `<div class="controls-form" ${attrs}>
      <div class="ctl-grid">${prefs}</div>
      <details class="disc" data-disc="${slug}-bind"${scope === 'game' ? ' open' : ''}>
        <summary>Bindings <span class="muted small">${Object.keys(bp).length} set</span></summary>
        <div class="ctl-grid bind-grid">${binds}</div>
      </details>
    </div>`;
  }
  function controlsDefaultsCard() {
    return `<section class="card setting">
      <div class="setting-head">
        <span class="section-title">Controller defaults</span><span class="spacer"></span>
        <button class="btn ghost sm" data-act="controls-reset-defaults">Reset</button>
      </div>
      <p>Passed to every game when it starts: a row set here wins over the game's own setting, and a game's Controls can override it. Left on “Game's own”, the game keeps what it has. With a controller, focus a row and use ← → (or A) to change it.</p>
      ${ctrlForm('defaults', S.snap.controls, {})}
    </section>`;
  }
  function controlsCard(g) {
    const n = ctrlCount(g.controls);
    const note = n ? tag(`${n} set`, '') : `<span class="muted small">${ctrlCount(S.snap.controls) ? 'using defaults' : "the game's own"}</span>`;
    return `<details class="disc" data-disc="controls-${esc(g.key)}">
      <summary>Controls ${note}</summary>
      <div class="disc-body">
        <p class="hint">These rows override the manager's defaults for this game only; “Use defaults” inherits them. A row the game itself sets and the manager does not is left alone. With a controller, focus a row and use ← → (or A) to change it.</p>
        ${ctrlForm('game', g.controls, S.snap.controls, g.key)}
        <div class="actions" style="margin-top:10px"><button class="btn ghost sm" data-act="controls-reset-game" data-key="${esc(g.key)}">Reset this game</button></div>
      </div>
    </details>`;
  }

  // ---- bits -----------------------------------------------------------------------------------------------------

  const toneClass = (t) => `tone-${t || 'muted'}`;
  const pill = (text, tone) => `<span class="pill ${toneClass(tone)}"><i class="dot ${toneClass(tone)}"></i>${esc(text)}</span>`;
  const tag = (text, kind) => `<span class="tag ${kind || ''}">${esc(text)}</span>`;

  function shortLabel(label) {
    return ({ 'Set up the game files': 'Set up', 'Build the game code': 'Build', 'Add its base game': 'Add base', 'Create a recomp': 'Create' })[label] || label;
  }

  function metaLine(g) {
    return [g.publisher, g.version ? `v${g.version}` : '', g.app_name].filter(Boolean).join('  ·  ');
  }

  function heroMeta(g) {
    const bits = [];
    if (g.publisher) bits.push(g.publisher);
    if (g.version) bits.push(`v${g.version}`);
    if (g.title_id) bits.push(g.title_id.toUpperCase());
    if (g.app_name) bits.push(`${g.app_name} ${g.app_version || ''}`.trim());
    return bits.join('  ·  ');
  }

  function cover(g) {
    const initialsHtml = `<span class="initials">${esc(initials(g.title))}</span>`;
    const img = g.has_icon && g.title_id ? `<img data-cover="${esc(g.title_id)}" alt="">` : '';
    return `<div class="cover">${initialsHtml}${img}</div>`;
  }

  function loadingHtml(text) {
    return `<div class="page"><div class="loading"><span class="spinner"></span>${esc(text)}</div></div>`;
  }

  // ---- navigation -----------------------------------------------------------------------------------------------

  function renderNav() {
    const n = S.snap ? S.snap.games.length : 0;
    const items = [
      ['library', 'Library', 'grid', S.snap ? n : null],
      ['logs', 'Logs', 'doc', null],
      ['settings', 'Settings', 'sliders', null],
    ];
    nav.innerHTML = items
      .map(([id, label, icon, badge]) => {
        const on = S.view === id || (id === 'library' && S.view === 'game');
        return `<button class="nav-item ${on ? 'on' : ''}" data-nav="${id}">${ICONS[icon]}<span>${label}</span>${badge != null ? `<span class="badge">${badge}</span>` : ''}</button>`;
      })
      .join('');
    if (S.snap && S.snap.running > 0) {
      nav.insertAdjacentHTML('beforeend', `<div class="sidebar-run"><span class="spinner"></span>${S.snap.running} running</div>`);
    }
  }

  function keysState() {
    const s = S.snap || {};
    if (s.keys && s.keys_ok) return { tone: 'ok', name: baseName(s.keys), path: dirName(s.keys) };
    if (s.keys && !s.keys_ok) return { tone: 'error', name: 'missing', path: s.keys };
    if (s.keys_home) return { tone: 'muted', name: 'prod.keys', path: dirName(s.keys_home) };
    return { tone: 'warn', name: 'none chosen', path: 'prod.keys beside each package' };
  }

  function renderKeys() {
    if (!S.snap) {
      keysEl.innerHTML = '';
      return;
    }
    const k = keysState();
    keysEl.innerHTML = `<div class="keys-card">
      <div class="eyebrow">Keys</div>
      <div class="keys-name ${toneClass(k.tone)}"><i class="dot ${toneClass(k.tone)}"></i><span>${esc(k.name)}</span></div>
      <div class="keys-path" title="${esc(k.path)}">${esc(k.path)}</div>
      <div class="keys-actions">
        <button class="btn ghost sm" data-act="keys-change">Change…</button>
        ${S.snap.keys ? '<button class="btn ghost sm" data-act="keys-forget">Forget</button>' : ''}
      </div>
    </div>`;
  }

  // ---- library --------------------------------------------------------------------------------------------------

  function viewLibrary() {
    const s = S.snap;
    if (!s) return loadingHtml('Reading the library…');
    const ready = s.games.filter((g) => g.ready).length;
    const working = s.games.filter((g) => g.running).length;
    const line = `${s.games.length} game${s.games.length === 1 ? '' : 's'}  ·  ${ready} ready to play${working ? `  ·  ${working} working` : ''}`;
    return `<div class="page page-enter">
      <header class="head">
        <div>
          <div class="eyebrow">Collection</div>
          <h1>Library</h1>
          <p class="sub">${esc(line)}</p>
        </div>
        <div class="actions">
          <button class="btn ghost" data-act="refresh">Refresh</button>
          <button class="btn secondary" data-act="add-app">Add an nxrt app</button>
          <button class="btn primary" data-act="add-game">${ICONS.plus} Add a Switch game</button>
        </div>
      </header>
      ${s.games.length ? `<div class="grid">${s.games.map(cardHtml).join('')}</div>` : emptyLibrary()}
      ${s.games.length ? dropzone() : ''}
    </div>`;
  }

  function cardHtml(g) {
    const tone = g.running ? 'info' : g.tone;
    const state = g.running ? (g.running.playing ? 'Playing' : 'Working…') : g.state;
    const n = g.next;
    const btn = n
      ? `<button class="btn ${n.primary ? 'primary' : 'outline'} sm" data-act="next" data-key="${esc(g.key)}">${esc(shortLabel(n.label))}</button>`
      : `<button class="btn secondary sm" data-act="open" data-key="${esc(g.key)}">Open</button>`;
    return `<article class="game-card" data-card="${esc(g.key)}" tabindex="0" role="button" aria-label="${esc(g.title)}">
      ${cover(g)}
      <div class="title">${esc(g.title || 'Unknown game')}</div>
      <div class="meta" title="${esc(metaLine(g))}">${esc(metaLine(g))}</div>
      <div class="foot">
        <span class="state ${toneClass(tone)}"><i class="dot ${toneClass(tone)}"></i><span class="txt" title="${esc(state)}">${esc(state)}</span></span>
        ${btn}
      </div>
    </article>`;
  }

  function emptyLibrary() {
    return `<div class="empty">
      <div class="badge-icon">${ICONS.grid}</div>
      <h2>Your library is empty</h2>
      <p>Add a Switch game from its .nsp or .xci: its name, icon and version come from the package itself.</p>
      <div class="actions">
        <button class="btn primary" data-act="add-game">${ICONS.plus} Add a Switch game</button>
        <button class="btn secondary" data-act="add-app">Add an nxrt app</button>
      </div>
    </div>`;
  }

  function dropzone() {
    return `<button class="dropzone" data-act="add-game" id="dropzone">
      <span class="plus">${ICONS.plus}</span>
      <span class="two"><span class="one">Add a game</span><small>Drop an .nsp or .xci anywhere on the window, or choose one</small></span>
      <span class="btn secondary">Browse…</span>
    </button>`;
  }

  // ---- one game -------------------------------------------------------------------------------------------------

  function viewGame() {
    const g = game();
    if (!g) return viewLibrary();
    const running = g.running;
    const tone = running ? 'info' : g.tone;
    const state = running ? (running.playing ? 'Playing' : 'Working…') : g.state;
    const solo = !g.exe;
    return `<div class="page page-enter">
      <button class="back" data-act="back">${ICONS.back} Library</button>
      <section class="hero">
        ${cover(g)}
        <div class="hero-body">
          <h1>${esc(g.title)}</h1>
          <div class="meta" title="${esc(heroMeta(g))}">${esc(heroMeta(g))}</div>
          <div class="js-hero-pill">${pill(state, tone)}</div>
          <div class="actions">${heroActions(g)}</div>
        </div>
      </section>
      <div class="issue-row">
        <button class="btn ghost sm" data-act="issue">${ICONS.bug} Report an issue…</button>
        <span class="muted small">Opens the nxrt GitHub new-issue form with this game, its build and its last log filled in.</span>
      </div>
      ${
        g.build_mismatch
          ? `<p class="note error" style="margin-top:16px">This package is another version of the game than ${esc(g.app_name)} is for (v${esc((g.describe || {}).game_version || '')}).</p>`
          : ''
      }
      ${solo ? noApp(g) : setup(g)}
      ${solo ? '' : controlsCard(g)}
      ${
        solo
          ? `<div class="cols" style="grid-template-columns:1fr">${logCard(g)}</div>`
          : `<div class="cols">${placesCard(g)}${logCard(g)}</div>`
      }
      ${solo ? '' : details(g) + dev(g) + remove(g)}
    </div>`;
  }

  function heroActions(g) {
    if (g.running) return `<button class="btn secondary" data-act="stop">${ICONS.stop} Stop</button>`;
    if (!g.exe) return '';
    return `<button class="btn primary" data-act="play" ${g.ready ? '' : 'disabled'}>${ICONS.play} Play</button>
      <button class="btn secondary" data-act="start-screen">${ICONS.screen} Start screen</button>`;
  }

  function noApp(g) {
    const update =
      !g.package && g.update
        ? `<h2>This is the game's update</h2>
           <p>An update holds the game's new code and only its changes to the game's data: it is set up and played together with the base game (its .xci or .nsp).</p>
           <div class="actions"><button class="btn primary" data-act="add-base">Add its base game…</button></div>
           <div style="height:18px"></div>`
        : '';
    return `<section class="card noapp">
      ${update}
      <h2>No recomp runs this game yet</h2>
      <p>If its recomp exists, add its app. Otherwise make one from the package: a plain recompilation to start a port from.</p>
      <div class="actions">
        ${g.package ? '<button class="btn primary" data-act="create">Create a recomp</button>' : ''}
        <button class="btn secondary" data-act="choose-app">Choose its app…</button>
        <button class="btn danger" data-act="remove">Remove from the list</button>
      </div>
      ${g.running ? progressHtml(g) : g.last_exit ? exitHtml(g) : ''}
    </section>`;
  }

  function setup(g) {
    const steps = stepList(g);
    const done = steps.filter((s) => s.done).length;
    return `<section class="card setup">
      <div class="setup-head">
        <span class="section-title">Setup</span>
        <span class="small">${done} of ${steps.length} done</span>
        <span class="spacer"></span>
        <div class="menu">
          <button class="btn secondary sm" data-act="menu">Set up again ${ICONS.chevron}</button>
          <div class="menu-pop" hidden>
            <button data-act="setup-package">From a game package (.nsp / .xci)…</button>
            <button data-act="setup-folder">From a folder (exefs + romfs)…</button>
            ${g.steps.files ? '<button data-act="rebuild">Generate and compile again</button>' : ''}
          </div>
        </div>
      </div>
      <div class="steps">${steps.map((s) => stepHtml(g, s)).join('')}</div>
      ${progressHtml(g)}
    </section>`;
  }

  function stepList(g) {
    const r = g.running || {};
    const n = g.next || {};
    const list = [
      { title: 'Game files', detail: g.files_detail, done: g.steps.files, running: r.kind === 'files', next: n.kind === 'files', label: 'Set up', stage: 'files' },
    ];
    if (g.steps.builder || !g.steps.dev) {
      list.push({ title: 'Generate the game code', detail: "The game's ARM code, translated to C", done: g.steps.generated, running: r.kind === 'generate', next: n.kind === 'generate', label: 'Generate', stage: 'generate' });
      list.push({ title: 'Compile the game code', detail: "That C, built into the game's library", done: g.steps.code, running: r.kind === 'compile' || r.kind === 'all', next: n.kind === 'compile', label: 'Compile', stage: 'compile' });
    } else {
      list.push({ title: 'Build the game code', detail: "This app's Dev build command", done: g.steps.code, running: r.kind === 'all' || r.kind === 'build', next: n.kind === 'build', label: 'Build', stage: 'build' });
    }
    list.push({ title: 'Play', detail: 'The game in its own window', play: true, ready: g.ready, running: r.kind === 'play' });
    list.forEach((s, i) => (s.n = i + 1));
    return list;
  }

  function stepHtml(g, s) {
    const cls = s.play ? (s.running ? 'running' : s.ready ? 'play-ready' : '') : s.done ? 'done' : s.running ? 'running' : s.next ? 'next' : '';
    const head = s.play ? (s.running ? 'Playing' : s.ready ? 'Ready' : `Step ${s.n}`) : s.done ? `Step ${s.n} · done` : s.running ? `Step ${s.n} · running` : s.next ? `Step ${s.n} · next` : `Step ${s.n}`;
    const headTone = s.play ? (s.running ? 'tone-info' : s.ready ? 'tone-ok' : 'tone-muted') : s.done ? 'tone-ok' : s.running ? 'tone-info' : s.next ? 'tone-ok' : 'tone-muted';
    const circle = s.running ? '<span class="spinner"></span>' : s.play ? (s.ready ? ICONS.check : ICONS.play) : s.done ? ICONS.check : String(s.n);
    const act = s.next && s.stage ? `<div class="act"><button class="btn primary sm" data-act="next">${esc(s.label)}</button></div>` : '';
    return `<div class="step ${cls}">
      <div class="step-head"><span class="step-num">${circle}</span><span class="eyebrow ${headTone}">${esc(head)}</span></div>
      <div class="name">${esc(s.title)}</div>
      <div class="detail">${esc(s.detail || '')}</div>
      ${act}
    </div>`;
  }

  function progressHtml(g) {
    const r = g.running;
    if (r && r.playing) return '';
    if (!r) {
      if (g.last_exit) return exitHtml(g);
      return '';
    }
    const pct = r.progress != null ? Math.round(r.progress * 100) : null;
    return `<div class="progress js-progress">
      <div class="row">
        ${r.playing ? '' : '<span class="spinner"></span>'}
        <span>${esc(r.step || (r.playing ? 'Playing' : 'Starting…'))}</span>
        ${pct != null ? `<span class="pct">${pct}%</span>` : ''}
        <button class="btn ghost sm" data-act="stop">${r.playing ? 'Stop' : 'Cancel'}</button>
      </div>
      ${pct != null ? `<div class="bar"><i style="width:${pct}%"></i></div>` : ''}
      ${r.playing ? '' : '<div class="note">The next step asks to start when this one is done.</div>'}
    </div>`;
  }

  function exitHtml(g) {
    const e = g.last_exit;
    if (!e) return '';
    return `<div class="js-progress exit ${toneClass(e.ok ? 'ok' : 'error')}"><i class="dot ${toneClass(e.ok ? 'ok' : 'error')}"></i>${esc(e.text)}</div>`;
  }

  function placesCard(g) {
    const rows = [];
    const add = (name, tagText, tagKind, about, path, file) => path && rows.push({ name, tagText, tagKind, about, path, file });
    add('Game package', 'keep', 'keep', ABOUT.package, g.package, true);
    add('Update', 'keep', 'keep', ABOUT.update, g.update, true);
    add('Data folder', 'back up', 'backup', ABOUT.data, g.data_root, false);
    add('Game folder', 'rebuildable', 'rebuild', ABOUT.game, g.game_dir, false);
    add('App', 'keep', 'keep', ABOUT.app, g.exe, true);
    add('Recomp folder', 'source', 'source', ABOUT.recomp, g.recomp_dir, false);
    add('Keys', 'keep', 'keep', ABOUT.keys, g.keys, true);
    return `<section class="card places">
      <div class="section-title">Where things are</div>
      ${rows
        .map(
          (r) => `<div class="place">
        <div class="who">
          <div class="name-row"><span class="name" title="${esc(r.about)}">${esc(r.name)}</span>${tag(r.tagText, r.tagKind)}</div>
          <div class="path" title="${esc(r.path)}">${esc(r.path)}</div>
        </div>
        <button class="btn ghost sm" data-act="reveal" data-path="${esc(r.path)}" data-file="${r.file ? '1' : '0'}">${r.file ? 'Show' : 'Open'}</button>
      </div>`
        )
        .join('')}
    </section>`;
  }

  function logCard(g) {
    const file = (g.running && g.running.log_file) || '';
    return `<section class="card log-card">
      <div class="log-head"><span class="section-title">Log</span>${file ? `<span class="file">${esc(file)}</span>` : ''}</div>
      <div class="log-well js-log">${logHtml(g)}</div>
    </section>`;
  }

  function logHtml(g) {
    const log = (g.running && g.running.log) || [];
    if (log.length) return `<pre class="log-pre">${log.map(esc).join('\n')}</pre>`;
    if (g.running) {
      return `<div class="log-empty"><div class="ring"><span class="spinner"></span></div><div class="t">Waiting for output…</div><div class="s">The app has started; its first lines are on the way.</div></div>`;
    }
    return `<div class="log-empty"><div class="ring">${ICONS.doc}</div><div class="t">Nothing running</div><div class="s">The log of a step or a game, live. Every run is kept in Logs.</div></div>`;
  }

  function details(g) {
    const rows = [];
    const row = (k, v, cls = '') => rows.push(`<dt>${esc(k)}</dt><dd class="${cls}">${v}</dd>`);
    if (g.describe) {
      row('App version', esc(g.describe.version));
      row('Game build', esc(g.describe.build_id));
      row('Game code id', esc(g.describe.gamelib_id));
    }
    if (g.load_error) row('App', `<span class="err">${esc(g.load_error)}</span>`);
    if (g.status) {
      if (!g.status.files_ready) row('Game files', esc(g.status.files));
      if (!g.status.ready && g.status.status) row('The app says', esc(g.status.status));
      row('Game-code builder', g.status.builder ? 'in the app' : 'none (a dev build: Dev build below)');
    }
    if (g.files && g.files.build_id && g.describe) {
      const ok = (g.files.build_id || '').toLowerCase() === (g.describe.build_id || '').toLowerCase();
      row("Files' build", `${esc(g.files.build_id)}${ok ? '' : '  (not the version the app is for)'}`, ok ? '' : 'err');
    }
    return `<details class="disc" data-disc="details-${esc(g.key)}">
      <summary>Details</summary>
      <div class="disc-body"><dl class="kv">${rows.join('')}</dl></div>
    </details>`;
  }

  function dev(g) {
    const id = esc(g.key);
    return `<details class="disc" data-disc="dev-${id}">
      <summary>Dev build</summary>
      <div class="disc-body">
        <p class="hint">For an app built from source without a game-code builder: what builds its game code, and where.</p>
        <div class="dev-grid">
          <label>Folder</label>
          <div class="row" style="display:flex;gap:8px">
            <input id="dev-dir-${id}" value="${esc(g.build_dir || '')}" placeholder="the game's source checkout">
            <button class="btn secondary sm" data-act="browse-dev-dir">Browse…</button>
          </div>
          <label>Command</label>
          <input id="dev-cmd-${id}" value="${esc(g.build_cmd || '')}" placeholder="make gen && make build/game">
        </div>
        <div class="actions"><button class="btn secondary" data-act="save-dev">Save</button></div>
      </div>
    </details>`;
  }

  function remove(g) {
    return `<div style="margin-top:16px"><button class="btn danger" data-act="remove">Remove from the library</button></div>`;
  }


  // ---- controller map: the pad, its lit buttons and the bindings it sends ------------------------------------------
  // (the nxrt built-in defaults, as input -> host; the global defaults override them; a game's Controls may differ)
  const DEFAULT_PAD_BINDINGS = {
    a: 'east', b: 'south', x: 'north', y: 'west',
    l: 'lshoulder', r: 'rshoulder', zl: 'ltrigger', zr: 'rtrigger',
    plus: 'start', minus: 'back', lstick: 'lstick', rstick: 'rstick',
    left: 'dpad_left', up: 'dpad_up', right: 'dpad_right', down: 'dpad_down',
  };
  const TEST_HOST = {
    south: 'south', east: 'east', north: 'north', west: 'west',
    lb: 'lshoulder', rb: 'rshoulder', lt: 'ltrigger', rt: 'rtrigger',
    start: 'start', select: 'back', mode: 'guide', l3: 'lstick', r3: 'rstick',
    up: 'dpad_up', down: 'dpad_down', left: 'dpad_left', right: 'dpad_right',
  };
  const ctrlInputLabel = (k) => (CTRL_INPUTS.find(([x]) => x === k) || [k, k])[1];
  function effectivePadMap() {
    return { ...DEFAULT_PAD_BINDINGS, ...((S.snap && S.snap.controls && S.snap.controls.bind_pad) || {}) };
  }
  function inputsForHost(host) {
    const m = effectivePadMap();
    return Object.keys(m).filter((k) => m[k] === host);
  }
  function hostName(host) {
    const L = padLabels(S.pad.name);
    switch (host) {
      case 'south': return `Bottom (${L.south})`;
      case 'east': return `Right (${L.east})`;
      case 'north': return `Top (${L.north})`;
      case 'west': return `Left (${L.west})`;
      case 'lshoulder': return 'LB / L1';
      case 'rshoulder': return 'RB / R1';
      case 'ltrigger': return 'LT / L2';
      case 'rtrigger': return 'RT / R2';
      case 'back': return 'Back / Minus';
      case 'start': return 'Start / Plus';
      case 'guide': return 'Guide';
      case 'lstick': return 'Left stick press';
      case 'rstick': return 'Right stick press';
      case 'dpad_up': return 'D-pad Up';
      case 'dpad_down': return 'D-pad Down';
      case 'dpad_left': return 'D-pad Left';
      case 'dpad_right': return 'D-pad Right';
      default: return host;
    }
  }
  function padTestCaption() {
    if (!S.pad.connected) return 'Connect a controller to test its buttons.';
    if (!S.padLast) return S.padTest ? 'Press any button…' : 'Press Test, then a button.';
    const inputs = inputsForHost(S.padLast).map(ctrlInputLabel).join(', ');
    return `${esc(hostName(S.padLast))} <span class="arrow">→</span> ${inputs ? `<b>${esc(inputs)}</b>` : '<span class="muted">not bound</span>'}`;
  }
  function controllerSvg() {
    const L = padLabels(S.pad.name);
    const mapped = (host) => inputsForHost(host).map(ctrlInputLabel).join(' · ') || '–';
    const on = (host) => (S.padHeld.has(host) ? ' on' : '');
    const group = (host, inner) => `<g class="pad-host${on(host)}" data-host="${host}">${inner}</g>`;
    const face = (host, x, y, glyph) =>
      group(host, `<circle class="pad-btn" cx="${x}" cy="${y}" r="16"/><text class="pad-glyph" x="${x}" y="${y + 4}" text-anchor="middle">${esc(glyph)}</text><text class="pad-map" data-map="${host}" x="${x}" y="${y + 29}" text-anchor="middle">${esc(mapped(host))}</text>`);
    const shoulder = (host, x, label) =>
      group(host, `<rect class="pad-shoulder" x="${x}" y="70" width="118" height="20" rx="10"/><text class="pad-key" x="${x + 11}" y="84">${label}</text><text class="pad-map pad-map-in" data-map="${host}" x="${x + 107}" y="84" text-anchor="end">${esc(mapped(host))}</text>`);
    const trigger = (host, x, label) =>
      group(host, `<rect class="pad-trigger" x="${x}" y="48" width="82" height="16" rx="8"/><text class="pad-key" x="${x + 10}" y="59">${label}</text><text class="pad-map pad-map-in" data-map="${host}" x="${x + 72}" y="59" text-anchor="end">${esc(mapped(host))}</text>`);
    const stick = (host, x, y, label) =>
      group(host, `<circle class="pad-stick-out" cx="${x}" cy="${y}" r="28"/><circle class="pad-stick-in" cx="${x}" cy="${y}" r="19"/><text class="pad-glyph" x="${x}" y="${y + 3}" text-anchor="middle" font-size="8">${label}</text><text class="pad-map" data-map="${host}" x="${x}" y="${y + 45}" text-anchor="middle">${esc(mapped(host))}</text>`);
    const dir = (host, x, y, w, h) => group(host, `<rect class="pad-dir" x="${x}" y="${y}" width="${w}" height="${h}" rx="3"/>`);
    const pill = (host, x, glyph) =>
      group(host, `<rect class="pad-btn" x="${x}" y="110" width="24" height="13" rx="6.5"/><text class="pad-glyph" x="${x + 12}" y="120" text-anchor="middle" font-size="8">${glyph}</text><text class="pad-map" data-map="${host}" x="${x + 12}" y="134" text-anchor="middle">${esc(mapped(host))}</text>`);
    return `<svg viewBox="0 0 560 350" role="img" aria-label="Controller with its button bindings">
      <path class="pad-body" d="M132 92 C78 92 58 136 66 206 C74 278 108 316 150 306 C192 296 205 252 232 244 L328 244 C355 252 368 296 410 306 C452 316 486 278 494 206 C502 136 482 92 428 92 Z"/>
      ${trigger('ltrigger', 120, 'LT')}${trigger('rtrigger', 358, 'RT')}
      ${shoulder('lshoulder', 102, 'LB')}${shoulder('rshoulder', 340, 'RB')}
      ${stick('lstick', 176, 156, 'L3')}${stick('rstick', 348, 245, 'R3')}
      ${dir('dpad_up', 166, 223, 20, 22)}${dir('dpad_down', 166, 265, 20, 22)}
      ${dir('dpad_left', 146, 245, 22, 20)}${dir('dpad_right', 184, 245, 22, 20)}
      <rect class="pad-dir-core" x="166" y="245" width="20" height="20" rx="3"/>
      ${pill('back', 238, '⧉')}${pill('start', 298, '☰')}
      ${group('guide', '<circle class="pad-guide" cx="280" cy="88" r="10"/>')}
      ${face('north', 392, 118, L.north)}${face('east', 436, 162, L.east)}
      ${face('south', 392, 206, L.south)}${face('west', 348, 162, L.west)}
    </svg>`;
  }
  function controllerMapCard() {
    const connected = S.pad.connected;
    return `<section class="card setting controller-map-card">
      <div class="setting-head">
        <span class="section-title">Controller map</span>
        ${connected ? pill(S.pad.name || 'connected', 'ok') : pill('none connected', 'muted')}
        <span class="spacer"></span>
        <button class="btn secondary sm" data-act="${S.padTest ? 'pad-test-stop' : 'pad-test'}" ${connected ? '' : 'disabled'}>${S.padTest ? 'Stop' : 'Test'}</button>
      </div>
      <p>${S.padTest ? 'Press any button on the controller; the map lights and the line below names the nxrt input it sends. Esc or Stop finishes.' : 'Your controller defaults at a glance: the large label is the pad’s button, the small one the nxrt input it sends. Test to light each button as you press it.'}</p>
      <div class="controller-map">${controllerSvg()}</div>
      <div class="pad-test-caption">${padTestCaption()}</div>
      <p class="small">A game's Controls can override these. The diagram covers the face, shoulders, triggers, sticks and D-pad; paddles and touchpads are in the bindings list below.</p>
    </section>`;
  }
  function updatePadMap() {
    const root = document.querySelector('.controller-map');
    if (root) {
      root.querySelectorAll('[data-host]').forEach((el) => el.classList.toggle('on', S.padHeld.has(el.dataset.host)));
      root.querySelectorAll('[data-map]').forEach((el) => {
        const inputs = inputsForHost(el.dataset.map).map(ctrlInputLabel).join(' · ');
        el.textContent = inputs || '–';
      });
    }
    const cap = document.querySelector('.pad-test-caption');
    if (cap) cap.innerHTML = padTestCaption();
  }
  function testPadPress(name, pressed) {
    const host = TEST_HOST[name];
    if (!host) return;
    if (pressed) {
      S.padHeld.add(host);
      S.padLast = host;
    } else {
      S.padHeld.delete(host);
    }
    updatePadMap();
  }
  function startPadTest() {
    if (!S.pad.connected) return toast('Connect a controller first', 'warn');
    S.padTest = true;
    S.padHeld.clear();
    S.padLast = null;
    stopRepeat();
    heldDirs.clear();
    render();
    renderHints();
  }
  function stopPadTest() {
    S.padTest = false;
    S.padHeld.clear();
    S.padLast = null;
    render();
    renderHints();
  }


  // ---- updates: a small public manifest, a quiet sidebar chip ------------------------------------------------
  async function checkForUpdate(force = false) {
    try {
      S.update = await invoke('check_update', { force });
    } catch (e) {
      S.update = { available: false, error: errText(e), current: (S.snap && S.snap.version) || '', latest: '' };
    }
    renderUpdate();
    if (S.view === 'settings') render();
  }

  function renderUpdate() {
    const el = document.getElementById('update');
    if (!el) return;
    const u = S.update;
    if (!u || !u.available) {
      el.hidden = true;
      el.innerHTML = '';
      return;
    }
    const title = u.notes || `v${u.latest} is available`;
    el.hidden = false;
    el.innerHTML = `<button class="update-chip" data-act="open-update" title="${esc(title)}"><span class="update-dot"></span><span>Update available</span><span class="update-ver">v${esc(u.latest)}</span></button>`;
  }

  function updateStatus() {
    const u = S.update;
    const current = (S.snap && S.snap.version) || (u && u.current) || '';
    if (!u) return 'checking…';
    if (u.error) {
      const err = u.error.length > 140 ? `${u.error.slice(0, 137)}…` : u.error;
      return `could not check <span class="muted">(${esc(err)})</span>`;
    }
    if (u.available) return `<b>v${esc(u.latest)}</b> available — use the sidebar`;
    return `up to date <span class="muted">(v${esc(current)})</span>`;
  }

  async function openUpdate() {
    const u = S.update;
    if (!u || !u.available || !u.url) return;
    try {
      await invoke('open_update', { url: u.url });
    } catch (e) {
      toast(errText(e), 'error');
    }
  }

  // ---- logs -----------------------------------------------------------------------------------------------------

  function viewLogs() {
    if (S.logs === null && !S.logsBusy) loadLogs();
    if (S.manager === null && !S.managerBusy) loadManager();
    if (S.manager && S.managerFollow) scheduleManager();
    const list = (S.logs || []).filter((r) => r.kind !== 'manager');
    return `<div class="page page-enter">
      <header class="head">
        <div>
          <div class="eyebrow">Activity</div>
          <h1>Logs</h1>
          <p class="sub">${esc(ABOUT.logs)}</p>
        </div>
        <div class="actions"><button class="btn secondary" data-act="open-logs-dir">Open the folder</button></div>
      </header>
      ${managerCard()}
      <section class="card logs-card">
        ${
          S.logs === null
            ? `<div class="loading" style="min-height:220px"><span class="spinner"></span>Reading the logs…</div>`
            : list.length
            ? `<div class="logs-head"><span class="eyebrow">Runs</span><span class="eyebrow">${list.length} log${list.length === 1 ? '' : 's'}</span></div>${list.map(logRow).join('')}`
            : `<div class="empty" style="padding:36px 0"><div class="badge-icon">${ICONS.doc}</div><h2 style="font-size:14px">No runs yet</h2><p style="margin-bottom:0;font-size:11px">Every step and every game started from here keeps its log in this folder.</p></div>`
        }
      </section>
    </div>`;
  }

  const managerChip = (level) =>
    `<button class="chip${S.managerLevel === level ? ' on' : ''}" data-act="manager-level" data-level="${level}">${level === 'all' ? 'All' : level.toUpperCase()}</button>`;

  function managerCard() {
    const m = S.manager;
    return `<section class="card logs-card manager-card">
      <div class="setting-head">
        <span class="section-title">Manager</span><span id="manager-filter" class="tag">${esc((m && m.filter) || 'info')}</span>
        <span class="spacer"></span>
        <button class="btn ghost sm" data-act="manager-refresh">Refresh</button>
        <button class="btn secondary" data-act="open-logs-dir">Folder</button>
      </div>
      <p class="hint">The manager's own log: one line per event, a daily file, seven kept. <span class="mono" id="manager-path" title="${esc((m && m.path) || '')}">${esc((m && m.path) || '')}</span></p>
      <div class="log-tools">
        ${['all', 'error', 'warn', 'info', 'debug', 'trace'].map(managerChip).join('')}
        <input id="manager-search" type="search" placeholder="Filter lines…" value="${esc(S.managerSearch)}">
        <label class="follow"><input type="checkbox" id="manager-follow"${S.managerFollow ? ' checked' : ''}> Follow</label>
      </div>
      <div class="log-well manager-tail" id="manager-tail">${managerTailHtml()}</div>
    </section>`;
  }

  function managerTailHtml() {
    if (!S.manager) return `<div class="manager-empty"><span class="spinner"></span>Reading the manager log…</div>`;
    const q = S.managerSearch.trim().toLowerCase();
    const re = S.managerLevel === 'all' ? null : new RegExp(`\\b${S.managerLevel.toUpperCase()}\\b`);
    const lines = (S.manager.lines || []).filter((l) => (!re || re.test(l)) && (!q || l.toLowerCase().includes(q)));
    if (!lines.length) return `<div class="manager-empty">No lines match${q ? ` “${esc(S.managerSearch)}”` : ''}.</div>`;
    return lines
      .map((l) => {
        const m = l.match(/\b(ERROR|WARN|INFO|DEBUG|TRACE)\b/);
        return `<div class="log-line${m ? ' ' + m[1].toLowerCase() : ''}">${esc(l)}</div>`;
      })
      .join('');
  }

  async function loadManager(silent = false) {
    if (S.view !== 'logs' || S.managerBusy) return;
    S.managerBusy = true;
    try {
      S.manager = await invoke('manager_log', { lines: 300 });
    } catch (e) {
      if (!silent) toast(errText(e), 'error');
    } finally {
      S.managerBusy = false;
      if (S.view === 'logs') {
        const tail = document.getElementById('manager-tail');
        if (!tail) {
          render();
        } else {
          tail.innerHTML = managerTailHtml();
          const path = document.getElementById('manager-path');
          if (path) {
            path.textContent = S.manager?.path || '';
            path.title = S.manager?.path || '';
          }
          const filter = document.getElementById('manager-filter');
          if (filter) filter.textContent = S.manager?.filter || 'info';
        }
        scheduleManager();
      }
    }
  }

  function scheduleManager() {
    clearTimeout(S.managerTimer);
    if (S.view === 'logs' && S.managerFollow) S.managerTimer = setTimeout(() => loadManager(true), 3000);
  }

  function logBits(name) {
    if (String(name || '').startsWith('manager.log')) return { kind: 'manager', game: '' };
    const stem = String(name || '').replace(/\.log$/, '');
    let parts = stem.split('-').filter(Boolean);
    if (parts.length && /^\d+$/.test(parts[parts.length - 1])) parts.pop();
    const stages = ['files', 'generate', 'compile', 'build', 'create', 'play'];
    const last = parts[parts.length - 1];
    if (stages.includes(last)) return { kind: last, game: parts.slice(0, -1).join('-') };
    return { kind: '', game: parts.join('-') };
  }

  function logRow(row) {
    const { kind, game } = logBits(row.name);
    return `<div class="log-row">
      <div class="fileline">${kind ? `<span class="kind">${esc(kind.toUpperCase())}</span>` : ''}<span class="fname">${esc(row.name)}</span>${game ? `<span class="game">${esc(game)}</span>` : ''}</div>
      <span class="when">${esc(ago(row.mtime))}</span>
      <span class="size">${esc(bytes(row.bytes))}</span>
      <button class="btn ghost sm" data-act="show-log" data-path="${esc(row.path)}">Show</button>
    </div>`;
  }

  async function loadLogs() {
    S.logsBusy = true;
    try {
      S.logs = await invoke('logs');
    } catch (e) {
      toast(errText(e), 'error');
      S.logs = [];
    } finally {
      S.logsBusy = false;
      if (S.view === 'logs') render();
    }
  }

  // ---- settings -------------------------------------------------------------------------------------------------

  function viewSettings() {
    if (!S.snap) return loadingHtml('Reading the settings…');
    const s = S.snap;
    const k = keysState();
    return `<div class="page page-enter">
      <header class="head">
        <div><div class="eyebrow">Preferences</div><h1>Settings</h1><p class="sub">The keys games are read with, and where the manager keeps its own things.</p></div>
      </header>
      <div class="settings-col">
        <section class="card setting">
          <div class="setting-head">
            <span class="section-title">Keys</span>${pill(k.name, k.tone)}<span class="spacer"></span>
            ${s.keys ? '<button class="btn ghost sm" data-act="keys-forget">Forget</button>' : ''}
            <button class="btn secondary" data-act="keys-change">Choose prod.keys…</button>
          </div>
          <p>${esc(ABOUT.keys)}</p>
          <div class="path" title="${esc(k.path)}">${esc(k.path)}</div>
        </section>
        <section class="card setting">
          <div class="setting-head">
            <span class="section-title">Controller (this window)</span>
            ${S.pad.connected ? pill(S.pad.name || 'connected', 'ok') : pill(S.pad.error ? 'not available' : 'none connected', S.pad.error ? 'error' : 'muted')}
            <span class="spacer"></span>
            <button class="btn secondary" data-act="pad-help">Controls…</button>
          </div>
          <p>Any controller works over USB or Bluetooth, and is picked up while the window is focused. The D-pad or left stick moves, ${padLabels(S.pad.name).south} selects, ${padLabels(S.pad.name).east} goes back, and LB / RB change section.</p>
          ${S.pad.error ? `<p style="color:var(--amber)">${esc(S.pad.error)} — macOS may need Input Monitoring for the app.</p>` : ''}
          <p class="small">If a connected controller isn't seen, allow Input Monitoring for nxrt in System Settings › Privacy &amp; Security.</p>
          ${padHelpGrid()}
        </section>
        ${controllerMapCard()}
        ${controlsDefaultsCard()}
        <section class="card setting">
          <div class="setting-head"><span class="section-title">New recomps</span></div>
          <p>Where Create a recomp puts them. The nxrt source is built into the app; set <span class="mono">NXRT_DIR</span> to use another checkout while developing.</p>
          <div class="path">${esc(s.recomps_dir)}<br><span class="muted">nxrt</span>&nbsp; ${esc(s.nxrt_dir)}</div>
        </section>
        <section class="card setting">
          <div class="setting-head"><span class="section-title">The manager's own files</span><span class="spacer"></span><button class="btn secondary" data-act="open-config">Show in Finder</button></div>
          <p>The library (library.toml), the games' icons and the logs.</p>
          <div class="path">${esc(s.config_dir)}</div>
          <div style="border-top:1px solid var(--line);margin:14px 0 12px"></div>
          <div style="display:flex;align-items:center;gap:12px">
            <div style="flex:1"><div style="font-size:12px">nxrt-frontend</div><div class="muted small">A manager for the nxrt Switch recomps.</div></div>
            ${tag(`v${s.version}`, '')}
          </div>
        </section>
        <section class="card setting">
          <div class="setting-head"><span class="section-title">Diagnostics</span><span class="spacer"></span><button class="btn secondary" data-act="go-logs">Manager log</button></div>
          <p>The manager's own log: level <span class="mono">${esc((s.log || {}).filter || 'info')}</span>, a daily file, seven kept. Set <span class="mono">NXRT_FRONTEND_LOG=debug</span> for more detail; <span class="mono">NXRT_GAME_LOG=debug</span> passes a level to a game's nxrt log.</p>
          <div class="path">${esc((s.log || {}).path || (s.log || {}).dir || '')}</div>
          <div style="border-top:1px solid var(--line);margin:12px 0"></div>
          <div class="update-line">
            <span class="update-status">${updateStatus()}</span>
            <span class="spacer"></span>
            <button class="btn ghost sm" data-act="check-update">Check now</button>
          </div>
        </section>
      </div>
    </div>`;
  }

  // ---- sheets ---------------------------------------------------------------------------------------------------

  let createDirTouched = false;

  let sheetOpener = null;

  function openSheet(html) {
    sheetOpener = selOf(document.activeElement);
    sheet.innerHTML = html;
    if (!sheet.open) sheet.showModal();
    const first = sheet.querySelector('input') || sheet.querySelector('.btn.primary') || sheet.querySelector('button');
    if (first) setTimeout(() => setFocus(first), 30);
    renderHints();
  }

  function closeSheet() {
    if (sheet.open) sheet.close();
    renderHints();
    if (S.padMode && sheetOpener) {
      const el = document.querySelector(sheetOpener);
      if (el) setFocus(el);
    }
    sheetOpener = null;
  }

  function addAppSheet() {
    openSheet(`<div class="sheet-head"><h2 id="sheet-title">Add an nxrt app</h2><button class="btn ghost icon" data-close aria-label="Close">${ICONS.x}</button></div>
      <div class="sheet-body">
        <p class="hint">A recomp's app: fe3hrecomp, eowrecomp, lasrecomp …</p>
        <div class="field">
          <label for="app-path">App</label>
          <div class="row"><input id="app-path" placeholder="path to the app" autocomplete="off"><button class="btn secondary" data-act="browse-app">Browse…</button></div>
        </div>
        <div class="field">
          <label for="app-data">Data folder <span class="muted">(optional)</span></label>
          <div class="row"><input id="app-data" placeholder="default"><button class="btn secondary" data-act="browse-data">Browse…</button></div>
          <div class="hintline">Its settings, saves and mods. Leave empty for the app's own.</div>
        </div>
        <div class="err" id="sheet-err" style="color:var(--red);font-size:11px"></div>
      </div>
      <div class="sheet-foot">${padSheetHints()}<button class="btn ghost" data-close>Cancel</button><button class="btn primary" data-act="submit-app">Add</button></div>`);
  }

  function createSheet(g) {
    createDirTouched = false;
    const id = slugId(g.title);
    const dir = `${S.snap.recomps_dir}/${id}`;
    openSheet(`<div class="sheet-head"><h2 id="sheet-title">Create a recomp of ${esc(g.title)}</h2><button class="btn ghost icon" data-close aria-label="Close">${ICONS.x}</button></div>
      <div class="sheet-body">
        <p class="hint">A new nxrt game folder from the game's package: who it is, its code modules, its game files (read from the package in place), and its app built. Then generate and compile its game code as for any game. A plain recompilation: a port starts from here.</p>
        <div class="field">
          <label for="c-id">Id</label>
          <div class="row"><input id="c-id" value="${esc(id)}" style="max-width:170px"><span class="hintline" style="align-self:center">the app: <span id="c-id-hint" class="mono">${esc(id)}</span>recomp</span></div>
        </div>
        <div class="field">
          <label for="c-dir">Folder</label>
          <div class="row"><input id="c-dir" value="${esc(dir)}"><button class="btn secondary" data-act="browse-create-dir">Browse…</button></div>
        </div>
        <p class="hint" style="margin-bottom:0">Needs: the Xcode command line tools, Python 3, Homebrew's SDL3 and FFmpeg. The app builds in a minute or two; its game code is the next steps.</p>
        <div class="err" id="sheet-err" style="color:var(--red);font-size:11px;margin-top:8px"></div>
      </div>
      <div class="sheet-foot">${padSheetHints()}<button class="btn ghost" data-close>Cancel</button><button class="btn primary" data-act="submit-create">Create</button></div>`);
    const idEl = sheet.querySelector('#c-id');
    const dirEl = sheet.querySelector('#c-dir');
    idEl.addEventListener('input', () => {
      sheet.querySelector('#c-id-hint').textContent = idEl.value;
      if (!createDirTouched) dirEl.value = `${S.snap.recomps_dir}/${idEl.value}`;
    });
    dirEl.addEventListener('input', () => (createDirTouched = true));
  }

  function slugId(name) {
    const words = String(name || '').split(/[^A-Za-z0-9]+/).filter(Boolean);
    const id = (words.length >= 2 ? words.map((w) => w[0]).join('') : words.join('')).toLowerCase().replace(/[^a-z0-9]/g, '');
    return id || 'game';
  }

  // ---- toasts ---------------------------------------------------------------------------------------------------

  function toast(text, kind = '') {
    const el = document.createElement('div');
    el.className = `toast ${kind}`;
    const icon = kind === 'error' ? ICONS.x : kind === 'ok' ? ICONS.check : kind === 'busy' ? '<span class="spinner"></span>' : '';
    el.innerHTML = `${icon ? `<span class="ico">${icon}</span>` : ''}<span>${esc(text)}</span>`;
    toasts.append(el);
    if (kind !== 'busy') {
      setTimeout(() => {
        el.style.opacity = '0';
        el.style.transform = 'translateY(4px)';
        setTimeout(() => el.remove(), 200);
      }, kind === 'error' ? 7000 : 4200);
    }
    return () => el.remove();
  }

  // ---- state & rendering ----------------------------------------------------------------------------------------

  const game = () => (S.snap ? S.snap.games.find((g) => g.key === S.key) : null);

  function render() {
    const scroll = main.scrollTop;
    const same = S.lastView === S.view && S.lastKey === S.key;
    const prev = sheet.open ? null : selOf(document.activeElement);
    if (prev && (main.contains(document.activeElement) || nav.contains(document.activeElement))) S.focusSel = prev;
    renderNav();
    renderKeys();
    renderUpdate();
    main.innerHTML =
      S.view === 'game' ? viewGame() : S.view === 'logs' ? viewLogs() : S.view === 'settings' ? viewSettings() : viewLibrary();
    S.lastView = S.view;
    S.lastKey = S.key;
    main.scrollTop = same ? scroll : 0;
    hydrateIcons();
    hydrateLog();
    restoreDisc();
    renderPad();
    renderHints();
    if (S.padMode && !sheet.open) {
      if (S.wantFocusFirst) {
        S.wantFocusFirst = false;
        focusFirst();
      } else {
        const el = (S.pendingFocus && document.querySelector(S.pendingFocus)) || (S.focusSel && document.querySelector(S.focusSel));
        S.pendingFocus = null;
        if (el) setFocus(el);
        else focusFirst();
      }
    }
  }

  function go(view, key = null) {
    if (S.padTest && view !== 'settings') {
      S.padTest = false;
      S.padHeld.clear();
      S.padLast = null;
    }
    S.view = view;
    S.key = key;
    if (S.padMode) {
      S.wantFocusFirst = true;
      S.focusSel = null;
      S.pendingFocus = null;
    }
    if (view === 'logs') loadLogs();
    render();
  }

  async function load(silent = false) {
    if (S.loading) {
      S.queued = true;
      return;
    }
    S.loading = true;
    try {
      const snap = await invoke('snapshot_command');
      S.snap = snap;
      if (S.view === 'game' && !snap.games.some((g) => g.key === S.key)) {
        S.view = 'library';
        S.key = null;
      }
      render();
      applyPendingView();
    } catch (e) {
      toast(errText(e), 'error');
      if (!S.snap) main.innerHTML = `<div class="page"><div class="loading">Could not read the library.<br>${esc(errText(e))}</div></div>`;
    } finally {
      S.loading = false;
      if (S.queued) {
        S.queued = false;
        setTimeout(() => load(true), 120);
      }
    }
  }

  function queueLoad() {
    clearTimeout(S.loadTimer);
    S.loadTimer = setTimeout(() => load(true), 220);
  }

  async function hydrateIcons() {
    for (const img of document.querySelectorAll('img[data-cover]')) {
      const tid = img.dataset.cover;
      if (!tid || img.dataset.hydrated) continue;
      img.dataset.hydrated = '1';
      let url = S.icons.get(tid);
      if (url === undefined) {
        try {
          url = await invoke('icon', { titleId: tid });
        } catch {
          url = null;
        }
        S.icons.set(tid, url);
      }
      if (url) {
        img.src = url;
        img.classList.add('on');
      }
    }
  }

  function hydrateLog() {
    const pre = document.querySelector('.log-pre');
    if (pre) pre.scrollTop = pre.scrollHeight;
  }

  function restoreDisc() {
    document.querySelectorAll('details[data-disc]').forEach((d) => {
      d.open = S.disc.has(d.dataset.disc);
      d.addEventListener('toggle', () => {
        if (d.open) S.disc.add(d.dataset.disc);
        else S.disc.delete(d.dataset.disc);
      });
    });
  }

  // ---- actions --------------------------------------------------------------------------------------------------

  async function addGame() {
    if (!S.snap) return;
    if (!S.snap.keys_ok && !S.snap.keys_home) {
      const path = await invoke('pick_keys');
      if (!path) return;
      try {
        S.snap = await invoke('set_keys', { path });
        render();
      } catch (e) {
        return toast(errText(e), 'error');
      }
    }
    const path = await invoke('pick_package');
    if (path) await addPackagePath(path);
  }

  async function addPackagePath(path) {
    const done = toast('Reading the package…', 'busy');
    try {
      const snap = await invoke('add_package', { path });
      S.snap = snap;
      const g = snap.games.find((x) => samePath(x.package, path) || samePath(x.update, path));
      if (g) {
        S.view = 'game';
        S.key = g.key;
      }
      render();
      toast(g ? `Added ${g.title}` : 'Added the game', 'ok');
      loadLogsSoon();
    } catch (e) {
      toast(errText(e), 'error');
    } finally {
      done();
    }
  }

  async function addBase() {
    const path = await invoke('pick_package');
    if (!path) return;
    await addPackagePath(path);
  }

  async function removeGame() {
    const g = game();
    if (!g) return;
    try {
      S.snap = await invoke('remove', { index: g.index });
      S.view = 'library';
      S.key = null;
      render();
      toast('Removed from the list', 'ok');
    } catch (e) {
      toast(errText(e), 'error');
    }
  }

  async function changeKeys() {
    const path = await invoke('pick_keys');
    if (!path) return;
    try {
      S.snap = await invoke('set_keys', { path });
      render();
      toast('Keys chosen', 'ok');
    } catch (e) {
      toast(errText(e), 'error');
    }
  }

  async function forgetKeys() {
    try {
      S.snap = await invoke('forget_keys');
      render();
      toast('Keys forgotten', 'ok');
    } catch (e) {
      toast(errText(e), 'error');
    }
  }

  async function startGame(g, mode) {
    try {
      await invoke('start_game', { key: g.key, mode });
    } catch (e) {
      toast(errText(e), 'error');
    }
  }

  async function startStep(g, stage, src) {
    try {
      await invoke('start_step', { key: g.key, stage, src: src || null });
    } catch (e) {
      toast(errText(e), 'error');
    }
  }

  async function startDev(g) {
    try {
      await invoke('start_dev', { key: g.key });
    } catch (e) {
      toast(errText(e), 'error');
    }
  }

  async function stopRun() {
    if (!S.key) return;
    try {
      await invoke('stop', { key: S.key });
    } catch (e) {
      toast(errText(e), 'error');
    }
  }

  async function runNext() {
    const g = game();
    if (!g || !g.next) return;
    const n = g.next;
    if (n.kind === 'play') return startGame(g, 'play');
    if (n.kind === 'files') {
      if (g.package) return startStep(g, 'files', g.package);
      const path = await invoke('pick_package');
      if (path) startStep(g, 'files', path);
      return;
    }
    if (n.kind === 'generate' || n.kind === 'compile') return startStep(g, n.kind);
    if (n.kind === 'build') return startDev(g);
    if (n.kind === 'add-base') return addBase();
    if (n.kind === 'create') return createSheet(g);
  }

  async function setupFrom(kind) {
    const g = game();
    if (!g) return;
    if (kind === 'package') {
      const path = await invoke('pick_package');
      if (path) startStep(g, 'files', path);
    } else if (kind === 'folder') {
      const path = await invoke('pick_folder', { title: "The game files' folder" });
      if (path) startStep(g, 'files', path);
    } else if (kind === 'all') {
      startStep(g, 'all');
    }
  }

  async function chooseApp() {
    const g = game();
    if (!g) return;
    const path = await invoke('pick_app');
    if (!path) return;
    const done = toast('Checking the app…', 'busy');
    try {
      S.snap = await invoke('choose_app', { index: g.index, path });
      render();
      toast('App added', 'ok');
    } catch (e) {
      toast(errText(e), 'error');
    } finally {
      done();
    }
  }

  async function saveDev() {
    const g = game();
    if (!g) return;
    const dir = (document.getElementById(`dev-dir-${g.key}`) || {}).value || '';
    const cmd = (document.getElementById(`dev-cmd-${g.key}`) || {}).value || '';
    try {
      S.snap = await invoke('save_dev', { index: g.index, dir, cmd });
      render();
      toast('Dev build saved', 'ok');
    } catch (e) {
      toast(errText(e), 'error');
    }
  }

  async function submitApp() {
    const path = sheet.querySelector('#app-path').value.trim();
    const dataRoot = sheet.querySelector('#app-data').value.trim();
    const err = sheet.querySelector('#sheet-err');
    err.textContent = '';
    if (!path) {
      err.textContent = 'Choose the app.';
      return;
    }
    try {
      S.snap = await invoke('add_app', { path, dataRoot: dataRoot || null });
      closeSheet();
      render();
      toast('App added', 'ok');
    } catch (e) {
      err.textContent = errText(e);
    }
  }

  async function submitCreate() {
    const g = game();
    if (!g) return;
    const id = sheet.querySelector('#c-id').value.trim();
    const dir = sheet.querySelector('#c-dir').value.trim();
    const err = sheet.querySelector('#sheet-err');
    const btn = sheet.querySelector('[data-act="submit-create"]');
    err.textContent = '';
    btn.disabled = true;
    btn.innerHTML = '<span class="spinner"></span> Preparing…';
    try {
      await invoke('create_recomp', { index: g.index, id, dir });
      closeSheet();
      toast('Building the recomp’s app…', 'busy');
      queueLoad();
    } catch (e) {
      err.textContent = errText(e);
      btn.disabled = false;
      btn.textContent = 'Create';
    }
  }

  function closeMenus() {
    document.querySelectorAll('.menu-pop').forEach((p) => (p.hidden = true));
  }

  function collectControls(form) {
    const controls = {};
    for (const el of form.querySelectorAll('select[data-c]')) {
      if (!el.value) continue;
      const name = el.dataset.c;
      if (name.startsWith('bind_pad.')) (controls.bind_pad = controls.bind_pad || {})[name.slice(9)] = el.value;
      else if (name === 'prompts') controls.prompts = el.value;
      else if (name === 'rumble_strength') controls.rumble_strength = Number(el.value);
      else controls[name] = el.value === '1';
    }
    return controls;
  }

  async function saveControls(form) {
    if (!form) return;
    try {
      S.snap = await invoke('save_controls', {
        scope: form.dataset.scope,
        key: form.dataset.key || null,
        controls: collectControls(form),
      });
      if (form.dataset.scope === 'defaults') updatePadMap();
    } catch (e) {
      toast(errText(e), 'error');
      load(true);
    }
  }

  async function openDonate() {
    try {
      await invoke('open_donate');
    } catch (e) {
      toast(errText(e), 'error');
    }
  }

  async function openIssue(g) {
    if (!g) return;
    try {
      await invoke('open_issue', { key: g.key });
      toast('The nxrt issue form is open in your browser', 'ok');
    } catch (e) {
      toast(errText(e), 'error');
    }
  }

  async function resetControls(scope, key) {
    try {
      S.snap = await invoke('save_controls', { scope, key: key || null, controls: {} });
      render();
      toast(scope === 'defaults' ? 'Controller defaults cleared' : 'This game follows the defaults again');
    } catch (e) {
      toast(errText(e), 'error');
    }
  }

  async function doAct(act, el) {
    const g = game();
    closeMenus();
    switch (act) {
      case 'menu': {
        const pop = el.nextElementSibling;
        const open = pop.hidden;
        closeMenus();
        pop.hidden = !open;
        return;
      }
      case 'refresh':
        return load(true);
      case 'add-game':
        return addGame();
      case 'add-app':
        return addAppSheet();
      case 'open':
        return go('game', el.dataset.key);
      case 'next':
        return runNext();
      case 'play':
        return g && startGame(g, 'play');
      case 'start-screen':
        return g && startGame(g, 'start');
      case 'stop':
        return stopRun();
      case 'back':
        return go('library');
      case 'setup-package':
        return setupFrom('package');
      case 'setup-folder':
        return setupFrom('folder');
      case 'rebuild':
        return setupFrom('all');
      case 'add-base':
        return addBase();
      case 'create':
        return g && createSheet(g);
      case 'choose-app':
        return chooseApp();
      case 'remove':
        return removeGame();
      case 'reveal':
        return invoke('reveal', { path: el.dataset.path, file: el.dataset.file === '1' });
      case 'show-log':
        return invoke('reveal', { path: el.dataset.path, file: true });
      case 'open-config':
        return invoke('reveal', { path: S.snap.config_dir, file: false });
      case 'open-logs-dir':
        return invoke('open_logs');
      case 'manager-refresh':
        return loadManager(false);
      case 'manager-level':
        S.managerLevel = el.dataset.level;
        return render();
      case 'go-logs':
        return go('logs');
      case 'open-update':
        return openUpdate();
      case 'check-update':
        return checkForUpdate(true);
      case 'donate':
        return openDonate();
      case 'pad-test':
        return startPadTest();
      case 'pad-test-stop':
        return stopPadTest();
      case 'pad-help':
        return helpSheet();
      case 'issue':
        return openIssue(g);
      case 'controls-reset-defaults':
        return resetControls('defaults');
      case 'controls-reset-game':
        return resetControls('game', el.dataset.key);
      case 'keys-change':
        return changeKeys();
      case 'keys-forget':
        return forgetKeys();
      case 'save-dev':
        return saveDev();
      case 'browse-dev-dir': {
        const p = await invoke('pick_folder', { title: "The game's source checkout" });
        if (p) {
          const input = document.getElementById(`dev-dir-${g.key}`);
          if (input) input.value = p;
        }
        return;
      }
      case 'browse-app': {
        const p = await invoke('pick_app');
        if (p) sheet.querySelector('#app-path').value = p;
        return;
      }
      case 'browse-data': {
        const p = await invoke('pick_folder', { title: 'Data folder' });
        if (p) sheet.querySelector('#app-data').value = p;
        return;
      }
      case 'browse-create-dir': {
        const p = await invoke('pick_folder', { title: 'Where the recomp goes (a folder for it is made there)' });
        if (p) {
          sheet.querySelector('#c-dir').value = p;
          createDirTouched = true;
        }
        return;
      }
      case 'submit-app':
        return submitApp();
      case 'submit-create':
        return submitCreate();
    }
  }


  // ---- controllers & focus --------------------------------------------------------------------------------------

  const FOCUS_SEL = 'button, [tabindex="0"], summary, input, select, a[href]';
  const cssq = (v) => (window.CSS && CSS.escape ? CSS.escape(String(v)) : String(v).replace(/["\\]/g, '\\$&'));

  function selOf(el) {
    if (!el || el === document.body) return null;
    const d = el.dataset || {};
    if (d.card) return `[data-card="${cssq(d.card)}"]`;
    if (d.nav) return `[data-nav="${cssq(d.nav)}"]`;
    if (el.id) return `#${cssq(el.id)}`;
    if (el.tagName === 'SUMMARY' && el.parentElement && el.parentElement.dataset.disc) return `[data-disc="${cssq(el.parentElement.dataset.disc)}"] > summary`;
    if (d.act) return `[data-act="${cssq(d.act)}"]${d.key ? `[data-key="${cssq(d.key)}"]` : ''}`;
    return null;
  }

  function visible(el) {
    if (el.disabled) return false;
    const r = el.getBoundingClientRect();
    if (r.width < 2 || r.height < 2) return false;
    const st = getComputedStyle(el);
    return st.visibility !== 'hidden' && st.display !== 'none' && st.opacity !== '0';
  }

  function focusables() {
    const root = sheet.open ? sheet : document;
    return Array.from(root.querySelectorAll(FOCUS_SEL)).filter(visible);
  }

  function revealInMain(el) {
    if (!main.contains(el)) return;
    const r = el.getBoundingClientRect();
    const m = main.getBoundingClientRect();
    const pad = 16;
    if (r.top < m.top + pad) main.scrollTop = Math.max(0, main.scrollTop - (m.top + pad - r.top));
    else if (r.bottom > m.bottom - pad) main.scrollTop += r.bottom - (m.bottom - pad);
  }

  function setFocus(el) {
    if (!el) return;
    try {
      el.focus({ preventScroll: true });
    } catch {
      el.focus();
    }
    document.querySelectorAll('.pad-focus').forEach((x) => x.classList.remove('pad-focus'));
    el.classList.add('pad-focus');
    revealInMain(el);
  }

  function focusFirst() {
    const list = focusables();
    const inMain = list.filter((el) => main.contains(el));
    const destructive = (el) => el.dataset.act === 'remove' || el.dataset.act === 'keys-forget' || el.classList.contains('danger');
    const pick =
      inMain.find((el) => el.hasAttribute('data-card')) ||
      inMain.find((el) => el.classList.contains('primary')) ||
      inMain.find((el) => el.dataset.act === 'next' || el.dataset.act === 'play' || el.dataset.act === 'stop') ||
      inMain.find((el) => !destructive(el)) ||
      inMain[0] ||
      list[0];
    if (pick) setFocus(pick);
    return pick;
  }

  function padWake() {
    if (!S.padMode) {
      S.padMode = true;
      document.body.classList.add('pad-mode');
    }
    const a = document.activeElement;
    if (!a || a === document.body || !document.contains(a) || !focusables().includes(a)) focusFirst();
  }

  function moveFocus(dir) {
    padWake();
    const list = focusables();
    const cur = document.activeElement;
    if (!list.includes(cur)) {
      focusFirst();
      return;
    }
    // (a card is one thing on the grid: right goes to the next card, not to the Play pill inside this one)
    const skip = cur.hasAttribute && cur.hasAttribute('data-card') ? (el) => !cur.contains(el) : () => true;
    const cr = cur.getBoundingClientRect();
    const cx = cr.left + cr.width / 2;
    const cy = cr.top + cr.height / 2;
    const horiz = dir === 'left' || dir === 'right';
    const sign = dir === 'right' || dir === 'down' ? 1 : -1;
    let best = null;
    let bestScore = Infinity;
    for (const el of list) {
      if (el === cur || !skip(el)) continue;
      const r = el.getBoundingClientRect();
      const ex = r.left + r.width / 2;
      const ey = r.top + r.height / 2;
      const delta = (horiz ? ex - cx : ey - cy) * sign;
      if (delta < 4) continue;
      const overlap = horiz
        ? Math.min(cr.bottom, r.bottom) - Math.max(cr.top, r.top)
        : Math.min(cr.right, r.right) - Math.max(cr.left, r.left);
      const cross = overlap > 0 ? 0 : Math.abs(horiz ? ey - cy : ex - cx);
      const score = delta + cross * 2.4 + (overlap > 0 ? 0 : 30);
      if (score < bestScore) {
        bestScore = score;
        best = el;
      }
    }
    if (best) setFocus(best);
  }

  function adjustSelect(el, delta) {
    if (!el || el.tagName !== 'SELECT' || !el.options.length) return false;
    el.selectedIndex = (el.selectedIndex + delta + el.options.length) % el.options.length;
    el.dispatchEvent(new Event('change', { bubbles: true }));
    return true;
  }

  function activateFocused() {
    padWake();
    const el = document.activeElement;
    if (!el || el === document.body) {
      focusFirst();
      return;
    }
    if (el.tagName === 'INPUT') return;
    if (el.tagName === 'SELECT') {
      adjustSelect(el, 1);  // (a pad cannot steer a native popup)
      return;
    }
    el.click();
  }

  function back() {
    if (sheet.open) {
      closeSheet();
      return;
    }
    if (document.querySelector('.menu-pop:not([hidden])')) {
      closeMenus();
      return;
    }
    if (S.view !== 'library') {
      const key = S.key;
      S.view = 'library';
      S.key = null;
      S.wantFocusFirst = false;
      S.pendingFocus = key ? `[data-card="${cssq(key)}"]` : '[data-nav="library"]';
      render();
    }
  }

  function cycleSection(dir) {
    if (S.view === 'game') {
      go(dir > 0 ? 'logs' : 'library');
      return;
    }
    const order = ['library', 'logs', 'settings'];
    const here = S.view;
    let i = order.indexOf(here);
    i = (i + (dir > 0 ? 1 : -1) + order.length) % order.length;
    go(order[i]);
  }

  async function playFocused() {
    padWake();
    const el = document.activeElement;
    const card = el && el.closest ? el.closest('[data-card]') : null;
    const g = card ? S.snap && S.snap.games.find((x) => x.key === card.dataset.card) : game();
    if (!g) return;
    if (g.running) {
      try {
        await invoke('stop', { key: g.key });
      } catch (e) {
        toast(errText(e), 'error');
      }
      return;
    }
    if (!g.ready) {
      toast(g.exe ? 'Not ready to play yet: finish Setup first' : 'No nxrt app for this game yet', 'error');
      return;
    }
    if (S.view !== 'game') go('game', g.key);
    startGame(g, 'play');
  }

  function infoFocused() {
    padWake();
    const el = document.activeElement;
    const card = el && el.closest ? el.closest('[data-card]') : null;
    if (card) {
      go('game', card.dataset.card);
      return;
    }
    if (S.view === 'game') {
      const g = game();
      if (!g) return;
      const d = document.querySelector(`[data-disc="details-${cssq(g.key)}"]`);
      if (!d) return;
      d.open = !d.open;
      if (d.open) S.disc.add(d.dataset.disc);
      else S.disc.delete(d.dataset.disc);
      const sum = d.querySelector('summary');
      if (sum) setFocus(sum);
    }
  }

  function padLabels(name) {
    const n = String(name || '');
    if (/pro controller|joy-?con|switch|nintendo/i.test(n)) return { south: 'B', east: 'A', north: 'X', west: 'Y' };
    if (/dualsense|dualshock|playstation|ps[45]|sony/i.test(n)) return { south: '✕', east: '○', north: '△', west: '□' };
    return { south: 'A', east: 'B', north: 'Y', west: 'X' };
  }

  const kbd = (k) => `<kbd>${esc(k)}</kbd>`;
  const hint = (k, label) => `<span class="hint">${kbd(k)}${esc(label)}</span>`;

  function renderPad() {
    const el = document.getElementById('pad');
    document.body.classList.toggle('pad-on', !!S.pad.connected);
    if (!el) return;
    if (!S.pad.connected) {
      el.innerHTML = '';
      return;
    }
    el.innerHTML = `<div class="pad-chip" title="${esc(S.pad.name || 'Controller')}">${ICONS.pad}<span>${esc(S.pad.name || 'Controller')}</span></div>`;
  }

  function renderHints() {
    const el = document.getElementById('pad-hints');
    if (!el) return;
    if (!S.pad.connected || sheet.open) {
      el.hidden = true;
      return;
    }
    if (S.padTest) {
      el.innerHTML = `<span class="pad-mini">${ICONS.pad}</span><span class="pad-name">${esc(S.pad.name || 'Controller')}</span><span class="pad-sep"></span><span class="hint">Press any button</span>${hint('Esc', 'Stop')}`;
      el.hidden = false;
      return;
    }
    const L = padLabels(S.pad.name);
    let parts = [];
    if (S.view === 'game') {
      const g = game();
      parts = [hint(L.south, 'Select'), hint(L.east, 'Back'), hint(L.west, g && g.running ? 'Stop' : 'Play'), hint(L.north, 'Details'), hint('LB / RB', 'Section'), hint('Select', 'Help')];
    } else if (S.view === 'library') {
      parts = [hint(L.south, 'Open'), hint(L.west, 'Play'), hint(L.north, 'Info'), hint('LB / RB', 'Section'), hint('Select', 'Help')];
    } else {
      parts = [hint(L.south, 'Select'), hint('LB / RB', 'Section'), hint('Select', 'Help')];
    }
    el.innerHTML = `<span class="pad-mini">${ICONS.pad}</span><span class="pad-name">${esc(S.pad.name || 'Controller')}</span><span class="pad-sep"></span>${parts.join('')}`;
    el.hidden = false;
  }

  function padSheetHints() {
    if (!S.pad.connected) return '';
    const L = padLabels(S.pad.name);
    return `<div class="sheet-pad">${hint(L.south, 'Select')}${hint(L.east, 'Cancel')}</div>`;
  }

  function padHelpGrid() {
    const L = padLabels(S.pad.name);
    return `<div class="pad-help-grid">
      <span>${kbd(`${L.south}`)} Select</span><span>${kbd(`${L.east}`)} Back</span><span>${kbd(`${L.west}`)} Play / Stop</span>
      <span>${kbd(`${L.north}`)} Details</span><span>${kbd('LB / RB')} Section</span><span>${kbd('D-pad')} Move</span>
    </div>`;
  }

  // ---- pad input -------------------------------------------------------------------------------------------------

  const heldDirs = new Set();
  const scrollTimers = {};
  let repeatDelay = 0;
  let repeatEvery = 0;
  let repeatDir = null;

  function stopRepeat() {
    clearTimeout(repeatDelay);
    clearInterval(repeatEvery);
    repeatDelay = 0;
    repeatEvery = 0;
    repeatDir = null;
  }

  function startRepeat(dir) {
    stopRepeat();
    repeatDir = dir;
    repeatDelay = setTimeout(() => {
      repeatEvery = setInterval(() => moveFocus(dir), 105);
    }, 380);
  }

  function padButton(name, pressed) {
    // (the manager only listens while its window is the one in front: a pad in a running game is the game's)
    if (!document.hasFocus()) return;
    if (S.padTest) return testPadPress(name, pressed);
    padWake();
    if (name === 'up' || name === 'down' || name === 'left' || name === 'right') {
      if (pressed) {
        if (heldDirs.has(name)) return;
        heldDirs.add(name);
        moveFocus(name);
        startRepeat(name);
      } else {
        heldDirs.delete(name);
        if (repeatDir === name) stopRepeat();
      }
      return;
    }
    if (!pressed) return;
    // (a sheet is modal: only moving, selecting and going back while it is open)
    if (sheet.open && name !== 'south' && name !== 'east') return;
    switch (name) {
      case 'south':
        return activateFocused();
      case 'east':
        return back();
      case 'west':
        return playFocused();
      case 'north':
        return infoFocused();
      case 'lb':
        return cycleSection(-1);
      case 'rb':
        return cycleSection(1);
      case 'start':
        return playFocused();
      case 'select':
        return helpSheet();
    }
  }

  function padDir(dir) {
    if (!document.hasFocus()) return;
    if (S.padTest) return;
    padWake();
    if (dir) {
      const sel = document.activeElement;
      if (sel && sel.tagName === 'SELECT' && (dir === 'left' || dir === 'right')) {
        adjustSelect(sel, dir === 'right' ? 1 : -1);
        return;
      }
      if (repeatDir === dir) return;
      moveFocus(dir);
      startRepeat(dir);
    } else if (!heldDirs.size) {
      stopRepeat();
    }
  }

  function padTrigger(side, pressed) {
    if (!document.hasFocus()) return;
    if (S.padTest) return testPadPress(side === 'left' ? 'lt' : 'rt', pressed);
    padWake();
    if (pressed) {
      if (scrollTimers[side]) return;
      const step = side === 'left' ? -340 : 340;
      main.scrollBy({ top: step, behavior: 'smooth' });
      scrollTimers[side] = setInterval(() => main.scrollBy({ top: step, behavior: 'smooth' }), 280);
    } else if (scrollTimers[side]) {
      clearInterval(scrollTimers[side]);
      delete scrollTimers[side];
    }
  }

  function helpSheet() {
    padWake();
    const L = padLabels(S.pad.name);
    const rows = [
      [L.south, 'Select'],
      [L.east, 'Back'],
      [L.west, 'Play / Stop'],
      [L.north, 'Details'],
      ['LB / RB', 'Library, Logs, Settings'],
      ['LT / RT', 'Scroll the page'],
      ['D-pad / left stick', 'Move between things'],
      ['Start', 'Play the game'],
      ['Select', 'This help'],
    ];
    openSheet(`<div class="sheet-head"><h2 id="sheet-title">Controls</h2><button class="btn ghost icon" data-close aria-label="Close">${ICONS.x}</button></div>
      <div class="sheet-body">
        <p class="hint">Any controller works over USB or Bluetooth, and the labels follow the one connected. The same moves work from the keyboard: arrows to move, Enter to select, Esc to go back.</p>
        <div class="help-grid">${rows.map(([k, v]) => `<div class="help-k">${kbd(k)}</div><div>${esc(v)}</div>`).join('')}</div>
      </div>
      <div class="sheet-foot">${padSheetHints()}<button class="btn primary" data-close>Close</button></div>`);
  }

  // ---- events ---------------------------------------------------------------------------------------------------

  function bind() {
    document.addEventListener('click', (e) => {
      if (!e.target.closest('.menu')) closeMenus();
      const navBtn = e.target.closest('[data-nav]');
      if (navBtn) {
        go(navBtn.dataset.nav);
        return;
      }
      if (e.target.closest('[data-close]')) {
        closeSheet();
        return;
      }
      const actEl = e.target.closest('[data-act]');
      if (actEl) {
        e.preventDefault();
        doAct(actEl.dataset.act, actEl);
        return;
      }
      const card = e.target.closest('[data-card]');
      if (card) go('game', card.dataset.card);
    });

    document.addEventListener('change', (e) => {
      if (e.target.id === 'manager-follow') {
        S.managerFollow = e.target.checked;
        scheduleManager();
        return;
      }
      const sel = e.target.closest('select[data-c]');
      if (sel) saveControls(sel.closest('.controls-form'));
    });

    document.addEventListener('input', (e) => {
      if (e.target.id === 'manager-search') {
        S.managerSearch = e.target.value;
        const tail = document.getElementById('manager-tail');
        if (tail) tail.innerHTML = managerTailHtml();
      }
    });

    const reportUi = (level, message, source, stack) =>
      invoke('log_ui', { level, message, source, stack }).catch(() => {});
    window.addEventListener('error', (e) => {
      reportUi('error', e.message || 'Script error', e.filename ? `${e.filename}:${e.lineno}:${e.colno}` : '', e.error && e.error.stack ? e.error.stack : '');
    });
    window.addEventListener('unhandledrejection', (e) => {
      const reason = e.reason;
      reportUi('error', `Unhandled rejection: ${reason && reason.message ? reason.message : String(reason)}`, 'promise', reason && reason.stack ? reason.stack : '');
    });

    document.addEventListener('keydown', (e) => {
      if ((e.key === 'Enter' || e.key === ' ') && document.activeElement && document.activeElement.dataset.card) {
        e.preventDefault();
        go('game', document.activeElement.dataset.card);
      }
    });

    sheet.addEventListener('click', (e) => {
      if (e.target === sheet) closeSheet();
    });

    listen('run-state', (event) => {
      const st = event.payload;
      S.runs[st.key] = st;
      const sg = S.snap && S.snap.games.find((x) => x.key === st.key);
      if (sg) sg.running = st;
      if (S.view === 'game' && S.key === st.key) updateRun(st.key);
      else if (sg) updateCard(sg);
    }).catch((e) => console.error('run-state', e));

    listen('run-finished', (event) => {
      const { key, text, ok } = event.payload;
      delete S.runs[key];
      toast(text, ok ? 'ok' : 'error');
      loadLogsSoon();
      queueLoad();
    }).catch((e) => console.error('run-finished', e));

    listen('library-changed', () => queueLoad()).catch((e) => console.error('library-changed', e));

    listen('pad-status', (event) => {
      const next = event.payload || { connected: false, name: null, error: null };
      const was = S.pad && S.pad.connected;
      S.pad = next;
      if (!next.connected && S.padTest) {
        S.padTest = false;
        S.padHeld.clear();
        S.padLast = null;
      }
      if (!next.connected) {
        heldDirs.clear();
        stopRepeat();
        Object.keys(scrollTimers).forEach((k) => {
          clearInterval(scrollTimers[k]);
          delete scrollTimers[k];
        });
      }
      if (next.connected && !was) toast(`Controller connected: ${next.name || 'controller'}`, 'ok');
      if (!next.connected && was) toast('Controller disconnected');
      renderPad();
      renderHints();
      if (S.view === 'settings') render();
    }).catch((e) => console.error('pad-status', e));
    listen('pad-button', (event) => padButton(event.payload.button, event.payload.pressed)).catch((e) => console.error('pad-button', e));
    listen('pad-dir', (event) => padDir(event.payload.dir)).catch((e) => console.error('pad-dir', e));
    listen('pad-trigger', (event) => padTrigger(event.payload.side, event.payload.pressed)).catch((e) => console.error('pad-trigger', e));

    document.addEventListener('pointerdown', () => {
      if (!S.padMode) return;
      S.padMode = false;
      document.body.classList.remove('pad-mode');
      document.querySelectorAll('.pad-focus').forEach((el) => el.classList.remove('pad-focus'));
    }, true);

    window.addEventListener('blur', () => {
      heldDirs.clear();
      stopRepeat();
      Object.keys(scrollTimers).forEach((k) => {
        clearInterval(scrollTimers[k]);
        delete scrollTimers[k];
      });
    });

    document.addEventListener('keydown', (e) => {
      if (e.defaultPrevented) return;
      if (S.padTest) {
        if (e.key === 'Escape') {
          e.preventDefault();
          stopPadTest();
        }
        return;
      }
      if (sheet.open) {
        if (e.key === 'Escape') closeSheet();
        return;
      }
      if (e.target.matches('input, textarea, select')) return;  // (let a native select take its arrows and Enter)
      const dirs = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right' };
      if (dirs[e.key]) {
        e.preventDefault();
        padWake();
        moveFocus(dirs[e.key]);
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        back();
        return;
      }
      if (e.key === '[') {
        e.preventDefault();
        cycleSection(-1);
        return;
      }
      if (e.key === ']') {
        e.preventDefault();
        cycleSection(1);
        return;
      }
      if (e.key === 'Enter' && document.activeElement && document.activeElement.dataset.card) {
        e.preventDefault();
        document.activeElement.click();
      }
    });

    setTimeout(() => checkForUpdate(false), 1200);
    invoke('pad_status')
      .then((s) => {
        S.pad = s || { connected: false, name: null, error: null };
        renderPad();
        renderHints();
        if (S.view === 'settings') render();
      })
      .catch(() => {});

    try {
      const { getCurrentWebview } = window.__TAURI__.webview;
      getCurrentWebview().onDragDropEvent((event) => {
        const p = event.payload;
        const dz = document.getElementById('dropzone');
        if (p.type === 'over') dz && dz.classList.add('hot');
        if (p.type === 'leave') dz && dz.classList.remove('hot');
        if (p.type === 'drop') {
          dz && dz.classList.remove('hot');
          const path = (p.paths || []).find((x) => /\.(nsp|xci)$/i.test(x));
          if (path) addPackagePath(path);
          else toast('A Switch game is an .nsp or .xci', 'error');
        }
      });
    } catch (e) {
      console.warn('drag and drop unavailable', e);
    }
  }

  let logsTimer = 0;
  function loadLogsSoon() {
    clearTimeout(logsTimer);
    logsTimer = setTimeout(() => {
      S.logs = null;
      if (S.view === 'logs') loadLogs();
    }, 500);
  }

  function updateRun(key) {
    const g = game();
    if (!g) return;
    g.running = S.runs[key];
    const pillEl = document.querySelector('.js-hero-pill');
    if (pillEl) pillEl.innerHTML = pill(g.running.playing ? 'Playing' : 'Working…', 'info');
    const prog = document.querySelector('.js-progress');
    if (prog) prog.outerHTML = progressHtml(g) || '<div class="js-progress"></div>';
    const log = document.querySelector('.js-log');
    if (log) {
      log.innerHTML = logHtml(g);
      hydrateLog();
    }
  }

  function updateCard(g) {
    const card = Array.from(document.querySelectorAll('[data-card]')).find((c) => c.dataset.card === g.key);
    if (!card) return;
    const state = card.querySelector('.state');
    if (!state) return;
    const txt = g.running ? (g.running.playing ? 'Playing' : 'Working…') : g.state;
    const tone = g.running ? 'info' : g.tone;
    state.className = `state ${toneClass(tone)}`;
    state.innerHTML = `<i class="dot ${toneClass(tone)}"></i><span class="txt" title="${esc(txt)}">${esc(txt)}</span>`;
  }

  // ---- the section a screenshot asks for ------------------------------------------------------------------------

  let pendingView = null;

  function applyPendingView() {
    if (!pendingView || !S.snap) return;
    const v = pendingView;
    pendingView = null;
    if (v === 'logs') return go('logs');
    if (v === 'settings') return go('settings');
    if (v.startsWith('game:')) {
      const g = S.snap.games[Number(v.slice(5))];
      if (g) go('game', g.key);
      return;
    }
    // (a demo of the controller furniture for screenshots; NXRT_FRONTEND_VIEW=pad:...)
    if (v.startsWith('pad:')) {
      const rest = v.slice(4);
      S.pad = { connected: true, name: 'Xbox Wireless Controller', error: null };
      renderPad();
      renderHints();
      if (rest === 'help') {
        padWake();
        helpSheet();
        return;
      }
      if (rest === 'settings') {
        S.padHeld = new Set(['south', 'ltrigger', 'dpad_up']);
        S.padLast = 'south';
        go('settings');
        setTimeout(() => padWake(), 120);
        return;
      }
      if (rest.startsWith('game:')) {
        const g = S.snap.games[Number(rest.slice(5))];
        if (g) go('game', g.key);
      } else {
        go('library');
      }
      setTimeout(() => padWake(), 120);
      return;
    }
    if (v === 'sheet:add') return addAppSheet();
    if (v.startsWith('sheet:create:')) {
      const g = S.snap.games[Number(v.slice(13))];
      if (g) createSheet(g);
    }
  }

  window.__nxrtView = (v) => {
    pendingView = v;
    applyPendingView();
  };

  // ---- go -------------------------------------------------------------------------------------------------------

  function init() {
    render();
    bind();
    load();
  }

  init();
})();
