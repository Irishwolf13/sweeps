// ── Shapes constants ─────────────────────────────────────────────────────

const SHAPES = ['Circle', 'Square', 'Triangle', 'Rectangle'];
const SHADES = ['Unshaded', 'Shaded'];

const SHAPES_DECK_PRESETS_BY_PLAYERS = {
  2: { perType: 8, wild: 4, wildShaded: 4, wildUnshaded: 4 },
  3: { perType: 11, wild: 5, wildShaded: 5, wildUnshaded: 5 },
  4: { perType: 14, wild: 6, wildShaded: 6, wildUnshaded: 6 },
  5: { perType: 17, wild: 8, wildShaded: 8, wildUnshaded: 8 },
  6: { perType: 20, wild: 9, wildShaded: 9, wildUnshaded: 9 },
};

const SHAPES_ORIGINAL = { perType: 25, wild: 10, wildShaded: 10, wildUnshaded: 10 };

const SHAPES_TIER_PRESETS = {
  Beginner:     { shadematters: false, matching: true, cancellation: false, diagonal: false, wilds: false },
  Intermediate: { shadematters: true,  matching: true, cancellation: false, diagonal: false, wilds: true },
  Advanced:     { shadematters: true,  matching: true, cancellation: true,  diagonal: false, wilds: true },
  Expert:       { shadematters: true,  matching: true, cancellation: true,  diagonal: true,  wilds: true },
};

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
  Beginner:     { archetype: 'Methodical',   skill: 60, flipStrategy: 'Random' },
  Intermediate: { archetype: 'Opportunist',  skill: 70, flipStrategy: 'Random' },
  Advanced:     { archetype: 'Opportunist',  skill: 85, flipStrategy: 'Random' },
  Expert:       { archetype: 'Calculator',   skill: 100, flipStrategy: 'Random' },
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
      <h3>Player ${idx + 1}</h3>
      <div class="config-group" style="margin-bottom:0.6rem">
        <label>AI Archetype</label>
        <select id="archetype-${idx}">
          <option value="Methodical" ${p.archetype === 'Methodical' ? 'selected' : ''}>Methodical (Beginner)</option>
          <option value="Opportunist" ${p.archetype === 'Opportunist' ? 'selected' : ''}>Opportunist (Intermediate)</option>
          <option value="Calculator" ${p.archetype === 'Calculator' ? 'selected' : ''}>Calculator (Expert)</option>
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
  const gameMode = document.getElementById('game-mode').value;

  let deck;
  if (gameMode === 'Shapes') {
    const shapeQuantities = [];
    document.querySelectorAll('.shape-qty').forEach(input => {
      const count = parseInt(input.value) || 0;
      if (count > 0) {
        shapeQuantities.push([input.dataset.shape, input.dataset.shade, count]);
      }
    });
    deck = {
      type: 'Shapes',
      shape_quantities: shapeQuantities,
      wild_count: parseInt(document.getElementById('shapes-wild-count').value) || 0,
      wild_shaded_count: parseInt(document.getElementById('shapes-wild-shaded-count').value) || 0,
      wild_unshaded_count: parseInt(document.getElementById('shapes-wild-unshaded-count').value) || 0,
    };
  } else {
    const cardQuantities = [];
    document.querySelectorAll('.card-qty').forEach(input => {
      const value = parseInt(input.dataset.value);
      const count = parseInt(input.value) || 0;
      if (count > 0) { cardQuantities.push([value, count]); }
    });
    deck = {
      type: 'Numbers',
      neg_min: parseInt(document.getElementById('neg-min').value),
      pos_max: parseInt(document.getElementById('pos-max').value),
      card_quantities: cardQuantities,
      wild_count: parseInt(document.getElementById('wild-count').value) || 0,
    };
  }

  const players = [];
  for (let i = 0; i < playerCount; i++) {
    players.push({
      archetype: document.getElementById(`archetype-${i}`).value,
      skill: parseInt(document.getElementById(`skill-${i}`).value) / 100,
      flip_strategy: document.getElementById(`flip-strategy-${i}`).value,
    });
  }

  return {
    game_mode: gameMode,
    deck: deck,
    player_count: playerCount,
    allow_matching_elimination: document.getElementById('allow-matching').checked,
    allow_diagonal_elimination: document.getElementById('allow-diagonal').checked,
    allow_cancellation: document.getElementById('allow-cancellation')?.checked || false,
    shade_matters: document.getElementById('shade-matters')?.checked || false,
    scoring_mode: gameMode === 'Shapes' ? 'Basic' : document.getElementById('scoring-mode').value,
    starting_order: document.getElementById('starting-order').value,
    players: players,
    max_turns_per_round: 500,
    round_multiplier: parseInt(document.getElementById('round-multiplier')?.value) || 1,
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

// ── Shapes Mode Functions ─────────────────────────────────────────────────

function onGameModeChange() {
  const mode = document.getElementById('game-mode').value;
  const numbersConfig = document.getElementById('numbers-config');
  const shapesConfig = document.getElementById('shapes-config');
  const shapesRules = document.getElementById('shapes-rules');
  const scoringGroup = document.getElementById('scoring-mode-group');

  if (mode === 'Shapes') {
    numbersConfig.style.display = 'none';
    shapesConfig.style.display = '';
    shapesRules.style.display = '';
    if (scoringGroup) scoringGroup.style.display = 'none';
    applyShapesTier();
    buildShapeQuantityTable();
  } else {
    numbersConfig.style.display = '';
    shapesConfig.style.display = 'none';
    shapesRules.style.display = 'none';
    if (scoringGroup) scoringGroup.style.display = '';
  }
}

function buildShapeQuantityTable() {
  const container = document.getElementById('shape-quantity-table');
  let html = '<div class="quantity-table">';
  for (const shape of SHAPES) {
    for (const shade of SHADES) {
      const id = `shape-qty-${shape}-${shade}`;
      const label = shade === 'Shaded' ? `■ ${shape}` : `○ ${shape}`;
      const cls = shade === 'Shaded' ? 'negative' : 'positive';
      html += `
        <div class="quantity-cell">
          <span class="card-value ${cls}">${label}</span>
          <input type="number" class="shape-qty" data-shape="${shape}" data-shade="${shade}"
                 id="${id}" value="14" min="0" max="50" oninput="updateShapesTotalCards()" />
        </div>`;
    }
  }
  html += '</div>';
  container.innerHTML = html;
  updateShapesTotalCards();
}

function updateShapesTotalCards() {
  let total = 0;
  document.querySelectorAll('.shape-qty').forEach(input => {
    total += parseInt(input.value) || 0;
  });
  total += parseInt(document.getElementById('shapes-wild-count').value) || 0;
  total += parseInt(document.getElementById('shapes-wild-shaded-count').value) || 0;
  total += parseInt(document.getElementById('shapes-wild-unshaded-count').value) || 0;
  const el = document.getElementById('shapes-total-cards');
  if (el) el.textContent = total.toLocaleString();
}

function applyShapesTier() {
  const tier = document.getElementById('shapes-tier').value;
  const preset = SHAPES_TIER_PRESETS[tier];
  if (!preset) return;
  document.getElementById('shade-matters').checked = preset.shadematters;
  document.getElementById('allow-matching').checked = preset.matching;
  document.getElementById('allow-cancellation').checked = preset.cancellation;
  document.getElementById('allow-diagonal').checked = preset.diagonal;
  const count = parseInt(document.getElementById('player-count').value);
  applyShapesDeckForPlayers(count, preset.wilds);
}

function applyShapesDeckForPlayers(playerCount, includeWilds) {
  const preset = SHAPES_DECK_PRESETS_BY_PLAYERS[playerCount];
  if (!preset) return;
  document.querySelectorAll('.shape-qty').forEach(input => {
    input.value = preset.perType;
  });
  document.getElementById('shapes-wild-count').value = includeWilds ? preset.wild : 0;
  document.getElementById('shapes-wild-shaded-count').value = includeWilds ? preset.wildShaded : 0;
  document.getElementById('shapes-wild-unshaded-count').value = includeWilds ? preset.wildUnshaded : 0;
  updateShapesTotalCards();
}

function applyShapesDeckPresetUI() {
  const preset = document.getElementById('shapes-deck-preset').value;
  if (preset === 'auto') {
    const count = parseInt(document.getElementById('player-count').value);
    const tier = document.getElementById('shapes-tier').value;
    const tierPreset = SHAPES_TIER_PRESETS[tier];
    applyShapesDeckForPlayers(count, tierPreset ? tierPreset.wilds : true);
  } else if (preset === 'original') {
    document.querySelectorAll('.shape-qty').forEach(input => {
      input.value = SHAPES_ORIGINAL.perType;
    });
    document.getElementById('shapes-wild-count').value = SHAPES_ORIGINAL.wild;
    document.getElementById('shapes-wild-shaded-count').value = SHAPES_ORIGINAL.wildShaded;
    document.getElementById('shapes-wild-unshaded-count').value = SHAPES_ORIGINAL.wildUnshaded;
    updateShapesTotalCards();
  }
  // 'custom' does nothing
}

// ── Event Listeners ───────────────────────────────────────────────────────

document.getElementById('player-count').addEventListener('change', () => {
  const count = parseInt(document.getElementById('player-count').value);
  const mode = document.getElementById('game-mode').value;
  if (mode === 'Shapes') {
    const deckPreset = document.getElementById('shapes-deck-preset').value;
    if (deckPreset === 'auto') {
      const tier = document.getElementById('shapes-tier').value;
      const tierPreset = SHAPES_TIER_PRESETS[tier];
      applyShapesDeckForPlayers(count, tierPreset ? tierPreset.wilds : true);
    }
  } else {
    document.getElementById('deck-preset').value = 'auto';
    applyDeckPresetForPlayers(count);
  }
  buildPlayerPanels();
});
document.getElementById('neg-min').addEventListener('change', buildCardQuantityTable);
document.getElementById('pos-max').addEventListener('change', buildCardQuantityTable);
