# Variable Players, Per-AI Config, and Draw Pile Tracking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Support 2-6 players with auto-scaling deck presets, per-AI opponent configuration in play mode, and draw pile remaining tracking.

**Architecture:** Backend already supports variable player counts in game loop. Main work is: add `draw_pile_remaining` to RoundResult and SimulationSummary, update DeckConfig defaults, add deck presets to frontend, restructure play panel from fixed compass to dynamic layout with per-AI config.

**Tech Stack:** Rust (Tauri backend), vanilla JS frontend, HTML/CSS

**Spec:** `docs/superpowers/specs/2026-03-24-variable-players-and-draw-tracking-design.md`

---

## File Structure

### Files to Modify
- `src-tauri/src/engine/config.rs` — Update DeckConfig::default() to 4-player preset
- `src-tauri/src/engine/game.rs` — Add `draw_pile_remaining` to RoundResult, set at round end
- `src-tauri/src/simulation/stats.rs` — Add `avg_draw_pile_remaining` to SimulationSummary, compute in aggregate()
- `src-tauri/src/interactive/state.rs` — Update player_name() to numbered names
- `src/index.html` — Add 5/6 player options, restructure play board, per-AI config section
- `src/js/config-panel.js` — Deck presets per player count, auto-apply on count change
- `src/js/play-panel.js` — Per-AI config, player count selector, dynamic grid rendering
- `src/js/app.js` — Display avg_draw_pile_remaining in stats
- `src/styles/main.css` — Replace compass layout CSS with dynamic flex layout

---

## Task 1: Add draw_pile_remaining to RoundResult and SimulationSummary

**Files:**
- Modify: `src-tauri/src/engine/game.rs`
- Modify: `src-tauri/src/simulation/stats.rs`

- [ ] **Step 1: Add field to RoundResult**

In `src-tauri/src/engine/game.rs`, add to the `RoundResult` struct:
```rust
pub draw_pile_remaining: u32,
```

In `play_round`, set it when building the RoundResult return value:
```rust
draw_pile_remaining: state.draw_pile.len() as u32,
```

- [ ] **Step 2: Add field to SimulationSummary and compute it**

In `src-tauri/src/simulation/stats.rs`, add to `SimulationSummary`:
```rust
pub avg_draw_pile_remaining: f64,
```

In the `aggregate` function, compute it from round results. Find where `avg_turns_per_round` is computed (around line 100-120) and add similar logic:
```rust
let total_draw_remaining: f64 = results.iter()
    .flat_map(|g| g.round_results.iter())
    .map(|r| r.draw_pile_remaining as f64)
    .sum();
let total_rounds = results.iter().map(|g| g.round_results.len()).sum::<usize>() as f64;
let avg_draw_pile_remaining = if total_rounds > 0.0 { total_draw_remaining / total_rounds } else { 0.0 };
```

Set it in the SimulationSummary construction.

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test --lib -- --nocapture 2>&1 | tail -5
```
Expected: All tests pass. The new field has a default value in test contexts.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/engine/game.rs src-tauri/src/simulation/stats.rs
git commit -m "feat: add draw_pile_remaining to RoundResult and SimulationSummary"
```

---

## Task 2: Update DeckConfig::default() and add deck preset data

**Files:**
- Modify: `src-tauri/src/engine/config.rs`

- [ ] **Step 1: Update DeckConfig::default() to match 4-player preset**

Replace the current default deck quantities with the 4-player preset (132 total):
```rust
impl Default for DeckConfig {
    fn default() -> Self {
        // 4-player preset: 132 cards (120 number + 12 wild)
        let card_quantities = vec![
            (-5, 5), (-4, 6), (-3, 8), (-2, 9), (-1, 11),
            (0, 13),
            (1, 11), (2, 11), (3, 10), (4, 9), (5, 8), (6, 7), (7, 6), (8, 6),
        ];
        DeckConfig {
            neg_min: -5,
            pos_max: 8,
            card_quantities,
            wild_count: 12,
        }
    }
}
```

- [ ] **Step 2: Add a deck_preset_for_players function**

Add a public function that returns a DeckConfig for a given player count. This will be called from the frontend via a Tauri command (or the frontend can hardcode the presets in JS — simpler). Since the presets are just data, put them in JS (Task 5). No Rust function needed.

Skip this step — presets will live in `config-panel.js`.

- [ ] **Step 3: Run config tests**

