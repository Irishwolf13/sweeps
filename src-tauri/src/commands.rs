use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use tauri::State;

use crate::engine::config::GameConfig;
use crate::history::compare::ComparisonResult;
use crate::history::store::RunMeta;
use crate::history::{compare, store};
use crate::interactive::state::{ActionParams, InteractiveGame, PlayableGameState};
use crate::simulation::runner;
use crate::simulation::stats::SimulationSummary;

pub struct AppState {
    pub progress: Arc<AtomicU32>,
    pub total_games: Arc<AtomicU32>,
    pub running: Arc<AtomicU32>,
    pub interactive_game: Mutex<Option<InteractiveGame>>,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            progress: Arc::new(AtomicU32::new(0)),
            total_games: Arc::new(AtomicU32::new(0)),
            running: Arc::new(AtomicU32::new(0)),
            interactive_game: Mutex::new(None),
        }
    }
}

#[tauri::command]
pub fn get_default_config() -> GameConfig {
    GameConfig::default()
}

/// Run simulation on a background thread so the UI stays responsive
/// for progress polling. Uses a channel to send the result back.
#[tauri::command]
pub async fn run_simulation_cmd(
    config: GameConfig,
    num_games: u32,
    run_name: String,
    save_detailed: bool,
    state: State<'_, AppState>,
) -> Result<SimulationSummary, String> {
    config
        .deck
        .validate(config.player_count)
        .map_err(|e| e.to_string())?;

    state.progress.store(0, Ordering::Relaxed);
    state.total_games.store(num_games, Ordering::Relaxed);
    state.running.store(1, Ordering::Relaxed);

    let progress = Arc::clone(&state.progress);
    let running = Arc::clone(&state.running);

    // Use a oneshot channel to get the result from the background thread
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let summary = runner::run_simulation(&config, num_games, run_name, progress, save_detailed);
        running.store(0, Ordering::Relaxed);
        let _ = tx.send(summary);
    });

    // Wait for the result (this await yields to Tauri's async runtime,
    // allowing other commands like get_progress to be handled)
    let summary = tauri::async_runtime::spawn_blocking(move || {
        rx.recv()
            .map_err(|e| format!("Simulation thread error: {}", e))
    })
    .await
    .map_err(|e| format!("Runtime error: {}", e))?
    .map_err(|e| e.to_string())?;

    // Auto-save the run
    store::save_run(&summary).map_err(|e| format!("Failed to save run: {}", e))?;

    Ok(summary)
}

#[tauri::command]
pub fn get_progress(state: State<'_, AppState>) -> (u32, u32, u32) {
    (
        state.progress.load(Ordering::Relaxed),
        state.total_games.load(Ordering::Relaxed),
        state.running.load(Ordering::Relaxed),
    )
}

#[tauri::command]
pub fn list_runs_cmd() -> Result<Vec<RunMeta>, String> {
    store::list_runs()
}

#[tauri::command]
pub fn get_run_cmd(run_id: String) -> Result<SimulationSummary, String> {
    store::get_run(&run_id)
}

#[tauri::command]
pub fn compare_runs_cmd(
    run_id_a: String,
    run_id_b: String,
) -> Result<ComparisonResult, String> {
    let a = store::get_run(&run_id_a)?;
    let b = store::get_run(&run_id_b)?;
    Ok(compare::compare_runs(&a, &b))
}

#[tauri::command]
pub fn delete_run_cmd(run_id: String) -> Result<bool, String> {
    store::delete_run(&run_id)
}

#[tauri::command]
pub fn export_run_to_file_cmd(run_id: String, file_path: String) -> Result<(), String> {
    let csv = store::export_run_csv(&run_id)?;
    std::fs::write(&file_path, csv).map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(())
}

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

// ── Interactive Play Commands ────────────────────────────────────────────

#[tauri::command]
pub fn start_play_game(
    config: GameConfig,
    state: State<'_, AppState>,
) -> Result<PlayableGameState, String> {
    config.deck.validate(config.player_count).map_err(|e| e.to_string())?;
    let game = InteractiveGame::new(config);
    let view = game.get_state();
    *state.interactive_game.lock().unwrap() = Some(game);
    Ok(view)
}

#[tauri::command]
pub fn play_draw(
    source: String,
    state: State<'_, AppState>,
) -> Result<PlayableGameState, String> {
    let mut guard = state.interactive_game.lock().unwrap();
    let game = guard.as_mut().ok_or("No active game")?;
    game.human_draw(&source)?;
    Ok(game.get_state())
}

#[tauri::command]
pub fn play_action(
    action_type: String,
    params: ActionParams,
    state: State<'_, AppState>,
) -> Result<PlayableGameState, String> {
    let mut guard = state.interactive_game.lock().unwrap();
    let game = guard.as_mut().ok_or("No active game")?;
    game.human_action(&action_type, &params)?;
    Ok(game.get_state())
}

#[tauri::command]
pub fn play_slide(
    direction: String,
    state: State<'_, AppState>,
) -> Result<PlayableGameState, String> {
    let mut guard = state.interactive_game.lock().unwrap();
    let game = guard.as_mut().ok_or("No active game")?;
    game.human_slide(&direction)?;
    Ok(game.get_state())
}

#[tauri::command]
pub fn play_ai_turn(
    state: State<'_, AppState>,
) -> Result<PlayableGameState, String> {
    let mut guard = state.interactive_game.lock().unwrap();
    let game = guard.as_mut().ok_or("No active game")?;
    game.advance_ai()?;
    Ok(game.get_state())
}

#[tauri::command]
pub fn play_next_round(
    state: State<'_, AppState>,
) -> Result<PlayableGameState, String> {
    let mut guard = state.interactive_game.lock().unwrap();
    let game = guard.as_mut().ok_or("No active game")?;
    game.advance_round()?;
    Ok(game.get_state())
}

#[tauri::command]
pub fn play_get_state(
    state: State<'_, AppState>,
) -> Result<PlayableGameState, String> {
    let guard = state.interactive_game.lock().unwrap();
    let game = guard.as_ref().ok_or("No active game")?;
    Ok(game.get_state())
}

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
