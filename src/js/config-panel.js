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
  Beginner:     { archetype: 'Opportunist', skill: 30, flipStrategy: 'Random' },
  Intermediate: { archetype: 'Methodical',  skill: 60, flipStrategy: 'Random' },
  Advanced:     { archetype: 'Opportunist', skill: 85, flipStrategy: 'Random' },
  Expert:       { archetype: 'Calculator',  skill: 100, flipStrategy: 'Random' },
};

const DECK_PRESETS_BY_PLAYERS = {
  2: { quantities: {'-5':3,'-4':4,'-3':5,'-2':6,'-1':8,'0':10,'1':8,'2':8,'3':7,'4':6,'5':5,'6':4,'7':4,'8':4}, wildCount: 8 },
  3: { quantities: {'-5':4,'-4':5,'-3':6,'-2':8,'-1':9,'0':11,'1':9,'2':9,'3':9,'4':8,'5':7,'6':6,'7':5,'8':6}, wildCount: 10 },
  4: { quantities: {'-5':5,'-4':6,'-3':8,'-2':9,'-1':11,'0':13,'1':11,'2':11,'3':10,'4':9,'5':8,'6':7,'7':6,'8':6}, wildCount: 12 },
  5: { quantities: {'-5':6,'-4':7,'-3':9,'-2':11,'-1':13,'0':15,'1':13,'2':13,'3':12,'4':11,'5':9,'6':8,'7':7,'8':6}, wildCount: 14 },
  6: { quantities: {'-5':7,'-4':9,'-3':11,'-2':13,'-1':15,'0':17,'1':15,'2':15,'3':14,'4':12,'5':11,'6':9,'7':8,'8':8}, wildCount: 16 },
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
  if (preset === 'auto') {
    const count = parseInt(document.getElementById('player-count').value);
    applyDeckPresetForPlayers(count);
    return;
  } else if (preset === 'default') {
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
        <label>AI Archetype</label>
        <select id="archetype-${idx}">
          <option value="Opportunist" ${p.archetype === 'Opportunist' ? 'selected' : ''}>Opportunist</option>
          <option value="Methodical" ${p.archetype === 'Methodical' ? 'selected' : ''}>Methodical</option>
          <option value="Calculator" ${p.archetype === 'Calculator' ? 'selected' : ''}>Calculator</option>
        </select>
      </div>
      <div class="slider-group">
        <label>Skill <span class="slider-value" id="skill-val-${idx}">${p.skill}%</span></label>
        <input type="range" id="skill-${idx}" min="0" max="100" value="${p.skill}"
               oninput="document.getElementById('skill-val-${idx}').textContent = this.value + '%'" />
        <div style="display:flex;justify-content:space-between;font-size:0.7rem;color:var(--text-dim)">
          <span>Random play</span><span>Perfect execution</span>
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
  document.getElementById(`archetype-${idx}`).value = p.archetype;
  document.getElementById(`skill-${idx}`).value = p.skill;
  document.getElementById(`skill-val-${idx}`).textContent = p.skill + '%';
  document.getElementById(`flip-strategy-${idx}`).value = p.flipStrategy;
}

function applyToAll() {
  const count = parseInt(document.getElementById('player-count').value);
  const archetype = document.getElementById('archetype-0').value;
  const skill = document.getElementById('skill-0').value;
  const flipStrategy = document.getElementById('flip-strategy-0').value;
  for (let i = 1; i < count; i++) {
    document.getElementById(`archetype-${i}`).value = archetype;
    document.getElementById(`skill-${i}`).value = skill;
    document.getElementById(`skill-val-${i}`).textContent = skill + '%';
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
      archetype: document.getElementById(`archetype-${i}`).value,
      skill: parseInt(document.getElementById(`skill-${i}`).value) / 100,
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

function applyDeckPresetForPlayers(count) {
  const preset = DECK_PRESETS_BY_PLAYERS[count];
  if (!preset) return;

  document.getElementById('wild-count').value = preset.wildCount;
  buildCardQuantityTable();
  document.querySelectorAll('.card-qty').forEach(input => {
    const v = input.dataset.value;
    input.value = preset.quantities[v] || 0;
  });
  updateTotalCards();
}

// ── Event Listeners ───────────────────────────────────────────────────────

document.getElementById('player-count').addEventListener('change', () => {
  const count = parseInt(document.getElementById('player-count').value);
  document.getElementById('deck-preset').value = 'auto';
  applyDeckPresetForPlayers(count);
  buildPlayerPanels();
});
document.getElementById('neg-min').addEventListener('change', buildCardQuantityTable);
document.getElementById('pos-max').addEventListener('change', buildCardQuantityTable);
