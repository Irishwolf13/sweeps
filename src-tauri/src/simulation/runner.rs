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
