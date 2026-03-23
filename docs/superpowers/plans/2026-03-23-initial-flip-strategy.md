# Initial Flip Strategy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add configurable AI flip strategies for initial 2 face-up cards, and let the human player manually choose their starting flips in interactive mode.

**Architecture:** New `FlipStrategy` enum on `PlayerConfig` drives how AI players pick their 2 starting cards. `PlayerGrid::new()` accepts a strategy instead of a count. Interactive mode adds a `ChooseInitialFlips` phase where the human clicks 2 cards before play begins. AI players in interactive mode use their configured strategy.

**Tech Stack:** Rust (Tauri 2.x backend), HTML/JS/CSS frontend (no bundler).

**Spec:** `docs/superpowers/specs/2026-03-23-initial-flip-strategy-design.md`

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src-tauri/src/engine/config.rs` | Modify | Add `FlipStrategy` enum, field on `PlayerConfig` |
| `src-tauri/src/engine/grid.rs` | Modify | Update `new()` to accept `FlipStrategy`, add `new_no_flips()`, add `flip_positions()` |
| `src-tauri/src/engine/game.rs` | Modify | Pass per-player `FlipStrategy` to grid creation |
| `src-tauri/src/interactive/state.rs` | Modify | Add `ChooseInitialFlips` phase, `human_flip_initial()`, update `start_round()`, update `get_state()`, update `PendingAction` |
| `src-tauri/src/commands.rs` | Modify | Add `play_flip_initial` command |
| `src-tauri/src/main.rs` | Modify | Register new command |
| `src/js/config-panel.js` | Modify | Add flip strategy dropdown per player, update presets and `buildConfigFromUI()` |
| `src/js/play-panel.js` | Modify | Handle `choose_initial_flips` phase, update `canClickCell()` |
| `src/js/tauri-bridge.js` | Modify | Add `tauriPlayFlipInitial` bridge function |

---

## Task 1: Add `FlipStrategy` enum and config field

**Files:**
- Modify: `src-tauri/src/engine/config.rs`

- [ ] **Step 1: Add `FlipStrategy` enum after `StartingOrder`**

In `src-tauri/src/engine/config.rs`, after the `StartingOrder` `Default` impl (after line 75), add:

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum FlipStrategy {
    Random,
    SameColumn,
    SameRow,
    Corners,
    Diagonal,
}

impl Default for FlipStrategy {
    fn default() -> Self {
        FlipStrategy::Random
    }
}
```

- [ ] **Step 2: Add field to `PlayerConfig`**

Add after `opponent_awareness` (line 95):

```rust
    #[serde(default)]
    pub flip_strategy: FlipStrategy,
```

- [ ] **Step 3: Update `PlayerConfig::default()`**

Add to the default impl struct literal (after line 104):

```rust
            flip_strategy: FlipStrategy::default(),
```

- [ ] **Step 4: Add test**

Add to `test_default_game_config`:

```rust
        assert_eq!(config.players[0].flip_strategy, FlipStrategy::Random);
```

- [ ] **Step 5: Verify and commit**

Run: `cd src-tauri && cargo test --lib engine::config`

```bash
git add src-tauri/src/engine/config.rs
git commit -m "feat: add FlipStrategy enum and config field"
```

---

## Task 2: Update grid.rs, game.rs, and state.rs grid construction (atomic — all callers must change together)

**Files:**
- Modify: `src-tauri/src/engine/grid.rs`
- Modify: `src-tauri/src/engine/game.rs`
- Modify: `src-tauri/src/interactive/state.rs`

- [ ] **Step 1: Add FlipStrategy import**

At the top of `grid.rs`, add:

```rust
use super::config::FlipStrategy;
```

- [ ] **Step 2: Add `flip_positions()` function**

Add as a free function before the `impl PlayerGrid` block (before line 47):

