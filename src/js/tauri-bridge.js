const { invoke } = window.__TAURI__.core;

async function tauriGetDefaultConfig() {
  return await invoke('get_default_config');
}

async function tauriRunSimulation(config, numGames, runName, saveDetailed) {
  return await invoke('run_simulation_cmd', {
    config: config,
    numGames: numGames,
    runName: runName,
    saveDetailed: saveDetailed,
  });
}

async function tauriGetProgress() {
  return await invoke('get_progress');
}

async function tauriListRuns() {
  return await invoke('list_runs_cmd');
}

async function tauriGetRun(runId) {
  return await invoke('get_run_cmd', { runId: runId });
}

async function tauriCompareRuns(runIdA, runIdB) {
  return await invoke('compare_runs_cmd', { runIdA: runIdA, runIdB: runIdB });
}

async function tauriDeleteRun(runId) {
  return await invoke('delete_run_cmd', { runId: runId });
}

async function tauriExportRunToFile(runId, filePath) {
  return await invoke('export_run_to_file_cmd', { runId: runId, filePath: filePath });
}

async function tauriHasDetailedData(runId) {
  return await invoke('has_detailed_data_cmd', { runId: runId });
}

async function tauriExportRunDetailedToFile(runId, filePath) {
  return await invoke('export_run_detailed_to_file_cmd', { runId: runId, filePath: filePath });
}

// ── Interactive Play ─────────────────────────────────────────────────────

async function tauriStartPlayGame(config) {
  return await invoke('start_play_game', { config });
}

async function tauriPlayDraw(source) {
  return await invoke('play_draw', { source });
}

async function tauriPlayAction(actionType, params) {
  return await invoke('play_action', { actionType, params });
}

async function tauriPlaySlide(direction) {
  return await invoke('play_slide', { direction });
}

async function tauriPlayAiTurn() {
  return await invoke('play_ai_turn');
}

async function tauriPlayNextRound() {
  return await invoke('play_next_round');
}

async function tauriPlayGetState() {
  return await invoke('play_get_state');
}

async function tauriPlayFlipInitial(row, col) {
  return await invoke('play_flip_initial', { row, col });
}

async function tauriChooseElimination(index) {
  return await invoke('play_choose_elimination', { index });
}
