'use strict';

// ── IPC ───────────────────────────────────────────────────────────────────────
const invoke = (() => {
  const t = window.__TAURI__;
  return t?.core?.invoke ?? t?.tauri?.invoke ?? null;
})();

if (!invoke) console.error('[radioBOX] Tauri IPC unavailable — rebuild with withGlobalTauri:true');

function call(cmd, args) {
  return invoke(cmd, args);
}

// ── State ─────────────────────────────────────────────────────────────────────
let activeGenre    = '';
let filterOpen     = false;
let nowPlayingName = null;
let pollTimer      = null;
let stations       = [];   // current result set

// ── DOM ───────────────────────────────────────────────────────────────────────
const $list      = document.getElementById('station-list');
const $status    = document.getElementById('status-bar');
const $filterRow = document.getElementById('filter-row');
const $filterBtn = document.getElementById('filter-btn');
const $footer    = document.getElementById('now-playing');
const $npTitle   = document.getElementById('np-title');
const $npMeta    = document.getElementById('np-meta');
const $volSlider = document.getElementById('vol-slider');
const $volPct    = document.getElementById('vol-pct');
const $fName     = document.getElementById('f-name');
const $fGenre    = document.getElementById('f-genre');
const $fCountry  = document.getElementById('f-country');
const $fCodec    = document.getElementById('f-codec');

// ── Boot ──────────────────────────────────────────────────────────────────────
(async () => {
  const vol = await call('get_volume').catch(() => 60);
  $volSlider.value = vol;
  $volPct.textContent = vol + '%';
  await load({ top: true });
  startPoll();
})();

// ── Genre nav ─────────────────────────────────────────────────────────────────
const $nav = document.getElementById('genre-nav');
let dragX = 0, scrollX = 0, dragging = false;

function setActiveGenre(genre) {
  activeGenre = genre;
  $nav.querySelectorAll('.pill').forEach(p => {
    p.classList.toggle('active', p.dataset.genre === genre);
  });
}

$nav.addEventListener('mousedown', e => {
  dragging = true; dragX = e.pageX; scrollX = $nav.scrollLeft;
  $nav.style.cursor = 'grabbing';
});
window.addEventListener('mousemove', e => {
  if (!dragging) return;
  e.preventDefault();
  $nav.scrollLeft = scrollX - (e.pageX - dragX);
});
window.addEventListener('mouseup', () => {
  dragging = false;
  $nav.style.cursor = 'grab';
});

$nav.addEventListener('click', async e => {
  const pill = e.target.closest('.pill');
  if (!pill || Math.abs($nav.scrollLeft - scrollX) > 4) return;
  setActiveGenre(pill.dataset.genre);
  $fGenre.value = activeGenre;
  await load(activeGenre === '' ? { top: true } : { genre: activeGenre });
});

// ── Filter panel ──────────────────────────────────────────────────────────────
$filterBtn.addEventListener('click', () => {
  filterOpen = !filterOpen;
  $filterRow.classList.toggle('open', filterOpen);
  $filterBtn.setAttribute('aria-expanded', String(filterOpen));
  if (filterOpen) $fName.focus();
});

document.getElementById('search-btn').addEventListener('click', runSearch);
[$fName, $fGenre, $fCountry, $fCodec].forEach(el =>
  el.addEventListener('keydown', e => e.key === 'Enter' && runSearch())
);

async function runSearch() {
  const q = {
    name:        $fName.value.trim()             || null,
    genre:       $fGenre.value.trim()            || null,
    country:     $fCountry.value.trim().toUpperCase() || null,
    codec:       $fCodec.value.trim()            || null,
    min_bitrate: null,
  };
  if (!q.name && !q.genre && !q.country && !q.codec) { await load({ top: true }); return; }
  await load({ query: q });
}

$fGenre.addEventListener('input', () => {
  const genre = $fGenre.value.trim();
  const matchingPill = Array.from($nav.querySelectorAll('.pill'))
    .find(p => p.dataset.genre.toLowerCase() === genre.toLowerCase());

  if (matchingPill) {
    setActiveGenre(matchingPill.dataset.genre);
    return;
  }

  activeGenre = '';
  $nav.querySelectorAll('.pill').forEach(p => p.classList.remove('active'));
});

// ── Data loading ──────────────────────────────────────────────────────────────
async function load(opts) {
  setLoading();
  try {
    if (opts.top) {
      stations = await call('get_top_voted', { limit: 50 });
    } else if (opts.genre) {
      stations = await call('search_stations', {
        query: { name: null, genre: opts.genre, country: null, codec: null, min_bitrate: null }
      });
    } else {
      stations = await call('search_stations', { query: opts.query });
    }
    render();
  } catch (err) {
    renderError(err);
  }
}