```rust
/// Given a FlipStrategy, return the 2 positions to flip face-up on a 4x4 grid.
fn flip_positions(strategy: &FlipStrategy, rng: &mut impl Rng) -> Vec<(usize, usize)> {
    match strategy {
        FlipStrategy::Random => {
            let mut positions: Vec<(usize, usize)> = (0..4)
                .flat_map(|r| (0..4).map(move |c| (r, c)))
                .collect();
            positions.shuffle(rng);
            positions[..2].to_vec()
        }
        FlipStrategy::SameColumn => {
            let col = rng.gen_range(0..4);
            let mut rows: Vec<usize> = (0..4).collect();
            rows.shuffle(rng);
            vec![(rows[0], col), (rows[1], col)]
        }
        FlipStrategy::SameRow => {
            let row = rng.gen_range(0..4);
            let mut cols: Vec<usize> = (0..4).collect();
            cols.shuffle(rng);
            vec![(row, cols[0]), (row, cols[1])]
        }
        FlipStrategy::Corners => {
            let mut corners = vec![(0, 0), (0, 3), (3, 0), (3, 3)];
            corners.shuffle(rng);
            corners[..2].to_vec()
        }
        FlipStrategy::Diagonal => {
            // Pick main or anti-diagonal randomly
            let diag: Vec<(usize, usize)> = if rng.gen_bool(0.5) {
                vec![(0, 0), (1, 1), (2, 2), (3, 3)] // main
            } else {
                vec![(0, 3), (1, 2), (2, 1), (3, 0)] // anti
            };
            let mut diag = diag;
            diag.shuffle(rng);
            diag[..2].to_vec()
        }
    }
}
```

- [ ] **Step 3: Update `PlayerGrid::new()` signature**

Change the `new` method signature and body. Replace the entire `new` method (lines 50-81) with:

```rust
    /// Create a new 4x4 grid from 16 cards, all face-down, then flip
    /// 2 cards based on the given strategy.
    pub fn new(cards: Vec<Card>, strategy: &FlipStrategy, rng: &mut impl Rng) -> Self {
        assert!(cards.len() == 16, "Grid requires exactly 16 cards");

        let mut cells: Vec<Vec<Option<GridCell>>> = Vec::with_capacity(4);
        let mut idx = 0;
        for _ in 0..4 {
            let mut row = Vec::with_capacity(4);
            for _ in 0..4 {
                row.push(Some(GridCell {
                    card: cards[idx].clone(),
                    face_up: false,
                }));
                idx += 1;
            }
            cells.push(row);
        }

        let mut grid = PlayerGrid { cells };

        let positions = flip_positions(strategy, rng);
        for (r, c) in positions {
            if let Some(ref mut cell) = grid.cells[r][c] {
                cell.face_up = true;
            }
        }

        grid
    }
```

- [ ] **Step 4: Add `new_no_flips()` constructor**

Add after the `new` method:

```rust
    /// Create a new 4x4 grid with all cards face-down (no initial flips).
    /// Used for human player in interactive mode who picks their own flips.
    pub fn new_no_flips(cards: Vec<Card>) -> Self {
        assert!(cards.len() == 16, "Grid requires exactly 16 cards");

        let mut cells: Vec<Vec<Option<GridCell>>> = Vec::with_capacity(4);
        let mut idx = 0;
        for _ in 0..4 {
            let mut row = Vec::with_capacity(4);
            for _ in 0..4 {
                row.push(Some(GridCell {
                    card: cards[idx].clone(),
                    face_up: false,
                }));
                idx += 1;
            }
            cells.push(row);
        }

        PlayerGrid { cells }
    }
```

- [ ] **Step 5: Add tests for flip strategies**

Add to the `tests` module in `grid.rs`:

