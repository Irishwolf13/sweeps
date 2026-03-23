// ── Play Panel — Interactive Game ─────────────────────────────────────────

let playState = null;
let selectionMode = null;  // null, 'replace', 'flip'

const AI_PRESETS = {
  beginner:     { keep_threshold: 2, line_awareness: 0.1, opponent_awareness: 0.0 },
  intermediate: { keep_threshold: 3, line_awareness: 0.4, opponent_awareness: 0.2 },
  advanced:     { keep_threshold: 4, line_awareness: 0.7, opponent_awareness: 0.5 },
  expert:       { keep_threshold: 5, line_awareness: 0.95, opponent_awareness: 0.8 },
};

async function startPlayGame() {
  const preset = document.getElementById('play-ai-preset').value;
  const aiConfig = AI_PRESETS[preset];

  // Build a config using current deck settings from Configure tab
  const config = buildConfigFromUI();
  config.player_count = 4;

  // Human player gets perfect config (unused by engine, human makes own choices)
  config.players = [
    { keep_threshold: 5, line_awareness: 1.0, opponent_awareness: 0.5 },
    { ...aiConfig },
    { ...aiConfig },
    { ...aiConfig },
  ];

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
  renderGrid('grid-south', playState.grids[0], 0);
  renderGrid('grid-west', playState.grids[1], 1);
  renderGrid('grid-north', playState.grids[2], 2);
  renderGrid('grid-east', playState.grids[3], 3);
  renderPiles();
  renderPrompt();
  renderLog();
  renderPlayerLabels();
}

function renderScoreboard() {
  const sb = document.getElementById('play-scoreboard');
  const names = ['You (South)', 'West (AI)', 'North (AI)', 'East (AI)'];
  const round = playState.round + 1;

  sb.innerHTML = `
    <div class="scoreboard-round">Round ${round > 4 ? 4 : round} of 4 | Turn ${playState.turn}</div>
    <div class="scoreboard-players">
      ${names.map((name, i) => {
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
  if (!playState || playState.current_player !== 0) return false;
  const p = playState.pending.action_type;

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

function renderPlayerLabels() {
  const names = playState.player_names;
  document.getElementById('label-south').textContent = names[0];
  document.getElementById('label-west').textContent = names[1];
  document.getElementById('label-north').textContent = names[2];
  document.getElementById('label-east').textContent = names[3];

  // Highlight active player
  ['south', 'west', 'north', 'east'].forEach((dir, i) => {
    const label = document.getElementById(`label-${dir}`);
    label.classList.toggle('active-label', i === playState.current_player);
  });
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
  renderGrid('grid-south', playState.grids[0], 0);
}

async function handleCellClick(playerIdx, row, col) {
  if (!playState) return;
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
  // Play all AI turns until it's the human's turn again
  while (playState && playState.pending.action_type === 'not_your_turn') {
    try {
      playState = await tauriPlayAiTurn();
    } catch (e) {
      console.error('AI turn failed:', e);
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
