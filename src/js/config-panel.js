// ── Default deck quantities ───────────────────────────────────────────────

const DEFAULT_QUANTITIES = {
  '-5': 4, '-4': 6, '-3': 7, '-2': 8, '-1': 9,
  '0': 10,
  '1': 9, '2': 9, '3': 9, '4': 8, '5': 7, '6': 6, '7': 5, '8': 4,
};

const ORIGINAL_QUANTITIES = {
  '-5': 2, '-4': 4, '-3': 6, '-2': 8, '-1': 15,
  '0': 20,
  '1': 15, '2': 15, '3': 15, '4': 15, '5': 15,
  '6': 15, '7': 15, '8': 15, '9': 15, '10': 15,
};

const PLAYER_PRESETS = {
  Beginner:     { keepThreshold: 2, lineAwareness: 10, opponentAwareness: 0,  flipStrategy: 'Random' },
  Intermediate: { keepThreshold: 3, lineAwareness: 40, opponentAwareness: 20, flipStrategy: 'Random' },
  Advanced:     { keepThreshold: 4, lineAwareness: 70, opponentAwareness: 50, flipStrategy: 'Random' },
  Expert:       { keepThreshold: 5, lineAwareness: 95, opponentAwareness: 80, flipStrategy: 'Random' },
};

// ── Card Quantity Table ───────────────────────────────────────────────────

function buildCardQuantityTable() {
  const negMin = parseInt(document.getElementById('neg-min').value) || -5;
  const posMax = parseInt(document.getElementById('pos-max').value) || 10;
  const container = document.getElementById('card-quantity-table');

  let html = '<div class="quantity-table">';
  for (let v = negMin; v <= posMax; v++) {
    const defaultQty = DEFAULT_QUANTITIES[String(v)] || (v === 0 ? 10 : v < 0 ? 5 : 6);
    const cls = v < 0 ? 'negative' : v === 0 ? 'zero' : 'positive';
    html += `
      <div class="quantity-cell">
        <span class="card-value ${cls}">${v}</span>
        <input type="number" class="card-qty" data-value="${v}"
               value="${defaultQty}" min="0" max="50" oninput="updateTotalCards()" />
      </div>`;
  }
  html += '</div>';
  container.innerHTML = html;
  updateTotalCards();
}

function updateTotalCards() {
  let total = 0;
  document.querySelectorAll('.card-qty').forEach(input => {
    total += parseInt(input.value) || 0;
  });
  total += parseInt(document.getElementById('wild-count').value) || 0;

  document.getElementById('total-cards').textContent = total.toLocaleString();
}

function applyDeckPreset(preset) {
  if (preset === 'default') {
    document.getElementById('neg-min').value = -5;
    document.getElementById('pos-max').value = 8;
    document.getElementById('wild-count').value = 8;
    buildCardQuantityTable();
    document.querySelectorAll('.card-qty').forEach(input => {
      const v = input.dataset.value;
      input.value = DEFAULT_QUANTITIES[v] || 0;
    });
  } else if (preset === 'original') {
    document.getElementById('neg-min').value = -5;
    document.getElementById('pos-max').value = 10;
    document.getElementById('wild-count').value = 15;
    buildCardQuantityTable();
    document.querySelectorAll('.card-qty').forEach(input => {
      const v = input.dataset.value;
      input.value = ORIGINAL_QUANTITIES[v] || 0;
    });
  }
  // 'custom' does nothing — user edits manually
  updateTotalCards();
}

// ── Player Panels ─────────────────────────────────────────────────────────

function buildPlayerPanels() {
  const count = parseInt(document.getElementById('player-count').value);
  const container = document.getElementById('player-panels');

  let html = '<div class="player-panels-grid">';
  for (let i = 0; i < count; i++) {
    html += buildPlayerPanel(i);
  }
  html += '</div>';
  container.innerHTML = html;
}