```rust
    use super::super::config::FlipStrategy;

    #[test]
    fn test_flip_positions_random_returns_2() {
        let mut rng = rand::thread_rng();
        let pos = super::flip_positions(&FlipStrategy::Random, &mut rng);
        assert_eq!(pos.len(), 2);
        assert_ne!(pos[0], pos[1]);
    }

    #[test]
    fn test_flip_positions_same_column() {
        let mut rng = rand::thread_rng();
        let pos = super::flip_positions(&FlipStrategy::SameColumn, &mut rng);
        assert_eq!(pos.len(), 2);
        assert_eq!(pos[0].1, pos[1].1); // same column
        assert_ne!(pos[0].0, pos[1].0); // different rows
    }

    #[test]
    fn test_flip_positions_same_row() {
        let mut rng = rand::thread_rng();
        let pos = super::flip_positions(&FlipStrategy::SameRow, &mut rng);
        assert_eq!(pos.len(), 2);
        assert_eq!(pos[0].0, pos[1].0); // same row
        assert_ne!(pos[0].1, pos[1].1); // different cols
    }

    #[test]
    fn test_flip_positions_corners() {
        let mut rng = rand::thread_rng();
        let pos = super::flip_positions(&FlipStrategy::Corners, &mut rng);
        assert_eq!(pos.len(), 2);
        let corners = vec![(0,0), (0,3), (3,0), (3,3)];
        assert!(corners.contains(&pos[0]));
        assert!(corners.contains(&pos[1]));
    }

    #[test]
    fn test_flip_positions_diagonal() {
        let mut rng = rand::thread_rng();
        let pos = super::flip_positions(&FlipStrategy::Diagonal, &mut rng);
        assert_eq!(pos.len(), 2);
        let main_diag = vec![(0,0), (1,1), (2,2), (3,3)];
        let anti_diag = vec![(0,3), (1,2), (2,1), (3,0)];
        let on_main = main_diag.contains(&pos[0]) && main_diag.contains(&pos[1]);
        let on_anti = anti_diag.contains(&pos[0]) && anti_diag.contains(&pos[1]);
        assert!(on_main || on_anti);
    }

    #[test]
    fn test_new_no_flips_all_face_down() {
        let cards: Vec<Card> = (0..16).map(|i| Card::Number(i)).collect();
        let grid = PlayerGrid::new_no_flips(cards);
        for r in 0..4 {
            for c in 0..4 {
                let cell = grid.get(r, c).unwrap();
                assert!(!cell.face_up, "Cell ({},{}) should be face-down", r, c);
            }
        }
    }
```

- [ ] **Step 6: Update `play_round()` in `game.rs`**

In `game.rs` `play_round()` (around line 136-145), change:

```rust
    for _ in 0..player_count {
        let hand: Vec<Card> = deck.drain(..16).collect();
        let grid = PlayerGrid::new(hand, 2, rng);
```

To:

```rust
    for i in 0..player_count {
        let hand: Vec<Card> = deck.drain(..16).collect();
        let grid = PlayerGrid::new(hand, &config.players[i].flip_strategy, rng);
```

- [ ] **Step 7: Update `start_round()` in `state.rs`**

In `state.rs`, add `FlipStrategy` to the config import (line 7):

```rust
use crate::engine::config::{FlipStrategy, GameConfig, ScoringMode};
```

Replace the player creation loop in `start_round()` (lines 137-147) with:

```rust
        let mut players = Vec::with_capacity(player_count);
        for i in 0..player_count {
            let hand: Vec<Card> = deck.drain(..16).collect();
            let grid = if i == self.human_player {
                PlayerGrid::new_no_flips(hand)
            } else {
                PlayerGrid::new(hand, &self.config.players[i].flip_strategy, &mut self.rng)
            };
            players.push(PlayerState {
                grid,
                went_out_first: false,
                cleared_all: false,
                eliminations: 0,
            });
        }
```

- [ ] **Step 8: Verify and commit**

Run: `cd src-tauri && cargo test`

```bash
git add src-tauri/src/engine/grid.rs src-tauri/src/engine/game.rs src-tauri/src/interactive/state.rs
git commit -m "feat: add strategy-based flip positions, no-flips constructor, update all callers"
```

---

## Task 3: Add flip strategy dropdown to frontend config panel

**Files:**
- Modify: `src/js/config-panel.js`

- [ ] **Step 1: Update `PLAYER_PRESETS` to include `flipStrategy`**

Change the presets (lines 16-21) to:

```javascript
const PLAYER_PRESETS = {
  Beginner:     { keepThreshold: 2, lineAwareness: 10, opponentAwareness: 0, flipStrategy: 'Random' },
  Intermediate: { keepThreshold: 3, lineAwareness: 40, opponentAwareness: 20, flipStrategy: 'Random' },
  Advanced:     { keepThreshold: 4, lineAwareness: 70, opponentAwareness: 50, flipStrategy: 'Random' },
  Expert:       { keepThreshold: 5, lineAwareness: 95, opponentAwareness: 80, flipStrategy: 'Random' },
};
```

- [ ] **Step 2: Add dropdown to `buildPlayerPanel()`**

In `buildPlayerPanel()`, add after the Opponent Awareness slider block (after the closing `</div>` of the slider-group around line 130, before the closing `</div>` of the player-panel):

```javascript
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
```

- [ ] **Step 3: Update `applyPlayerPreset()` to set flip strategy**

