// ── Play Panel — Interactive Game ─────────────────────────────────────────

let playState = null;
let selectionMode = null;  // null, 'replace', 'flip'

const AI_PRESETS = {
  beginner:     { archetype: 'Methodical',   skill: 0.6 },
  intermediate: { archetype: 'Opportunist',  skill: 0.7 },
  advanced:     { archetype: 'Opportunist',  skill: 0.85 },
  expert:       { archetype: 'Calculator',   skill: 1.0 },
};

function buildPlayAiPanels() {
  const count = parseInt(document.getElementById('play-player-count').value);
  const container = document.getElementById('play-ai-panels');
  let html = '';
  for (let i = 1; i < count; i++) {
    html += `
      <div class="play-ai-config" style="display:flex;align-items:center;gap:0.8rem;margin:0.5rem 0;padding:0.5rem;background:var(--bg-secondary);border-radius:4px;">
        <strong style="min-width:5rem">Player ${i + 1}:</strong>
        <select id="play-arch-${i}">
          <option value="Opportunist">Opportunist (Intermediate)</option>
          <option value="Methodical">Methodical (Beginner)</option>
          <option value="Calculator">Calculator (Expert)</option>
        </select>
        <label style="font-size:0.8rem">Skill:</label>
        <input type="range" id="play-skill-${i}" min="0" max="100" value="85" style="width:100px"
               oninput="document.getElementById('play-skill-val-${i}').textContent=this.value+'%'" />
        <span id="play-skill-val-${i}" style="font-size:0.8rem;min-width:2.5rem">85%</span>
        <select id="play-flip-${i}" style="font-size:0.8rem">
          <option value="Random">Random</option>
          <option value="SameColumn">Same Col</option>
          <option value="SameRow">Same Row</option>
          <option value="Corners">Corners</option>
          <option value="Diagonal">Diagonal</option>
        </select>
      </div>`;
  }
  container.innerHTML = html;
}

function quickFillAi(preset) {
  const count = parseInt(document.getElementById('play-player-count').value);
  const presets = {
    beginner: { archetype: 'Methodical', skill: 60 },
    expert: { archetype: 'Calculator', skill: 100 },
  };
  const mixedOrder = ['beginner', 'intermediate', 'advanced', 'expert'];
  for (let i = 1; i < count; i++) {
    let p;
    if (preset === 'mixed') {
      const key = mixedOrder[(i - 1) % mixedOrder.length];
      p = AI_PRESETS[key];
      p = { archetype: p.archetype, skill: Math.round(p.skill * 100) };
    } else {
      p = presets[preset];
    }
    document.getElementById(`play-arch-${i}`).value = p.archetype;
    document.getElementById(`play-skill-${i}`).value = p.skill;
    document.getElementById(`play-skill-val-${i}`).textContent = p.skill + '%';
  }
}

async function startPlayGame() {
  const playerCount = parseInt(document.getElementById('play-player-count').value);
  const config = buildConfigFromUI();
  config.player_count = playerCount;

  // Human player
  const players = [{ archetype: 'Opportunist', skill: 1.0, flip_strategy: 'Random' }];
  // AI players from per-AI config
  for (let i = 1; i < playerCount; i++) {
    players.push({
      archetype: document.getElementById(`play-arch-${i}`).value,
      skill: parseInt(document.getElementById(`play-skill-${i}`).value) / 100,
      flip_strategy: document.getElementById(`play-flip-${i}`).value,
    });
  }
  config.players = players;

  try {
    playState = await tauriStartPlayGame(config);
    document.getElementById('play-setup').classList.add('hidden');
    document.getElementById('play-board').classList.remove('hidden');
    renderPlayState();
  } catch (e) {
    alert('Failed to start game: ' + e);
  }
}

function renderPlayState() {
  if (!playState) return;

  renderScoreboard();

  // Render human grid (always player 0)
  renderGrid('grid-player-0', playState.grids[0], 0);

  // Render AI grids dynamically
  const aiContainer = document.getElementById('play-ai-grids');
  const playerCount = playState.grids.length;
  let html = '';
  for (let i = 1; i < playerCount; i++) {
    html += `
      <div class="play-position play-ai">
        <div class="play-player-label" id="label-player-${i}">${playState.player_names[i]}</div>
        <div class="play-grid" id="grid-player-${i}"></div>
      </div>`;
  }
  aiContainer.innerHTML = html;
  for (let i = 1; i < playerCount; i++) {
    renderGrid(`grid-player-${i}`, playState.grids[i], i);
  }

  renderPiles();
  renderPrompt();
  renderLog();
}