// ── Render ────────────────────────────────────────────────────────────────────
function setLoading() {
  $status.textContent = '';
  $list.innerHTML = '<div class="state-center"><div class="spinner"></div></div>';
}

function renderError(err) {
  $status.textContent = '';
  $list.innerHTML = `<div class="state-center"><p class="state-error">⚠ ${esc(String(err))}</p></div>`;
}

function render() {
  if (!stations.length) {
    $status.textContent = '';
    $list.innerHTML = '<div class="state-center"><p class="state-dim">No stations found</p></div>';
    return;
  }

  $status.textContent = `${stations.length} station${stations.length !== 1 ? 's' : ''}`;

  $list.innerHTML = stations.map(s => {
    const playing = nowPlayingName === s.name;
    return `<div class="card${playing ? ' playing' : ''}" data-url="${esc(s.url)}" data-name="${esc(s.name)}">
      <div class="card-info">
        <div class="card-name">${esc(s.name)}</div>
        <div class="card-meta">${buildMeta(s)}</div>
      </div>
      <div class="card-action">
        ${playing
          ? '<span class="playing-label">▶ Playing</span>'
          : `<button class="play-btn" aria-label="Play ${esc(s.name)}">▶</button>`}
      </div>
    </div>`;
  }).join('');

  $list.addEventListener('click', onCardClick, { once: true });
  // re-attach after each render via delegation
  $list.onclick = onCardClick;
}

function onCardClick(e) {
  const btn = e.target.closest('.play-btn');
  if (!btn) return;
  const card = btn.closest('.card');
  playStation(card.dataset.url, card.dataset.name);
}

function buildMeta(s) {
  let html = '';
  // country + bitrate + codec as plain chips
  const chips = [];
  if (s.country) chips.push(esc(s.country));
  if (s.bitrate) chips.push(`${s.bitrate} kbps`);
  if (s.codec)   chips.push(esc(s.codec));
  if (chips.length) html += `<span class="meta-chips">${chips.join('<span class="dot">·</span>')}</span>`;

  // genre tags — split on comma, render individually
  if (s.tags) {
    const tags = s.tags.split(',').map(t => t.trim()).filter(Boolean).slice(0, 4);
    if (tags.length) html += tags.map(t => `<span class="tag">${esc(t)}</span>`).join('');
  }
  return html;
}

// ── Playback ──────────────────────────────────────────────────────────────────
async function playStation(url, name) {
  nowPlayingName = name;
  await call('play', { url }).catch(console.error);
  showFooter(name);
  render();
}

document.getElementById('stop-btn').addEventListener('click', async () => {
  await call('stop').catch(() => {});
  nowPlayingName = null;
  hideFooter();
  render();
});

// ── Volume ────────────────────────────────────────────────────────────────────
$volSlider.addEventListener('input', () => {
  const vol = +$volSlider.value;
  $volPct.textContent = vol + '%';
  call('set_volume', { vol }).catch(() => {});
});

// ── Footer ────────────────────────────────────────────────────────────────────
function showFooter(name) {
  $npTitle.textContent = name;
  $npMeta.textContent  = '';
  $footer.hidden = false;
}

function hideFooter() {
  $footer.hidden = true;
  $npTitle.textContent = '—';
  $npMeta.textContent  = '';
}

// ── Poll (500 ms) ─────────────────────────────────────────────────────────────
function startPoll() {
  pollTimer = setInterval(async () => {
    const playing = await call('is_playing').catch(() => false);
    if (!playing && nowPlayingName) { nowPlayingName = null; hideFooter(); render(); return; }
    if (!playing) return;
    const info = await call('get_stream_info').catch(() => null);
    if (info) updateFooter(info);
  }, 500);
}

function updateFooter(info) {
  if (info.media_title) $npTitle.textContent = info.media_title;
  const p = [];
  if (info.audio_bitrate > 0)  p.push(`${(info.audio_bitrate / 1000).toFixed(0)} kbps`);
  if (info.audio_codec)        p.push(info.audio_codec.toUpperCase());
  if (info.sample_rate > 0)    p.push(`${info.sample_rate} Hz`);
  if (info.channels > 0)       p.push(info.channels === 2 ? 'Stereo' : info.channels === 1 ? 'Mono' : `${info.channels}ch`);
  if (info.cache_duration > 0) p.push(`buf ${info.cache_duration.toFixed(1)}s`);
  if (info.uptime_secs > 0)    p.push(fmtTime(info.uptime_secs));
  $npMeta.textContent = p.join('  ·  ');
}

function fmtTime(s) {
  if (s >= 3600) return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
  if (s >= 60)   return `${Math.floor(s / 60)}m ${s % 60}s`;
  return `${s}s`;
}

// ── Util ──────────────────────────────────────────────────────────────────────
function esc(s) {
  return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
}
