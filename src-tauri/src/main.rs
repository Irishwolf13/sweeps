#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod engine;
mod history;
mod interactive;
mod simulation;

use commands::AppState;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_default_config,
            commands::run_simulation_cmd,
            commands::get_progress,
            commands::list_runs_cmd,
            commands::get_run_cmd,
            commands::compare_runs_cmd,
            commands::delete_run_cmd,
            commands::export_run_to_file_cmd,
            commands::has_detailed_data_cmd,
            commands::export_run_detailed_to_file_cmd,
            commands::start_play_game,
            commands::play_draw,
            commands::play_action,
            commands::play_slide,
            commands::play_ai_turn,
            commands::play_next_round,
            commands::play_get_state,
            commands::play_flip_initial,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