function renderScoreboard() {
  const sb = document.getElementById('play-scoreboard');
  const round = playState.round + 1;

  sb.innerHTML = `
    <div class="scoreboard-round">Round ${round > playState.total_rounds ? playState.total_rounds : round} of ${playState.total_rounds} | Turn ${playState.turn} | Draw pile: ${playState.draw_pile_count}</div>
    <div class="scoreboard-players">
      ${playState.player_names.map((name, i) => {
        const active = i === playState.current_player ? ' active-player' : '';
        return `<div class="scoreboard-player${active}">
          <span class="sb-name">${name}</span>
          <span class="sb-score">${playState.cumulative_scores[i]} pts</span>
          <span class="sb-cards">${playState.grids[i].remaining} cards</span>
          <span class="sb-elims">${playState.grids[i].eliminations} elims</span>
        </div>`;
      }).join('')}
    </div>
  `;
}

function renderGrid(containerId, gridView, playerIdx) {
  const container = document.getElementById(containerId);
  const isHuman = playerIdx === 0;

  let html = '<table class="grid-table">';
  for (let r = 0; r < gridView.cells.length; r++) {
    html += '<tr>';
    for (let c = 0; c < gridView.cells[r].length; c++) {
      const cell = gridView.cells[r][c];
      const clickable = isHuman && canClickCell(r, c, cell);
      const onClick = clickable ? ` onclick="handleCellClick(${playerIdx},${r},${c})"` : '';
      const cursorClass = clickable ? ' clickable' : '';

      if (cell.state === 'empty') {
        html += `<td class="grid-cell cell-empty${cursorClass}"${onClick}></td>`;
      } else if (cell.state === 'face_down') {
        html += `<td class="grid-cell cell-facedown${cursorClass}"${onClick}>?</td>`;
      } else {
        const typeClass = cell.card_type ? ` cell-${cell.card_type}` : '';
        html += `<td class="grid-cell cell-faceup${typeClass}${cursorClass}"${onClick}>${cell.card}</td>`;
      }
    }
    html += '</tr>';
  }
  html += '</table>';
  container.innerHTML = html;
}

function canClickCell(r, c, cell) {
  if (!playState) return false;
  const p = playState.pending.action_type;

  // Initial flips happen before turn order — always allow human to pick
  if (p === 'choose_initial_flips') return cell.state === 'face_down';

  if (playState.current_player !== 0) return false;
  if (p === 'handle_normal_card') {
    if (selectionMode === 'replace') return cell.state !== 'empty';
    if (selectionMode === 'flip') return cell.state === 'face_down';
  }
  return false;
}

function renderPiles() {
  const drawCount = document.getElementById('draw-pile-count');
  const discardCard = document.getElementById('discard-pile-card');
  const drawPile = document.getElementById('draw-pile');
  const discardPile = document.getElementById('discard-pile');

  drawCount.textContent = playState.draw_pile_count;

  if (playState.discard_top) {
    discardCard.textContent = playState.discard_top.display;
    discardCard.className = `pile-card pile-discard card-type-${playState.discard_top.card_type}`;
  } else {
    discardCard.textContent = '-';
    discardCard.className = 'pile-card pile-discard';
  }

  // Highlight piles when choosing draw source
  const isDrawPhase = playState.pending.action_type === 'choose_draw_source' && playState.current_player === 0;
  drawPile.classList.toggle('pile-clickable', isDrawPhase);
  discardPile.classList.toggle('pile-clickable', isDrawPhase);
}