```bash
cd src-tauri && cargo test --lib engine::config -- --nocapture
```
Expected: All config tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/engine/config.rs
git commit -m "feat: update DeckConfig default to 4-player preset (132 cards)"
```

---

## Task 3: Update player_name() and add 5/6 player HTML options

**Files:**
- Modify: `src-tauri/src/interactive/state.rs`
- Modify: `src/index.html`

- [ ] **Step 1: Update player_name() in state.rs**

Find the `player_name` method (around line 711). Replace the compass-based naming:
```rust
fn player_name(&self, idx: usize) -> String {
    if idx == self.human_player {
        "You".to_string()
    } else {
        format!("Player {} (AI)", idx + 1)
    }
}
```

- [ ] **Step 2: Add 5/6 player options to config panel in index.html**

Find the player-count select (line 69-73). Add options:
```html
<select id="player-count">
  <option value="2">2 Players</option>
  <option value="3">3 Players</option>
  <option value="4" selected>4 Players</option>
  <option value="5">5 Players</option>
  <option value="6">6 Players</option>
</select>
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/interactive/state.rs src/index.html
git commit -m "feat: numbered player names and 2-6 player support in config"
```

---

## Task 4: Restructure play panel HTML for variable players

**Files:**
- Modify: `src/index.html`

This is the biggest HTML change — replace the fixed compass layout with a dynamic container.

- [ ] **Step 1: Replace play-setup section**

Find the `play-setup` div (lines 162-180). Replace with player count selector and per-AI config:

```html
<div id="play-setup" class="play-setup">
  <div class="config-section">
    <h2>Play a Game</h2>
    <p class="play-description">Play against AI opponents. You are always Player 1.</p>
    <div class="config-row">
      <div class="config-group">
        <label>Players</label>
        <select id="play-player-count" onchange="buildPlayAiPanels()">
          <option value="2">2 Players</option>
          <option value="3">3 Players</option>
          <option value="4" selected>4 Players</option>
          <option value="5">5 Players</option>
          <option value="6">6 Players</option>
        </select>
      </div>
      <div class="config-group">
        <label>Quick Fill</label>
        <div style="display:flex;gap:0.3rem">
          <button class="btn-secondary btn-sm" onclick="quickFillAi('beginner')">All Beginner</button>
          <button class="btn-secondary btn-sm" onclick="quickFillAi('expert')">All Expert</button>
          <button class="btn-secondary btn-sm" onclick="quickFillAi('mixed')">Mixed</button>
        </div>
      </div>
    </div>
    <div id="play-ai-panels"></div>
    <div style="margin-top:1rem">
      <button class="btn-primary" onclick="startPlayGame()">Start Game</button>
    </div>
  </div>
</div>
```

- [ ] **Step 2: Replace play-table with dynamic grid container**

Replace the fixed compass layout (lines 198-235) with a dynamic container:

```html
<!-- Game board with dynamic layout -->
<div class="play-table">
  <!-- Human player (always bottom) -->
  <div class="play-position play-human">
    <div class="play-player-label" id="label-player-0">You</div>
    <div class="play-grid" id="grid-player-0"></div>
  </div>
  <!-- AI players rendered dynamically above -->
  <div class="play-ai-grids" id="play-ai-grids"></div>
  <!-- Center piles -->
  <div class="play-center">
    <div class="play-pile" id="draw-pile" onclick="handleDrawClick('draw')">
      <div class="pile-label">Draw</div>
      <div class="pile-card pile-draw" id="draw-pile-card">?</div>
      <div class="pile-count" id="draw-pile-count">0</div>
    </div>
    <div class="play-pile" id="discard-pile" onclick="handleDrawClick('discard')">
      <div class="pile-label">Discard</div>
      <div class="pile-card pile-discard" id="discard-pile-card">-</div>
    </div>
  </div>
</div>
```

- [ ] **Step 3: Commit**

```bash
git add src/index.html
git commit -m "feat: restructure play board for variable player counts"
```

---

## Task 4b: Update CSS for new play board layout

**Files:**
- Modify: `src/styles/main.css`

- [ ] **Step 1: Replace compass layout CSS with dynamic layout**

Find the compass-specific CSS rules (`.play-north`, `.play-south`, `.play-west`, `.play-east`, `.play-middle`). Remove them and add rules for the new structure:

```css
/* Dynamic play board layout */
.play-table {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
}

.play-ai-grids {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 1rem;
}

.play-position.play-ai {
  display: flex;
  flex-direction: column;
  align-items: center;
}

.play-position.play-human {
  display: flex;
  flex-direction: column;
  align-items: center;
  margin-top: 0.5rem;
}