Add after the `opponent_awareness` line (around line 142):

```javascript
  document.getElementById(`flip-strategy-${idx}`).value = p.flipStrategy;
```

- [ ] **Step 4: Update `applyToAll()` to copy flip strategy**

Add to the source value reads (around line 151):

```javascript
  const flipStrategy = document.getElementById(`flip-strategy-${src}`).value;
```

And inside the loop (around line 158):

```javascript
    document.getElementById(`flip-strategy-${i}`).value = flipStrategy;
```

- [ ] **Step 5: Update `buildConfigFromUI()` to include `flip_strategy`**

In the players loop (around line 188), add to the pushed object:

```javascript
      flip_strategy: document.getElementById(`flip-strategy-${i}`).value,
```

- [ ] **Step 6: Commit**

```bash
git add src/js/config-panel.js
git commit -m "feat: add flip strategy dropdown to player config panels"
```

---

## Task 4: Add `ChooseInitialFlips` phase to interactive state machine

**Files:**
- Modify: `src-tauri/src/interactive/state.rs`

- [ ] **Step 1: Import FlipStrategy**

Change the config import (line 7):

```rust
use crate::engine::config::{GameConfig, ScoringMode};
```

To:

```rust
use crate::engine::config::{FlipStrategy, GameConfig, ScoringMode};
```

- [ ] **Step 2: Add `flips_remaining` to `PendingAction`**

Change `PendingAction` (lines 37-42) to:

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PendingAction {
    pub action_type: String,
    pub drawn_card: Option<CardView>,
    pub flips_remaining: Option<u8>,
}
```

- [ ] **Step 3: Add `ChooseInitialFlips` guard to `advance_ai()`**

In `advance_ai()` (around line 259), add after the `RoundOver` check:

```rust
        if matches!(self.pending, InternalPending::ChooseInitialFlips { .. }) {
            return Err("Human must complete initial flip selection first".to_string());
        }
```

- [ ] **Step 4: Add `ChooseInitialFlips` variant to `InternalPending`**

Add to the `InternalPending` enum (after line 75):

```rust
    ChooseInitialFlips { remaining: u8 },
```

- [ ] **Step 5: Update `start_round()` pending state**

The grid constructor changes were already made in Task 2 Step 7. Now replace the pending state assignment at the end of `start_round()` (lines 163-167) with:

```rust
        // Human always picks initial flips first
        self.pending = InternalPending::ChooseInitialFlips { remaining: 2 };
```

- [ ] **Step 6: Add `human_flip_initial()` method**

Add after `human_draw()` (around line 196):

```rust
    pub fn human_flip_initial(&mut self, row: usize, col: usize) -> Result<(), String> {
        let remaining = match &self.pending {
            InternalPending::ChooseInitialFlips { remaining } => *remaining,
            _ => return Err("Not in initial flip selection phase".to_string()),
        };

        if row >= 4 || col >= 4 {
            return Err("Position out of bounds".to_string());
        }

        if !self.players[self.human_player].grid.flip_card(row, col) {
            return Err("Card is already face-up".to_string());
        }

        self.action_log.push(format!("You flipped card at ({},{}).", row, col));

        let new_remaining = remaining - 1;
        if new_remaining == 0 {
            // Done picking flips, transition to normal play
            if self.current_player == self.human_player {
                self.pending = InternalPending::ChooseDrawSource;
            } else {
                self.pending = InternalPending::NotYourTurn;
            }
        } else {
            self.pending = InternalPending::ChooseInitialFlips { remaining: new_remaining };
        }

        Ok(())
    }
```

- [ ] **Step 7: Update `get_state()` to handle the new pending variant**

In the `get_state()` method, find the `pending` match block (around line 683). Add a new arm before `InternalPending::NotYourTurn`:

```rust
            InternalPending::ChooseInitialFlips { remaining } => PendingAction {
                action_type: "choose_initial_flips".to_string(),
                drawn_card: None,
                flips_remaining: Some(*remaining),
            },
```

And update ALL other `PendingAction` constructors in the same match to include `flips_remaining: None`. There are 6 existing arms — each needs `flips_remaining: None` added. For example:

```rust
            InternalPending::ChooseDrawSource => PendingAction {
                action_type: "choose_draw_source".to_string(),
                drawn_card: None,
                flips_remaining: None,
            },
