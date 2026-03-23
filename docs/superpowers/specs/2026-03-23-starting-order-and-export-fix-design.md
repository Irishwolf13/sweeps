# Starting Order Config & Export Fix

## Feature 1: Configurable Starting Order

### Problem
Currently, the starting player each round is determined by round-robin (`round_number % player_count`). The user wants the option to let the worst-scoring player go first instead.

### Design

**New enum in `config.rs`:**
```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum StartingOrder {
    RoundRobin,
    WorstScoreFirst,
}
```
Default: `RoundRobin`. Add `#[serde(default)]` on the `GameConfig` field for backward compatibility with saved runs that lack this field.

**New field on `GameConfig`:**
```rust
pub starting_order: StartingOrder,
```
Also add to `GameConfig::default()`.

**Game logic changes in `game.rs`:**
- `play_game()` passes cumulative scores into `play_round()`.
- `play_round()` accepts cumulative scores `&[i32]` and the `StartingOrder` from config.
- **Scoring note:** In this game, lower scores are better. The "worst" player has the highest cumulative score.
- For `WorstScoreFirst`: starting player is the one with the highest cumulative score. Ties broken by lowest player index.
- Round 0: always uses round-robin (no scores exist yet).

**Interactive play path (`interactive/state.rs`):**
- Interactive play keeps round-robin behavior regardless of config. The `starting_order` config only affects simulations. No changes needed to `InteractiveGame`.

**Frontend (`index.html` + `config-panel.js`):**
- New dropdown in the Game Rules section of `index.html`, after Scoring Mode:
  ```html
  <div class="config-group">
    <label>Starting Order</label>
    <select id="starting-order">
      <option value="RoundRobin">Round Robin</option>
      <option value="WorstScoreFirst">Worst Score First</option>
    </select>
  </div>
  ```
- `buildConfigFromUI()` reads `#starting-order` value and includes `starting_order` in the config object.

## Feature 2: Fix Export with Native Save Dialog

### Problem
The export button uses a browser blob download trick (`document.createElement('a')`) which does not work in Tauri's webview. Clicking Export does nothing visible.

### Design

**Dependencies:**
- Add `tauri-plugin-dialog` to `Cargo.toml`
- Register plugin in `main.rs`: `.plugin(tauri_plugin_dialog::init())`
- Add `"dialog:default"` to `capabilities/default.json`
- No npm package needed — this project uses plain JS with `window.__TAURI__` globals. The dialog plugin exposes its API at `window.__TAURI__.dialog` (specifically `window.__TAURI__.dialog.save()`).

**Backend (`commands.rs`):**
- New command `export_run_to_file(run_id: String, file_path: String)` that generates the CSV via existing `export_run_csv()` and writes it to the given path with `std::fs::write`.

**Frontend (`history-panel.js`):**
- `exportRun()` calls `window.__TAURI__.dialog.save()` with a `.csv` filter and default filename based on run name.
- If the user picks a path, calls `tauriExportRunToFile(runId, filePath)`.
- If the user cancels (returns null), does nothing.
- On error: shows `alert()` with the error message (matching existing pattern).
- On success: shows a brief success alert.

**Bridge (`tauri-bridge.js`):**
- New function `tauriExportRunToFile(runId, filePath)`.

**Cleanup:**
- Remove old `export_run_csv_cmd` from `commands.rs` and `main.rs` handler list.
- Remove old `tauriExportRunCsv` from `tauri-bridge.js`.

**Existing CSV generation in `store.rs` is unchanged.**