.play-center {
  display: flex;
  gap: 2rem;
  justify-content: center;
  align-items: center;
  margin: 0.5rem 0;
}
```

Keep the existing `.play-grid`, `.play-pile`, `.pile-card`, and card styles — those are layout-agnostic and still apply.

- [ ] **Step 2: Commit**

```bash
git add src/styles/main.css
git commit -m "feat: update CSS for dynamic play board layout"
```

---

## Task 5: Add deck presets and auto-apply in config-panel.js

**Files:**
- Modify: `src/js/config-panel.js`

- [ ] **Step 1: Add deck preset data**

Add at the top of the file after existing preset constants:

```javascript
const DECK_PRESETS_BY_PLAYERS = {
  2: { quantities: {'-5':3,'-4':4,'-3':5,'-2':6,'-1':8,'0':10,'1':8,'2':8,'3':7,'4':6,'5':5,'6':4,'7':4,'8':4}, wildCount: 8 },
  3: { quantities: {'-5':4,'-4':5,'-3':6,'-2':8,'-1':9,'0':11,'1':9,'2':9,'3':9,'4':8,'5':7,'6':6,'7':5,'8':6}, wildCount: 10 },
  4: { quantities: {'-5':5,'-4':6,'-3':8,'-2':9,'-1':11,'0':13,'1':11,'2':11,'3':10,'4':9,'5':8,'6':7,'7':6,'8':6}, wildCount: 12 },
  5: { quantities: {'-5':6,'-4':7,'-3':9,'-2':11,'-1':13,'0':15,'1':13,'2':13,'3':12,'4':11,'5':9,'6':8,'7':7,'8':6}, wildCount: 14 },
  6: { quantities: {'-5':7,'-4':9,'-3':11,'-2':13,'-1':15,'0':17,'1':15,'2':15,'3':14,'4':12,'5':11,'6':9,'7':8,'8':8}, wildCount: 16 },
};
```

- [ ] **Step 2: Auto-apply preset on player count change**

Update the player-count change handler. Find the event listener at the bottom of the file and modify:

```javascript
document.getElementById('player-count').addEventListener('change', () => {
  const count = parseInt(document.getElementById('player-count').value);
  applyDeckPresetForPlayers(count);
  buildPlayerPanels();
});

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
```

- [ ] **Step 3: Commit**

```bash
git add src/js/config-panel.js
git commit -m "feat: auto-apply deck presets when player count changes"
```

---

## Task 6: Rewrite play-panel.js for variable players and per-AI config

**Files:**
- Modify: `src/js/play-panel.js`

This is the largest frontend task. The key changes: dynamic AI config panels, dynamic grid rendering, numbered player names.

- [ ] **Step 1: Replace AI_PRESETS and add panel builders**

Replace the top of the file:

```javascript
let playState = null;
let selectionMode = null;