function buildPlayerPanel(idx) {
  const p = PLAYER_PRESETS.Advanced;
  return `
    <div class="player-panel" id="player-panel-${idx}">
      <h3>
        Player ${idx + 1}
        <select class="preset-select" onchange="applyPlayerPreset(${idx}, this.value)">
          <option value="">Preset...</option>
          <option value="Beginner">Beginner</option>
          <option value="Intermediate">Intermediate</option>
          <option value="Advanced" selected>Advanced</option>
          <option value="Expert">Expert</option>
        </select>
      </h3>
      <div class="config-group" style="margin-bottom:0.6rem">
        <label>Keep Threshold</label>
        <div style="display:flex;align-items:center;gap:0.5rem">
          <input type="number" id="keep-thresh-${idx}" min="0" max="10" value="${p.keepThreshold}" style="width:60px" />
          <span style="font-size:0.75rem;color:var(--text-dim)">Keep drawn cards with |value| &le; this</span>
        </div>
      </div>
      <div class="slider-group">
        <label>Line Awareness <span class="slider-value" id="line-aware-val-${idx}">${p.lineAwareness}%</span></label>
        <input type="range" id="line-aware-${idx}" min="0" max="100" value="${p.lineAwareness}"
               oninput="document.getElementById('line-aware-val-${idx}').textContent = this.value + '%'" />
        <div style="display:flex;justify-content:space-between;font-size:0.7rem;color:var(--text-dim)">
          <span>Ignores lines</span><span>Plans eliminations</span>
        </div>
      </div>
      <div class="slider-group">
        <label>Opponent Awareness <span class="slider-value" id="opp-aware-val-${idx}">${p.opponentAwareness}%</span></label>
        <input type="range" id="opp-aware-${idx}" min="0" max="100" value="${p.opponentAwareness}"
               oninput="document.getElementById('opp-aware-val-${idx}').textContent = this.value + '%'" />
        <div style="display:flex;justify-content:space-between;font-size:0.7rem;color:var(--text-dim)">
          <span>Ignores others</span><span>Watches opponents</span>
        </div>
      </div>
      <div class="config-group" style="margin-top:0.6rem">
        <label>Initial Flip</label>
        <select id="flip-strategy-${idx}">
          <option value="Random" ${p.flipStrategy === 'Random' ? 'selected' : ''}>Random</option>
          <option value="SameColumn" ${p.flipStrategy === 'SameColumn' ? 'selected' : ''}>Same Column</option>
          <option value="SameRow" ${p.flipStrategy === 'SameRow' ? 'selected' : ''}>Same Row</option>
          <option value="Corners" ${p.flipStrategy === 'Corners' ? 'selected' : ''}>Corners</option>
          <option value="Diagonal" ${p.flipStrategy === 'Diagonal' ? 'selected' : ''}>Diagonal</option>
        </select>
      </div>
    </div>`;
}

function applyPlayerPreset(idx, presetName) {
  const p = PLAYER_PRESETS[presetName];
  if (!p) return;

  document.getElementById(`keep-thresh-${idx}`).value = p.keepThreshold;
  document.getElementById(`line-aware-${idx}`).value = p.lineAwareness;
  document.getElementById(`line-aware-val-${idx}`).textContent = p.lineAwareness + '%';
  document.getElementById(`opp-aware-${idx}`).value = p.opponentAwareness;
  document.getElementById(`opp-aware-val-${idx}`).textContent = p.opponentAwareness + '%';
  document.getElementById(`flip-strategy-${idx}`).value = p.flipStrategy;
}

function applyToAll() {
  const count = parseInt(document.getElementById('player-count').value);
  const src = 0;

  const keepThresh = document.getElementById(`keep-thresh-${src}`).value;
  const lineAware = document.getElementById(`line-aware-${src}`).value;
  const oppAware = document.getElementById(`opp-aware-${src}`).value;
  const flipStrategy = document.getElementById(`flip-strategy-${src}`).value;

  for (let i = 1; i < count; i++) {
    document.getElementById(`keep-thresh-${i}`).value = keepThresh;
    document.getElementById(`line-aware-${i}`).value = lineAware;
    document.getElementById(`line-aware-val-${i}`).textContent = lineAware + '%';
    document.getElementById(`opp-aware-${i}`).value = oppAware;
    document.getElementById(`opp-aware-val-${i}`).textContent = oppAware + '%';
    document.getElementById(`flip-strategy-${i}`).value = flipStrategy;
  }
}

// ── Build Config Object ───────────────────────────────────────────────────

function buildConfigFromUI() {
  const playerCount = parseInt(document.getElementById('player-count').value);

  const cardQuantities = [];
  document.querySelectorAll('.card-qty').forEach(input => {
    const value = parseInt(input.dataset.value);
    const count = parseInt(input.value) || 0;
    if (count > 0) {
      cardQuantities.push([value, count]);
    }
  });

  const deck = {
    neg_min: parseInt(document.getElementById('neg-min').value),
    pos_max: parseInt(document.getElementById('pos-max').value),
    card_quantities: cardQuantities,
    wild_count: parseInt(document.getElementById('wild-count').value) || 0,
  };

  const players = [];
  for (let i = 0; i < playerCount; i++) {
    players.push({
      keep_threshold: parseInt(document.getElementById(`keep-thresh-${i}`).value) || 3,
      line_awareness: parseInt(document.getElementById(`line-aware-${i}`).value) / 100,
      opponent_awareness: parseInt(document.getElementById(`opp-aware-${i}`).value) / 100,
      flip_strategy: document.getElementById(`flip-strategy-${i}`).value,
    });
  }

  return {
    deck: deck,
    player_count: playerCount,
    allow_matching_elimination: document.getElementById('allow-matching').checked,
    allow_diagonal_elimination: document.getElementById('allow-diagonal').checked,
    scoring_mode: document.getElementById('scoring-mode').value,
    starting_order: document.getElementById('starting-order').value,
    players: players,
    max_turns_per_round: 500,
  };
}

// ── Event Listeners ───────────────────────────────────────────────────────

document.getElementById('player-count').addEventListener('change', buildPlayerPanels);
document.getElementById('neg-min').addEventListener('change', buildCardQuantityTable);
document.getElementById('pos-max').addEventListener('change', buildCardQuantityTable);