function renderPrompt() {
  const prompt = document.getElementById('play-prompt');
  const buttons = document.getElementById('play-buttons');
  const p = playState.pending;

  selectionMode = null;
  buttons.innerHTML = '';

  if (p.action_type === 'choose_initial_flips') {
    const remaining = playState.pending.flips_remaining || 0;
    prompt.innerHTML = `<p>Click ${remaining} card${remaining > 1 ? 's' : ''} to flip face-up</p>`;
    return;
  }

  if (p.action_type === 'game_over') {
    const winnerName = playState.player_names[playState.winner];
    prompt.textContent = `Game Over! ${winnerName} wins!`;
    buttons.innerHTML = '<button class="btn-primary" onclick="resetPlayGame()">New Game</button>';
    return;
  }

  if (p.action_type === 'round_over') {
    prompt.textContent = 'Round over! Click to continue.';
    buttons.innerHTML = '<button class="btn-primary" onclick="handleNextRound()">Next Round</button>';
    return;
  }

  if (p.action_type === 'not_your_turn') {
    const currentName = playState.player_names[playState.current_player];
    prompt.textContent = `${currentName}'s turn...`;
    buttons.innerHTML = '<button class="btn-primary" onclick="handleAiTurn()">Play AI Turn</button>' +
                        '<button class="btn-secondary" onclick="handleAllAiTurns()">Play All AI Turns</button>';
    return;
  }

  if (p.action_type === 'choose_draw_source') {
    prompt.textContent = 'Your turn! Click the Draw Pile or Discard Pile to draw a card.';
    return;
  }

  if (p.action_type === 'handle_normal_card') {
    const card = p.drawn_card;
    prompt.innerHTML = `You drew <strong class="drawn-card card-type-${card.card_type}">${card.display}</strong>. Choose an action:`;
    selectionMode = 'replace'; // default
    buttons.innerHTML = `
      <button class="btn-primary mode-active" id="btn-replace" onclick="setMode('replace')">Replace a Card</button>
      <button class="btn-secondary" id="btn-flip" onclick="setMode('flip')">Discard & Flip</button>
    `;
    return;
  }

  if (p.action_type === 'choose_slide_direction') {
    prompt.textContent = 'Diagonal elimination! Choose how to slide the remaining cards:';
    buttons.innerHTML = `
      <button class="btn-primary" onclick="handleSlide('horizontal')">Slide Horizontal</button>
      <button class="btn-primary" onclick="handleSlide('vertical')">Slide Vertical</button>
    `;
    return;
  }
}

function renderLog() {
  const log = document.getElementById('play-log');
  // Show last 30 entries, newest at bottom
  const entries = playState.action_log.slice(-30);
  log.innerHTML = entries.map(entry => `<div class="log-entry">${entry}</div>`).join('');
  log.scrollTop = log.scrollHeight;
}

// ── Click handlers ───────────────────────────────────────────────────────

async function handleDrawClick(source) {
  if (!playState || playState.pending.action_type !== 'choose_draw_source') return;
  if (playState.current_player !== 0) return;

  try {
    playState = await tauriPlayDraw(source);
    renderPlayState();
  } catch (e) {
    console.error('Draw failed:', e);
  }
}

function setMode(mode) {
  selectionMode = mode;
  document.getElementById('btn-replace').className = mode === 'replace' ? 'btn-primary mode-active' : 'btn-secondary';
  document.getElementById('btn-flip').className = mode === 'flip' ? 'btn-primary mode-active' : 'btn-secondary';
  // Re-render grid to update clickable states
  renderGrid('grid-player-0', playState.grids[0], 0);
}

async function handleCellClick(playerIdx, row, col) {
  if (!playState) return;

  if (playState.pending.action_type === 'choose_initial_flips') {
    try {
      playState = await tauriPlayFlipInitial(row, col);
      renderPlayState();
    } catch (e) {
      alert(e);
    }
    return;
  }

  const p = playState.pending.action_type;

  // Normal card: replace or flip
  if (p === 'handle_normal_card' && playerIdx === 0) {
    try {
      if (selectionMode === 'replace') {
        playState = await tauriPlayAction('replace', { row, col });
      } else if (selectionMode === 'flip') {
        playState = await tauriPlayAction('flip', { row, col });
      }
      renderPlayState();
    } catch (e) {
      console.error('Action failed:', e);
    }
  }
}

async function handleSlide(direction) {
  try {
    playState = await tauriPlaySlide(direction);
    renderPlayState();
  } catch (e) {
    console.error('Slide failed:', e);
  }
}

async function handleAiTurn() {
  try {
    playState = await tauriPlayAiTurn();
    renderPlayState();
  } catch (e) {
    console.error('AI turn failed:', e);
  }
}

async function handleAllAiTurns() {
  let guard = 0;
  while (playState && playState.pending.action_type === 'not_your_turn') {
    try {
      playState = await tauriPlayAiTurn();
    } catch (e) {
      console.error('AI turn failed:', e);
      break;
    }
    guard++;
    if (guard > 100) {
      console.error('handleAllAiTurns: exceeded 100 iterations, breaking');
      break;
    }
  }
  renderPlayState();
}

async function handleNextRound() {
  try {
    playState = await tauriPlayNextRound();
    renderPlayState();
  } catch (e) {
    console.error('Next round failed:', e);
  }
}

function resetPlayGame() {
  playState = null;
  document.getElementById('play-setup').classList.remove('hidden');
  document.getElementById('play-board').classList.add('hidden');
}

function confirmQuitGame() {
  if (confirm('Quit the current game and return to setup?')) {
    resetPlayGame();
  }
}

// Build initial AI panels on page load
document.addEventListener('DOMContentLoaded', () => {
  if (document.getElementById('play-ai-panels')) {
    buildPlayAiPanels();
  }
});
