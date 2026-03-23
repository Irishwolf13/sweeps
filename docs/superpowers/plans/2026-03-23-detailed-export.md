# Detailed Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add opt-in saving of raw per-game data during simulation, with a detailed CSV export option showing every game, round, and player stat.

**Architecture:** When the user checks "Save detailed data", the simulation runner serializes the full `Vec<GameResult>` to `{id}_raw.json` before dropping it. The export button detects whether detailed data exists and offers a choice between summary and detailed CSV. The detailed CSV has one row per game-round with all player stats.

**Tech Stack:** Rust (Tauri 2.x backend), HTML/JS/CSS frontend (no bundler), serde_json for serialization.

**Spec:** `docs/superpowers/specs/2026-03-23-detailed-export-design.md`

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src-tauri/src/engine/game.rs:11-33` | Modify | Add Serialize/Deserialize derives to GameResult and RoundResult |
| `src-tauri/src/history/store.rs:17` | Modify | Make `runs_dir()` pub, add `has_detailed_data()`, `export_run_detailed_csv()`, update `delete_run()`, filter `_raw.json` in `list_runs()` |
| `src-tauri/src/simulation/runner.rs` | Modify | Accept `save_detailed` flag, write raw JSON inside function |
| `src-tauri/src/commands.rs:40-81` | Modify | Pass `save_detailed` to runner, add new commands |
| `src-tauri/src/main.rs:15-31` | Modify | Register new commands |
| `src/index.html:119-149` | Modify | Add "Save detailed data" checkbox |
| `src/js/app.js:30-67` | Modify | Read checkbox, pass to bridge |
| `src/js/tauri-bridge.js:7-13` | Modify | Update `tauriRunSimulation`, add new bridge functions |
| `src/js/history-panel.js:125-140` | Modify | Add export type choice when detailed data exists |

---

## Task 1: Add serde derives to GameResult and RoundResult

**Files:**
- Modify: `src-tauri/src/engine/game.rs:11,19`

- [ ] **Step 1: Add Serialize, Deserialize derives**

In `src-tauri/src/engine/game.rs`, add `use serde::{Serialize, Deserialize};` at the top (after line 1, with the other imports).

Change line 11:
```rust
#[derive(Clone, Debug)]
pub struct GameResult {
```
To:
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameResult {
```

Change line 19:
```rust
#[derive(Clone, Debug)]
pub struct RoundResult {
```
To:
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoundResult {
```

- [ ] **Step 2: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: Compiles (serde is already a dependency).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/engine/game.rs
git commit -m "feat: add serde derives to GameResult and RoundResult"
```

---

## Task 2: Update store.rs — public runs_dir, filter raw files, cleanup on delete

**Files:**
- Modify: `src-tauri/src/history/store.rs`

- [ ] **Step 1: Make `runs_dir()` public**

Change line 17:
```rust
fn runs_dir() -> Result<PathBuf, String> {
```
To:
```rust
pub fn runs_dir() -> Result<PathBuf, String> {
```

- [ ] **Step 2: Filter `_raw.json` files in `list_runs()`**

In `list_runs()`, after the existing check for `.json` extension (line 42-44), add a filter to skip raw data files. Change:

```rust
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
```

To:

```rust
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        // Skip raw game data files
        if path.file_name().and_then(|n| n.to_str()).map_or(false, |n| n.ends_with("_raw.json")) {
            continue;
        }
```

- [ ] **Step 3: Update `delete_run()` to clean up raw data**

Replace the `delete_run` function (lines 80-89) with:

```rust
pub fn delete_run(run_id: &str) -> Result<bool, String> {
    let dir = runs_dir()?;
    let path = dir.join(format!("{}.json", run_id));
    let raw_path = dir.join(format!("{}_raw.json", run_id));

    // Delete raw data file if it exists
    if raw_path.exists() {
        fs::remove_file(&raw_path).map_err(|e| format!("Delete raw data error: {}", e))?;
    }

    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Delete error: {}", e))?;
        Ok(true)
    } else {
        Ok(false)
    }
}
```

- [ ] **Step 4: Add `has_detailed_data()` function**

Add after `delete_run`:

```rust
pub fn has_detailed_data(run_id: &str) -> Result<bool, String> {
    let dir = runs_dir()?;
    let raw_path = dir.join(format!("{}_raw.json", run_id));
    Ok(raw_path.exists())
}
```

- [ ] **Step 5: Add `export_run_detailed_csv()` function**

Add after `has_detailed_data`. This function needs to import `GameResult`:

At the top of `store.rs`, add:
```rust
use crate::engine::game::GameResult;
```

Then add the function:

```rust
pub fn export_run_detailed_csv(run_id: &str) -> Result<String, String> {
    let dir = runs_dir()?;
    let raw_path = dir.join(format!("{}_raw.json", run_id));
    let data = fs::read_to_string(&raw_path).map_err(|e| format!("Read raw data error: {}", e))?;
    let results: Vec<GameResult> =
        serde_json::from_str(&data).map_err(|e| format!("Deserialize raw data error: {}", e))?;

    if results.is_empty() {
        return Ok(String::from("No game data\n"));
    }

    let player_count = results[0].player_scores.len();

    // Build header
    let mut header = String::from("Game,Round,Turns,Draw Pile Exhausted,Game Winner");
    for p in 1..=player_count {
        header += &format!(
            ",P{} Round Score,P{} Eliminations,P{} Cards Remaining,P{} Went Out First,P{} Cleared All",
            p, p, p, p, p
        );
    }
    header += "\n";

    let mut csv = header;

    for (game_idx, result) in results.iter().enumerate() {
        let game_num = game_idx + 1;
        let winner = result.winner + 1; // 1-indexed

        for round in &result.round_results {
            let round_num = round.round_number + 1; // 1-indexed
            csv += &format!(
                "{},{},{},{},{}",
                game_num,
                round_num,
                round.turns,
                round.draw_pile_exhausted,
                winner,
            );

            for p in 0..player_count {
                let score = round.player_round_scores.get(p).copied().unwrap_or(0);
                let elims = round.eliminations_per_player.get(p).copied().unwrap_or(0);
                let remaining = round.cards_remaining_per_player.get(p).copied().unwrap_or(0);
                let went_out = round.went_out_first == Some(p);
                let cleared = round.cleared_all.contains(&p);
                csv += &format!(",{},{},{},{},{}", score, elims, remaining, went_out, cleared);
            }

            csv += "\n";
        }
    }

    Ok(csv)
}
```

- [ ] **Step 6: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: Compiles successfully.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/history/store.rs
git commit -m "feat: add detailed data storage helpers and CSV export"
```

---

## Task 3: Update simulation runner to save raw data

**Files:**
- Modify: `src-tauri/src/simulation/runner.rs`

- [ ] **Step 1: Update `run_simulation` signature and add raw data saving**

Replace the entire `runner.rs` with:

```rust
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use rayon::prelude::*;

use crate::engine::config::GameConfig;
use crate::engine::game::{play_game, GameResult};
use crate::history::store;
use crate::simulation::stats::{aggregate, SimulationSummary};

/// Maximum batch size to limit peak memory usage.
/// At ~1KB per GameResult, 50K games ≈ 50MB per batch.
const BATCH_SIZE: u32 = 50_000;

/// Run a simulation of `num_games` four-round games in parallel.
/// For large simulations, processes in batches to limit memory.
/// When `save_detailed` is true, writes raw game data to `{id}_raw.json`.
pub fn run_simulation(
    config: &GameConfig,
    num_games: u32,
    run_name: String,
    progress: Arc<AtomicU32>,
    save_detailed: bool,
) -> SimulationSummary {
    let id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    let results: Vec<GameResult> = if num_games <= BATCH_SIZE {
        (0..num_games)
            .into_par_iter()
            .map(|_| {
                let mut rng = rand::thread_rng();
                let result = play_game(config, &mut rng);
                progress.fetch_add(1, Ordering::Relaxed);
                result
            })
            .collect()
    } else {
        let mut all_results = Vec::with_capacity(num_games as usize);
        let mut remaining = num_games;

        while remaining > 0 {
            let batch = remaining.min(BATCH_SIZE);
            let batch_results: Vec<GameResult> = (0..batch)
                .into_par_iter()
                .map(|_| {
                    let mut rng = rand::thread_rng();
                    let result = play_game(config, &mut rng);
                    progress.fetch_add(1, Ordering::Relaxed);
                    result
                })
                .collect();

            all_results.extend(batch_results);
            remaining -= batch;
        }

        all_results
    };

    // Save raw game data if requested (must happen before results are consumed)
    if save_detailed {
        if let Ok(dir) = store::runs_dir() {
            let raw_path = dir.join(format!("{}_raw.json", id));
            if let Ok(json) = serde_json::to_string(&results) {
                let _ = std::fs::write(&raw_path, json);
            }
        }
    }

    aggregate(&results, config, id, run_name, timestamp)
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: Compiles. Note: `commands.rs` will need updating next (Task 4) since the call site changed.

Note: Do NOT commit yet — `commands.rs` still calls `run_simulation` without the new parameter. Continue to Task 4 and commit together.

---

## Task 4: Update commands and main.rs (commit with Task 3)

**Files:**
- Modify: `src-tauri/src/commands.rs:40-81`
- Modify: `src-tauri/src/main.rs:15-31`

- [ ] **Step 1: Update `run_simulation_cmd` to accept `save_detailed`**

In `src-tauri/src/commands.rs`, update the `run_simulation_cmd` function signature (line 40-44). Add `save_detailed: bool` parameter:

```rust
#[tauri::command]
pub async fn run_simulation_cmd(
    config: GameConfig,
    num_games: u32,
    run_name: String,
    save_detailed: bool,
    state: State<'_, AppState>,
) -> Result<SimulationSummary, String> {
```

And update the `run_simulation` call inside the `std::thread::spawn` closure (line 62):

```rust
        let summary = runner::run_simulation(&config, num_games, run_name, progress, save_detailed);
```

- [ ] **Step 2: Add new commands**

After `export_run_to_file_cmd` (line 122), add:

```rust
#[tauri::command]
pub fn has_detailed_data_cmd(run_id: String) -> Result<bool, String> {
    store::has_detailed_data(&run_id)
}

#[tauri::command]
pub fn export_run_detailed_to_file_cmd(run_id: String, file_path: String) -> Result<(), String> {
    let csv = store::export_run_detailed_csv(&run_id)?;
    std::fs::write(&file_path, csv).map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(())
}
```

- [ ] **Step 3: Register new commands in `main.rs`**

In `src-tauri/src/main.rs`, add to the `generate_handler!` macro (after `commands::export_run_to_file_cmd`):

```rust
            commands::has_detailed_data_cmd,
            commands::export_run_detailed_to_file_cmd,
```

- [ ] **Step 4: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: Compiles successfully.

- [ ] **Step 5: Run tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass.

- [ ] **Step 6: Commit (includes Task 3 runner changes)**

```bash
git add src-tauri/src/simulation/runner.rs src-tauri/src/commands.rs src-tauri/src/main.rs
git commit -m "feat: add save_detailed flag to runner and wire up detailed export commands"
```

---

## Task 5: Update frontend — checkbox, bridge, and app.js

**Files:**
- Modify: `src/index.html:119-149`
- Modify: `src/js/app.js:30-67`
- Modify: `src/js/tauri-bridge.js:7-13`

- [ ] **Step 1: Add checkbox to index.html**

In `src/index.html`, in the Simulation Controls section, add a new config group inside the existing `config-row` (after the "Number of Games" `</div>` which ends around line 138, before the Run button `<div>`):

Find:
```html
          <div class="config-group">
            <button id="run-btn" class="btn-primary" onclick="runSimulation()">Run Simulation</button>
          </div>
```

Add before it:
```html
          <div class="config-group toggle-group">
            <label>Save Detailed Data</label>
            <label class="toggle">
              <input type="checkbox" id="save-detailed" />
              <span class="toggle-slider"></span>
            </label>
            <span class="toggle-label">Per-game data for detailed CSV export</span>
          </div>
```

- [ ] **Step 2: Update `tauriRunSimulation` in bridge**

In `src/js/tauri-bridge.js`, replace the `tauriRunSimulation` function (lines 7-13):

```javascript
async function tauriRunSimulation(config, numGames, runName, saveDetailed) {
  return await invoke('run_simulation_cmd', {
    config: config,
    numGames: numGames,
    runName: runName,
    saveDetailed: saveDetailed,
  });
}
```

- [ ] **Step 3: Add new bridge functions**

In `src/js/tauri-bridge.js`, after `tauriExportRunToFile` (line 37), add:

```javascript
async function tauriHasDetailedData(runId) {
  return await invoke('has_detailed_data_cmd', { runId: runId });
}

async function tauriExportRunDetailedToFile(runId, filePath) {
  return await invoke('export_run_detailed_to_file_cmd', { runId: runId, filePath: filePath });
}
```

- [ ] **Step 4: Update `runSimulation()` in app.js**

In `src/js/app.js`, find the line (around line 67):

```javascript
    const summary = await tauriRunSimulation(config, numGames, runName);
```

Replace with:

```javascript
    const saveDetailed = document.getElementById('save-detailed').checked;
    const summary = await tauriRunSimulation(config, numGames, runName, saveDetailed);
```

- [ ] **Step 5: Commit**

```bash
git add src/index.html src/js/tauri-bridge.js src/js/app.js
git commit -m "feat: add save detailed data checkbox and wire up to backend"
```

---

## Task 6: Update export flow with type choice

**Files:**
- Modify: `src/js/history-panel.js:123-140`

- [ ] **Step 1: Replace `exportRun()` with choice-aware version**

Replace the entire export section (lines 123-140) with:

```javascript
// ── Export Run ─────────────────────────────────────────────────────────────

let activeExportPopup = null;

async function exportRun(runId, runName) {
  // Close any existing popup
  if (activeExportPopup) {
    activeExportPopup.remove();
    activeExportPopup = null;
  }

  try {
    const hasDetailed = await tauriHasDetailedData(runId);

    if (!hasDetailed) {
      // No detailed data — export summary directly
      await doExport(runId, runName, 'summary');
      return;
    }

    // Show inline choice popup in the action cell
    // Find the action cell for this run by looking for the export button
    const exportBtns = document.querySelectorAll('.action-cell button');
    let actionCell = null;
    for (const btn of exportBtns) {
      if (btn.textContent === 'Export' && btn.onclick && btn.onclick.toString().includes(runId)) {
        actionCell = btn.parentElement;
        break;
      }
    }
    if (!actionCell) return;

    const popup = document.createElement('div');
    popup.className = 'export-popup';
    popup.innerHTML = `
      <button class="btn-small" onclick="doExport('${runId}', '${escapeHtml(runName).replace(/'/g, "\\'")}', 'summary'); closeExportPopup()">Export Summary</button>
      <button class="btn-small" onclick="doExport('${runId}', '${escapeHtml(runName).replace(/'/g, "\\'")}', 'detailed'); closeExportPopup()">Export Detailed</button>
      <button class="btn-small btn-danger-small" onclick="closeExportPopup()">Cancel</button>
    `;

    actionCell.appendChild(popup);
    activeExportPopup = popup;
  } catch (e) {
    alert('Export failed: ' + e);
  }
}

function closeExportPopup() {
  if (activeExportPopup) {
    activeExportPopup.remove();
    activeExportPopup = null;
  }
}

async function doExport(runId, runName, exportType) {
  try {
    const safeName = runName.replace(/[^a-zA-Z0-9]/g, '_');
    const suffix = exportType === 'detailed' ? '_detailed' : '_summary';
    const filePath = await window.__TAURI__.dialog.save({
      defaultPath: `${safeName}${suffix}.csv`,
      filters: [{ name: 'CSV Files', extensions: ['csv'] }],
    });

    if (!filePath) return; // User cancelled

    if (exportType === 'detailed') {
      await tauriExportRunDetailedToFile(runId, filePath);
    } else {
      await tauriExportRunToFile(runId, filePath);
    }

    alert('Export saved successfully!');
  } catch (e) {
    alert('Export failed: ' + e);
  }
}
```

- [ ] **Step 2: Add CSS for the export popup**

Find the main CSS file and add basic popup styling. Check which CSS file exists:

In `src/styles/main.css`, add at the end:

```css
/* Export type popup */
.action-cell {
  position: relative;
}

.export-popup {
  display: flex;
  gap: 6px;
  padding: 8px;
  background: var(--bg-secondary, #2a2a2a);
  border: 1px solid var(--border-color, #444);
  border-radius: 6px;
  margin-top: 6px;
  position: absolute;
  right: 0;
  z-index: 10;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
}
```

If the app doesn't use CSS variables, use the fallback values directly. Check the existing CSS to match the theme.

If the app doesn't use CSS variables, use the fallback values directly. Check the existing CSS to match the style.

- [ ] **Step 3: Commit**

```bash
git add src/js/history-panel.js src/styles/main.css
git commit -m "feat: add export type choice for detailed data"
```

---

## Task 7: Final integration verification

- [ ] **Step 1: Run all Rust tests**

Run: `cd src-tauri && cargo test`
Expected: All tests pass.

- [ ] **Step 2: Launch app and test end-to-end**

Run: `cargo tauri dev`

Test detailed export:
1. Check "Save detailed data" checkbox
2. Run a small simulation (1,000 games)
3. Go to History tab, click Export
4. Verify you see "Export Summary" / "Export Detailed" / "Cancel" buttons
5. Click "Export Detailed" — verify native save dialog appears
6. Save file, open in text editor — verify CSV has one row per game-round with all player columns

Test without detailed data:
1. Uncheck "Save detailed data"
2. Run another simulation
3. Click Export — should go directly to save dialog (summary only, no choice popup)

Test cleanup:
1. Delete a run that has detailed data — should not leave orphaned `_raw.json` files

- [ ] **Step 3: Commit any final fixes if needed**
