# Starting Order Config & Export Fix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a configurable starting-order option (round-robin vs worst-score-first) to the simulation engine, and fix the broken CSV export by using Tauri's native save dialog.

**Architecture:** Two independent features touching shared config. Feature 1 adds a `StartingOrder` enum to the Rust config, modifies the game loop to use cumulative scores for starting-player selection, and adds a UI dropdown. Feature 2 adds the `tauri-plugin-dialog` dependency, replaces the broken blob-download with a native save dialog, and routes the file write through a new backend command.

**Tech Stack:** Rust (Tauri 2.x backend), HTML/JS/CSS frontend (no bundler), `tauri-plugin-dialog` for native file dialogs.

**Spec:** `docs/superpowers/specs/2026-03-23-starting-order-and-export-fix-design.md`

---

## File Map

**Feature 1 — Starting Order:**
| File | Action | Responsibility |
|------|--------|----------------|
| `src-tauri/src/engine/config.rs` | Modify | Add `StartingOrder` enum, field on `GameConfig`, default impl |
| `src-tauri/src/engine/game.rs` | Modify | Use cumulative scores + config to pick starting player each round |
| `src/index.html` | Modify | Add Starting Order dropdown to Game Rules section |
| `src/js/config-panel.js` | Modify | Read dropdown value in `buildConfigFromUI()` |

**Feature 2 — Export Fix:**
| File | Action | Responsibility |
|------|--------|----------------|
| `src-tauri/Cargo.toml` | Modify | Add `tauri-plugin-dialog` dependency |
| `src-tauri/src/main.rs` | Modify | Register dialog plugin, replace old export command |
| `src-tauri/src/commands.rs` | Modify | Replace `export_run_csv_cmd` with `export_run_to_file_cmd` |
| `src-tauri/capabilities/default.json` | Modify | Add `"dialog:default"` permission |
| `src/js/tauri-bridge.js` | Modify | Replace `tauriExportRunCsv` with `tauriExportRunToFile` |
| `src/js/history-panel.js` | Modify | Use native save dialog + new bridge function |

---

## Task 1: Add `StartingOrder` enum and config field

**Files:**
- Modify: `src-tauri/src/engine/config.rs:53-122`

- [ ] **Step 1: Add `StartingOrder` enum after `ScoringMode`**

In `src-tauri/src/engine/config.rs`, add after the `ScoringMode` `Default` impl (after line 63):

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum StartingOrder {
    RoundRobin,
    WorstScoreFirst,
}

impl Default for StartingOrder {
    fn default() -> Self {
        StartingOrder::RoundRobin
    }
}
```

- [ ] **Step 2: Add field to `GameConfig` struct**

Add after the `scoring_mode` field (line 103):

```rust
    #[serde(default)]
    pub starting_order: StartingOrder,
```

The `#[serde(default)]` ensures old saved runs (which lack this field) still deserialize correctly.

- [ ] **Step 3: Add field to `GameConfig::default()`**

In the `Default` impl for `GameConfig` (line 112-121), add to the struct literal:

```rust
            starting_order: StartingOrder::default(),
```

- [ ] **Step 4: Add test for the new default**

Add to the existing `test_default_game_config` test in `config.rs`:

```rust
        assert_eq!(config.starting_order, StartingOrder::RoundRobin);
```

- [ ] **Step 5: Verify it compiles and tests pass**

Run: `cargo test -p number-sweep-sim --lib engine::config`
Expected: All config tests pass including the new assertion.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/engine/config.rs
git commit -m "feat: add StartingOrder enum and config field"
```

---

## Task 2: Implement starting-order logic in game engine

**Files:**
- Modify: `src-tauri/src/engine/game.rs:65-127`

- [ ] **Step 1: Add helper function to determine starting player**

First, update the imports at the top of `game.rs` (line 5). Change:

```rust
use super::config::{GameConfig, ScoringMode};
```

To:

```rust
use super::config::{GameConfig, ScoringMode, StartingOrder};
```

Then add this helper function before `play_round` (around line 94):

```rust
/// Determine which player starts a round based on config.
fn determine_starting_player(
    config: &GameConfig,
    round_number: u8,
    cumulative_scores: &[i32],
) -> usize {
    let player_count = config.player_count as usize;

    match config.starting_order {
        StartingOrder::RoundRobin => (round_number as usize) % player_count,
        StartingOrder::WorstScoreFirst => {
            if round_number == 0 || cumulative_scores.is_empty() {
                // Round 0: no scores yet, fall back to round-robin
                0
            } else {
                // Worst = highest score (lower is better in this game).
                // Ties broken by lowest player index.
                cumulative_scores
                    .iter()
                    .enumerate()
                    .max_by_key(|(i, &score)| (score, -((*i) as i32)))
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            }
        }
    }
}
```

- [ ] **Step 2: Update `play_game()` to pass cumulative scores**

Replace the `play_round` call inside the `for` loop in `play_game()` (line 71):

```rust
        let starting = determine_starting_player(config, round_num, &cumulative_scores);
        let result = play_round(config, round_num, starting, rng);
