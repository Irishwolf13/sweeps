# Detailed Export — Opt-in Raw Game Data

## Problem
The CSV export only contains aggregated summary stats. The user wants access to per-game, per-round, per-player data for deeper analysis in spreadsheets.

## Design

### Config Panel
- New checkbox "Save detailed data" near the simulation run controls (game count / Run button area), not in Game Rules.
- Default: unchecked.

### Backend — Serialization (`game.rs`)
- Add `Serialize, Deserialize` derives to `GameResult` and `RoundResult` (currently only `Clone, Debug`).

### Backend — Simulation Runner (`runner.rs`)
- `run_simulation()` accepts a new `save_detailed: bool` parameter.
- When `save_detailed` is true, the raw `Vec<GameResult>` is written to `{id}_raw.json` **inside `run_simulation()`**, before the results vector is dropped. This is critical — the raw data cannot be returned alongside the summary without doubling peak memory.
- `run_simulation()` needs the runs directory path (use the same `runs_dir()` helper from `store.rs`, made `pub`).
- The summary JSON is always saved afterward in `commands.rs` (unchanged).
- **Known limitation:** For very large simulations (100K+ games), serializing the full `Vec<GameResult>` to JSON will temporarily double memory usage during serialization. This is acceptable since detailed export is opt-in and intended for smaller deep-dive runs.

### Backend — Storage (`store.rs`)
- Make `runs_dir()` pub so `runner.rs` can use it.
- New function `has_detailed_data(run_id: &str) -> Result<bool, String>` — checks if `{id}_raw.json` exists.
- New function `export_run_detailed_csv(run_id: &str) -> Result<String, String>` — loads `{id}_raw.json`, builds CSV with one row per game-round:
  - Columns: `Game, Round, Turns, Draw Pile Exhausted, Game Winner`
  - Per player (repeated for each): `P{n} Round Score, P{n} Eliminations, P{n} Cards Remaining, P{n} Went Out First, P{n} Cleared All`
- Update `delete_run()` to also delete `{id}_raw.json` if it exists.
- Update `list_runs()` to filter out `_raw.json` files (currently it iterates all `.json` files and tries to parse as SimulationSummary — the raw files would fail silently but waste I/O).

### Backend — Commands (`commands.rs`)
- Update `run_simulation_cmd` to accept `save_detailed: bool` and pass it through to `run_simulation()`.
- New command `has_detailed_data_cmd(run_id: String) -> Result<bool, String>`.
- New command `export_run_detailed_to_file_cmd(run_id: String, file_path: String) -> Result<(), String>` — generates detailed CSV via `store::export_run_detailed_csv()` and writes to disk.

### Backend — Main (`main.rs`)
- Register new commands in `generate_handler!`.

### Frontend — Config Panel (`index.html` + `config-panel.js`)
- New checkbox with id `save-detailed` near the run controls.
- Read checkbox value and pass as `saveDetailed` when calling `tauriRunSimulation`.

### Frontend — Bridge (`tauri-bridge.js`)
- Update `tauriRunSimulation` to accept and pass `saveDetailed` parameter.
- New `tauriHasDetailedData(runId)` function.
- New `tauriExportRunDetailedToFile(runId, filePath)` function.

### Frontend — Export Flow (`history-panel.js`)
- On export click: call `tauriHasDetailedData(runId)`.
- If no detailed data: straight to native save dialog → summary CSV (current flow).
- If detailed data exists: show a small inline two-button dialog ("Export Summary" / "Export Detailed") within the history panel. Not `confirm()` (can't show two named options). A simple inline prompt that appears near the export button, dismissed after selection.
- Based on selection, open native save dialog, then call the appropriate export command.

### Frontend — App (`app.js`)
- Update simulation run call to pass `saveDetailed` from checkbox.