```

Do this for all 6: `ChooseDrawSource`, `HandleNormalCard`, `ChooseSlideDirection`, `NotYourTurn`, `RoundOver`, `GameOver`.

- [ ] **Step 8: Verify and commit**

Run: `cd src-tauri && cargo build`

```bash
git add src-tauri/src/interactive/state.rs
git commit -m "feat: add ChooseInitialFlips phase to interactive state machine"
```

---

## Task 5: Add Tauri command and bridge for flip initial

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/js/tauri-bridge.js`
- Modify: `src/js/play-panel.js`

- [ ] **Step 1: Add command in `commands.rs`**

After `play_get_state` (around line 197), add:

```rust
#[tauri::command]
pub fn play_flip_initial(
    row: usize,
    col: usize,
    state: State<'_, AppState>,
) -> Result<PlayableGameState, String> {
    let mut guard = state.interactive_game.lock().unwrap();
    let game = guard.as_mut().ok_or("No active game")?;
    game.human_flip_initial(row, col)?;
    Ok(game.get_state())
}
```

- [ ] **Step 2: Register in `main.rs`**

Add to `generate_handler!` (after `commands::play_get_state`):

```rust
            commands::play_flip_initial,
```

- [ ] **Step 3: Add bridge function in `tauri-bridge.js`**

After `tauriPlayGetState` (around line 67), add:

```javascript
async function tauriPlayFlipInitial(row, col) {
  return await invoke('play_flip_initial', { row, col });
}
```

- [ ] **Step 4: Update `canClickCell()` in `play-panel.js`**

In `canClickCell()` (around line 102-111), add a new condition before the `handle_normal_card` check:

```javascript
  if (p === 'choose_initial_flips') return cell.state === 'face_down';
```

- [ ] **Step 5: Update `handleCellClick()` in `play-panel.js`**

Find `handleCellClick()` in play-panel.js. Add handling for the new phase. At the top of the function (before any existing logic), add:

```javascript
  if (playState.pending.action_type === 'choose_initial_flips') {
    try {
      playState = await tauriPlayFlipInitial(row, col);
      renderPlayState();
    } catch (e) {
      alert(e);
    }
    return;
  }
```

- [ ] **Step 6: Update `renderPrompt()` in `play-panel.js`**

Find `renderPrompt()`. Add a case for the new phase. Where the function checks `action_type` values, add:

```javascript
  if (p === 'choose_initial_flips') {
    const remaining = playState.pending.flips_remaining || 0;
    prompt.innerHTML = `<p>Click ${remaining} card${remaining > 1 ? 's' : ''} to flip face-up</p>`;
    return;
  }
```

- [ ] **Step 7: Update `startPlayGame()` to include `flip_strategy` on player configs**

In `play-panel.js`, `startPlayGame()` (around line 22-27), update the player configs to include `flip_strategy`. Change:

```javascript
  config.players = [
    { keep_threshold: 5, line_awareness: 1.0, opponent_awareness: 0.5 },
    { ...aiConfig },
    { ...aiConfig },
    { ...aiConfig },
  ];
```

To:

```javascript
  config.players = [
    { keep_threshold: 5, line_awareness: 1.0, opponent_awareness: 0.5, flip_strategy: 'Random' },
    { ...aiConfig, flip_strategy: 'Random' },
    { ...aiConfig, flip_strategy: 'Random' },
    { ...aiConfig, flip_strategy: 'Random' },
  ];
```

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/main.rs src/js/tauri-bridge.js src/js/play-panel.js
git commit -m "feat: add flip initial command and frontend handling"
```

---

## Task 6: Final integration verification

- [ ] **Step 1: Run all Rust tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass.

- [ ] **Step 2: Launch app and test**

Run: `cargo tauri dev`

Test AI flip strategies (simulation):
1. Set different players to different flip strategies
2. Run a simulation
3. Verify it completes without errors

Test human manual flip (interactive play):
1. Start a game from the Play tab
2. Verify you see all 16 cards face-down on your grid
3. Verify prompt says "Click 2 cards to flip face-up"
4. Click a face-down card — it flips, prompt updates to "Click 1 card..."
5. Click another — it flips, game transitions to normal play
6. Try clicking an already face-up card — should show error
7. Play a few turns to verify the game works normally after flips

- [ ] **Step 3: Commit any fixes**