const AI_PRESETS = {
  beginner:     { archetype: 'Opportunist', skill: 0.3 },
  intermediate: { archetype: 'Methodical',  skill: 0.6 },
  advanced:     { archetype: 'Opportunist', skill: 0.85 },
  expert:       { archetype: 'Calculator',  skill: 1.0 },
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
          <option value="Opportunist">Opportunist</option>
          <option value="Methodical">Methodical</option>
          <option value="Calculator">Calculator</option>
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
    beginner: { archetype: 'Opportunist', skill: 30 },
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
```

- [ ] **Step 2: Update startPlayGame() to read per-AI config**

```javascript
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
```

- [ ] **Step 3: Update renderPlayState() for dynamic grids**

Replace the hardcoded grid rendering:

```javascript
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
```

- [ ] **Step 4: Update renderScoreboard() for dynamic player count**

```javascript
function renderScoreboard() {
  const sb = document.getElementById('play-scoreboard');
  const round = playState.round + 1;
  const playerCount = playState.grids.length;

  sb.innerHTML = `
    <div class="scoreboard-round">Round ${round > 4 ? 4 : round} of 4 | Turn ${playState.turn} | Draw pile: ${playState.draw_pile_count}</div>
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
    </div>`;
}
```

- [ ] **Step 5: Remove renderPlayerLabels() and compass references**

Delete the `renderPlayerLabels()` function (it set compass names on fixed label elements). Remove any references to `label-north`, `label-south`, `label-west`, `label-east` — these IDs no longer exist. Also update `setMode` to use `grid-player-0` instead of `grid-south`:

```javascript
function setMode(mode) {
  selectionMode = mode;
  document.getElementById('btn-replace').className = mode === 'replace' ? 'btn-primary mode-active' : 'btn-secondary';
  document.getElementById('btn-flip').className = mode === 'flip' ? 'btn-primary mode-active' : 'btn-secondary';
  renderGrid('grid-player-0', playState.grids[0], 0);
}
```

- [ ] **Step 6: Add turn counter guard to handleAllAiTurns**

```javascript
async function handleAllAiTurns() {
  let guard = 0;
  while (playState.pending.action_type === 'not_your_turn') {
    playState = await tauriPlayAiTurn();
    guard++;
    if (guard > 100) {
      console.error('handleAllAiTurns: exceeded 100 iterations, breaking');
      break;
    }
  }
  renderPlayState();
}
```

- [ ] **Step 7: Initialize AI panels on page load**

Add at the bottom of the file or in an init function:
```javascript
// Build initial AI panels
document.addEventListener('DOMContentLoaded', () => {
  if (document.getElementById('play-ai-panels')) {
    buildPlayAiPanels();
  }
});
```

- [ ] **Step 8: Commit**

```bash
git add src/js/play-panel.js
git commit -m "feat: per-AI config and dynamic grid rendering for 2-6 players"
```

---

## Task 7: Display avg_draw_pile_remaining in simulation results

**Files:**
- Modify: `src/js/app.js`

- [ ] **Step 1: Add draw pile stat to results display**

Find the "Deck Health" section in the results HTML (around line 187-190). Add:
```javascript
<div class="stat-row"><span class="label">Avg draw pile remaining</span><span class="value">${summary.avg_draw_pile_remaining.toFixed(1)} cards</span></div>
```

Add it after the existing `draw_pile_exhaustion_rate` row.

- [ ] **Step 2: Commit**

```bash
git add src/js/app.js
git commit -m "feat: display avg draw pile remaining in simulation results"
```

---

## Task 8: Integration testing and validation

**Files:**
- Modify: `src-tauri/tests/smoke_test.rs`

- [ ] **Step 1: Add 2-player and 6-player game tests**

Add a 2-player test:
```rust
#[test]
fn test_two_player_with_preset_deck() {
    let mut config = GameConfig::default();
    config.player_count = 2;
    config.players = vec![PlayerConfig::advanced(), PlayerConfig::expert()];
    config.deck = DeckConfig {
        neg_min: -5,
        pos_max: 8,
        card_quantities: vec![
            (-5, 3), (-4, 4), (-3, 5), (-2, 6), (-1, 8),
            (0, 10),
            (1, 8), (2, 8), (3, 7), (4, 6), (5, 5), (6, 4), (7, 4), (8, 4),
        ],
        wild_count: 8,
    };
    assert!(config.deck.validate(config.player_count).is_ok());

    let mut rng = rand::thread_rng();
    for _ in 0..20 {
        let result = play_game(&config, &mut rng);
        assert_eq!(result.player_scores.len(), 2);
        assert!(result.winner < 2);
        assert_eq!(result.round_results.len(), 4);
    }
}
```

Add a 6-player test:

```rust
#[test]
fn test_six_player_games() {
    let mut config = GameConfig::default();
    config.player_count = 6;
    config.players = vec![
        PlayerConfig::beginner(),
        PlayerConfig::intermediate(),
        PlayerConfig::advanced(),
        PlayerConfig::expert(),
        PlayerConfig::beginner(),
        PlayerConfig::intermediate(),
    ];
    // Use a 6-player deck
    config.deck = DeckConfig {
        neg_min: -5,
        pos_max: 8,
        card_quantities: vec![
            (-5, 7), (-4, 9), (-3, 11), (-2, 13), (-1, 15),
            (0, 17),
            (1, 15), (2, 15), (3, 14), (4, 12), (5, 11), (6, 9), (7, 8), (8, 8),
        ],
        wild_count: 16,
    };
    assert!(config.deck.validate(config.player_count).is_ok());

    let mut rng = rand::thread_rng();
    for _ in 0..20 {
        let result = play_game(&config, &mut rng);
        assert_eq!(result.player_scores.len(), 6);
        assert!(result.winner < 6);
        assert_eq!(result.round_results.len(), 4);
        // Verify draw pile remaining is set
        for round in &result.round_results {
            assert!(round.draw_pile_remaining > 0 || round.draw_pile_exhausted,
                "Draw pile should have cards or be marked exhausted");
        }
    }
}
```

- [ ] **Step 2: Add draw pile no-exhaustion test**

```rust
#[test]
fn test_preset_decks_no_exhaustion() {
    // Test that 4-player default deck doesn't exhaust over 100 games
    let config = GameConfig::default();
    let mut rng = rand::thread_rng();
    let mut exhausted_count = 0u32;

    for _ in 0..100 {
        let result = play_game(&config, &mut rng);
        for round in &result.round_results {
            if round.draw_pile_exhausted {
                exhausted_count += 1;
            }
        }
    }

    println!("Draw pile exhausted in {} of 400 rounds", exhausted_count);
    // With the larger 132-card deck, exhaustion should be rare
    assert!(exhausted_count < 20, "Deck exhausted too often: {} of 400 rounds", exhausted_count);
}
```

- [ ] **Step 3: Run all tests**

```bash
cd src-tauri && cargo test -- --nocapture 2>&1 | tail -10
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tests/smoke_test.rs
git commit -m "test: add 6-player and draw pile exhaustion tests"
```
