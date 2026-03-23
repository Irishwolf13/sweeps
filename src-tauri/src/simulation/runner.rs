use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use rayon::prelude::*;

use crate::engine::config::GameConfig;
use crate::engine::game::{play_game, GameResult};
use crate::simulation::stats::{aggregate, SimulationSummary};

/// Maximum batch size to limit peak memory usage.
/// At ~1KB per GameResult, 50K games ≈ 50MB per batch.
const BATCH_SIZE: u32 = 50_000;

/// Run a simulation of `num_games` four-round games in parallel.
/// For large simulations, processes in batches to limit memory.
pub fn run_simulation(
    config: &GameConfig,
    num_games: u32,
    run_name: String,
    progress: Arc<AtomicU32>,
) -> SimulationSummary {
    let id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    if num_games <= BATCH_SIZE {
        // Small simulation — collect all at once
        let results: Vec<GameResult> = (0..num_games)
            .into_par_iter()
            .map(|_| {
                let mut rng = rand::thread_rng();
                let result = play_game(config, &mut rng);
                progress.fetch_add(1, Ordering::Relaxed);
                result
            })
            .collect();

        aggregate(&results, config, id, run_name, timestamp)
    } else {
        // Large simulation — process in batches, collect all results
        // but release each batch after aggregation would require streaming stats.
        // For now, use batched collection to avoid one massive allocation.
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

        aggregate(&all_results, config, id, run_name, timestamp)
    }
}