```

- [ ] **Step 3: Update `play_round()` signature to accept starting player**

Change `play_round` signature (line 96) from:

```rust
fn play_round(config: &GameConfig, round_number: u8, rng: &mut impl Rng) -> RoundResult {
```

To:

```rust
fn play_round(config: &GameConfig, round_number: u8, starting_player: usize, rng: &mut impl Rng) -> RoundResult {
```

And replace line 127:

```rust
        current_player: (round_number as usize) % player_count,
```

With:

```rust
        current_player: starting_player,
```

- [ ] **Step 4: Add test for worst-score-first logic**

Add to the `tests` module in `game.rs`:

```rust
    #[test]
    fn test_determine_starting_player_round_robin() {
        let config = GameConfig::default();
        assert_eq!(determine_starting_player(&config, 0, &[]), 0);
        assert_eq!(determine_starting_player(&config, 1, &[0, 0, 0, 0]), 1);
        assert_eq!(determine_starting_player(&config, 2, &[0, 0, 0, 0]), 2);
    }

    #[test]
    fn test_determine_starting_player_worst_first() {
        let mut config = GameConfig::default();
        config.starting_order = StartingOrder::WorstScoreFirst;

        // Round 0: always player 0 (no scores)
        assert_eq!(determine_starting_player(&config, 0, &[]), 0);

        // Player 2 has highest (worst) score
        assert_eq!(determine_starting_player(&config, 1, &[5, 3, 10, 7]), 2);

        // Tie: players 0 and 3 both have 8 — lowest index wins
        assert_eq!(determine_starting_player(&config, 2, &[8, 3, 5, 8]), 0);
    }

    #[test]
    fn test_play_game_worst_score_first_no_panic() {
        let mut config = GameConfig::default();
        config.starting_order = StartingOrder::WorstScoreFirst;
        let mut rng = rand::thread_rng();
        let result = play_game(&config, &mut rng);
        assert_eq!(result.round_results.len(), 4);
        assert!(result.total_turns > 0);
    }
```

- [ ] **Step 5: Verify all game tests pass**

Run: `cargo test -p number-sweep-sim --lib engine::game`
Expected: All tests pass including the three new ones.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/engine/game.rs
git commit -m "feat: implement worst-score-first starting order logic"
```

---

## Task 3: Add Starting Order dropdown to frontend

**Files:**
- Modify: `src/index.html:74-81`
- Modify: `src/js/config-panel.js:192-200`

- [ ] **Step 1: Add dropdown to `index.html`**

In `src/index.html`, after the Scoring Mode `</div>` (line 81), add a new config group within the same `config-row`:

```html
          <div class="config-group">
            <label>Starting Order</label>
            <select id="starting-order">
              <option value="RoundRobin">Round Robin</option>
              <option value="WorstScoreFirst">Worst Score First</option>
            </select>
          </div>
```

- [ ] **Step 2: Add `starting_order` to `buildConfigFromUI()`**

In `src/js/config-panel.js`, add to the return object in `buildConfigFromUI()` (after line 197, the `scoring_mode` line):

```javascript
    starting_order: document.getElementById('starting-order').value,
```

- [ ] **Step 3: Manually verify in browser**

Run: `cargo tauri dev`
Expected: "Starting Order" dropdown appears next to "Scoring Mode" in the Game Rules section. Both "Round Robin" and "Worst Score First" options are selectable. Running a simulation with each option should succeed without errors.

- [ ] **Step 4: Commit**

```bash
git add src/index.html src/js/config-panel.js
git commit -m "feat: add Starting Order dropdown to config panel"
```

---

## Task 4: Add `tauri-plugin-dialog` dependency and registration

**Files:**
- Modify: `src-tauri/Cargo.toml:15`
- Modify: `src-tauri/src/main.rs:12`
- Modify: `src-tauri/capabilities/default.json:6`

- [ ] **Step 1: Add dependency to `Cargo.toml`**

In `src-tauri/Cargo.toml`, add to `[dependencies]`:

```toml
tauri-plugin-dialog = "2"
```

- [ ] **Step 2: Register plugin in `main.rs`**

In `src-tauri/src/main.rs`, add the plugin to the builder chain. Change:

```rust
    tauri::Builder::default()
        .manage(AppState::default())
```

To:

```rust
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
```

- [ ] **Step 3: Add permission to capabilities**

In `src-tauri/capabilities/default.json`, add `"dialog:default"` to the permissions array:

```json
  "permissions": [
    "core:default",
    "dialog:default"
  ]
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p number-sweep-sim`
Expected: Compiles successfully with the dialog plugin.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/main.rs src-tauri/capabilities/default.json
git commit -m "feat: add tauri-plugin-dialog for native save dialogs"
```

---

## Task 5: Replace export command and frontend with native save dialog

**Files:**
- Modify: `src-tauri/src/commands.rs:117-120`
- Modify: `src-tauri/src/main.rs:22` (handler list)
- Modify: `src/js/tauri-bridge.js:35-37`
- Modify: `src/js/history-panel.js:125-141`

- [ ] **Step 1: Replace backend command in `commands.rs`**

Replace the `export_run_csv_cmd` function (lines 117-120) with:

```rust
#[tauri::command]
pub fn export_run_to_file_cmd(run_id: String, file_path: String) -> Result<(), String> {
    let csv = store::export_run_csv(&run_id)?;
    std::fs::write(&file_path, csv).map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(())
}
```

- [ ] **Step 2: Update handler list in `main.rs`**

In `src-tauri/src/main.rs`, replace `commands::export_run_csv_cmd` with `commands::export_run_to_file_cmd` in the `generate_handler!` macro.

- [ ] **Step 3: Replace bridge function in `tauri-bridge.js`**

Replace `tauriExportRunCsv` (lines 35-37) with:

```javascript
async function tauriExportRunToFile(runId, filePath) {
  return await invoke('export_run_to_file_cmd', { runId: runId, filePath: filePath });
}
```

- [ ] **Step 4: Rewrite `exportRun()` in `history-panel.js`**

Replace the entire `exportRun` function (lines 125-141) with:

```javascript
async function exportRun(runId, runName) {
  try {
    const safeName = runName.replace(/[^a-zA-Z0-9]/g, '_');
    const filePath = await window.__TAURI__.dialog.save({
      defaultPath: `${safeName}.csv`,
      filters: [{ name: 'CSV Files', extensions: ['csv'] }],
    });

    if (!filePath) return; // User cancelled

    await tauriExportRunToFile(runId, filePath);
    alert('Export saved successfully!');
  } catch (e) {
    alert('Export failed: ' + e);
  }
}
```

- [ ] **Step 5: Manually verify export flow**

Run: `cargo tauri dev`
1. Run a simulation (or view an existing one from History tab)
2. Click "Export" on a run
Expected: Native OS "Save As" dialog appears with `.csv` filter and a default filename. After picking a location, the file is saved and a success alert shows. Opening the CSV in a text editor or Excel shows the simulation data.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/main.rs src/js/tauri-bridge.js src/js/history-panel.js
git commit -m "fix: use native save dialog for CSV export"
```

---

## Task 6: Final integration verification

- [ ] **Step 1: Run all Rust tests**

Run: `cargo test -p number-sweep-sim`
Expected: All tests pass.

- [ ] **Step 2: Launch app and test both features end-to-end**

Run: `cargo tauri dev`

Test starting order:
1. Set Starting Order to "Worst Score First"
2. Run a simulation with 1000+ games
3. Verify simulation completes without errors
4. Compare results with a "Round Robin" simulation — first mover advantage stats should differ

Test export:
1. Go to History tab
2. Click Export on any run
3. Verify native save dialog appears
4. Save the file, verify it contains correct CSV data

- [ ] **Step 3: Commit any final fixes if needed**
