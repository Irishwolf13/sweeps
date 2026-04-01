use serde::{Deserialize, Serialize};

use crate::engine::config::GameConfig;
use crate::engine::game::GameResult;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerSummary {
    pub avg_score: f64,
    pub win_rate: f64,
    pub avg_eliminations: f64,
    pub avg_cards_remaining: f64,
    pub went_out_first_count: u32,
    pub cleared_all_count: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SimulationSummary {
    pub id: String,
    pub run_name: String,
    pub timestamp: String,
    pub config: GameConfig,
    pub num_games: u32,

    // Game-level stats
    pub avg_total_score: Vec<f64>,
    pub median_total_score: Vec<i32>,
    pub stddev_total_score: Vec<f64>,
    pub min_total_score: Vec<i32>,
    pub max_total_score: Vec<i32>,
    pub win_rates: Vec<f64>,
    pub first_mover_triggered_rate: f64,
    pub first_mover_lowest_score_rate: f64,
    pub avg_turns_per_round: f64,

    // Elimination stats
    pub avg_eliminations_per_round: f64,

    // Card stats
    pub draw_pile_exhaustion_rate: f64,
    pub avg_cards_remaining: Vec<f64>,
    // Scoring stats
    pub avg_score_per_round: f64,
    pub went_out_first_rate: f64,
    pub cleared_all_rate: f64,

    // Deck health
    pub effective_deck_usage: f64,
    pub round_completion_rate: f64,
    pub avg_draw_pile_remaining: f64,

    // Per-player breakdown
    pub player_summaries: Vec<PlayerSummary>,

    // Score distribution: per-player histograms
    // Each entry is a Vec of (bucket_center, count) pairs
    pub score_histograms: Vec<Vec<(i32, u32)>>,
}

/// Aggregate results from many game simulations into a summary.
pub fn aggregate(
    results: &[GameResult],
    config: &GameConfig,
    id: String,
    run_name: String,
    timestamp: String,
) -> SimulationSummary {
    let num_games = results.len() as u32;
    let player_count = config.player_count as usize;
    let num_games_f = num_games as f64;

    // ── Per-player accumulators ───────────────────────────────────────────
    let mut score_sums = vec![0i64; player_count];
    let mut score_lists: Vec<Vec<i32>> = vec![Vec::with_capacity(results.len()); player_count];
    let mut win_counts = vec![0u32; player_count];
    let mut elim_sums = vec![0u64; player_count];
    let mut remaining_sums = vec![0u64; player_count];
    let mut went_out_first_counts = vec![0u32; player_count];
    let mut cleared_all_counts = vec![0u32; player_count];

    // ── Game-level accumulators ───────────────────────────────────────────
    let mut total_turns = 0u64;
    let mut total_rounds = 0u64;
    let mut first_mover_triggered = 0u64;
    let mut first_mover_lowest_score = 0u64;
    let mut total_score_all = 0i64;
    let mut draw_pile_exhausted_rounds = 0u64;
    let mut went_out_first_rounds = 0u64;
    let mut cleared_all_rounds = 0u64;
    let mut total_eliminations = 0u64;

    // ── Deck health accumulators ──────────────────────────────────────────
    let mut total_cards_drawn_all = 0u64;
    let mut total_deck_size_all = 0u64;
    let mut rounds_completed_naturally = 0u64;

    for result in results {
        // Winners (ties count for all tied players)
        for &w in &result.winners {
            win_counts[w] += 1;
        }

        // Per-player scores
        for (i, &score) in result.player_scores.iter().enumerate() {
            score_sums[i] += score as i64;
            score_lists[i].push(score);
        }

        // Round-level stats
        for round in &result.round_results {
            total_turns += round.turns as u64;
            total_rounds += 1;

            if round.draw_pile_exhausted {
                draw_pile_exhausted_rounds += 1;
            }

            if round.went_out_first.is_some() {
                went_out_first_rounds += 1;
            }

            if !round.cleared_all.is_empty() {
                cleared_all_rounds += 1;
            }

            // Deck health
            total_cards_drawn_all += round.total_cards_drawn as u64;
            total_deck_size_all += round.total_deck_size as u64;
            if round.round_completed_naturally {
                rounds_completed_naturally += 1;
            }

            // First mover round-level stats
            let starter = round.starting_player;
            if round.went_out_first == Some(starter) {
                first_mover_triggered += 1;
            }
            if let Some(min_score) = round.player_round_scores.iter().min() {
                if round.player_round_scores[starter] == *min_score {
                    first_mover_lowest_score += 1;
                }
            }

            // Per-player round stats
            for i in 0..player_count {
                if i < round.eliminations_per_player.len() {
                    elim_sums[i] += round.eliminations_per_player[i] as u64;
                    total_eliminations += round.eliminations_per_player[i] as u64;
                }
                if i < round.cards_remaining_per_player.len() {
                    remaining_sums[i] += round.cards_remaining_per_player[i] as u64;
                }
                if i < round.player_round_scores.len() {
                    total_score_all += round.player_round_scores[i] as i64;
                }
                if round.went_out_first == Some(i) {
                    went_out_first_counts[i] += 1;
                }
                if round.cleared_all.contains(&i) {
                    cleared_all_counts[i] += 1;
                }
            }
        }
    }

    // ── Compute aggregated metrics ────────────────────────────────────────

    let total_rounds_f = total_rounds as f64;

    // Per-player score stats
    let avg_total_score: Vec<f64> = score_sums
        .iter()
        .map(|&s| s as f64 / num_games_f)
        .collect();

    let mut median_total_score = Vec::with_capacity(player_count);
    let mut stddev_total_score = Vec::with_capacity(player_count);
    let mut min_total_score = Vec::with_capacity(player_count);
    let mut max_total_score = Vec::with_capacity(player_count);

    for i in 0..player_count {
        let mut scores = score_lists[i].clone();
        scores.sort();

        let median = if scores.is_empty() {
            0
        } else if scores.len() % 2 == 1 {
            scores[scores.len() / 2]
        } else {
            let mid = scores.len() / 2;
            (scores[mid - 1] + scores[mid]) / 2
        };
        median_total_score.push(median);

        let min = scores.first().copied().unwrap_or(0);
        let max = scores.last().copied().unwrap_or(0);
        min_total_score.push(min);
        max_total_score.push(max);

        let mean = avg_total_score[i];
        let variance = scores
            .iter()
            .map(|&s| {
                let diff = s as f64 - mean;
                diff * diff
            })
            .sum::<f64>()
            / num_games_f;
        stddev_total_score.push(variance.sqrt());
    }

    // Win rates
    let win_rates: Vec<f64> = win_counts
        .iter()
        .map(|&w| (w as f64 / num_games_f) * 100.0)
        .collect();

    let first_mover_triggered_rate = if total_rounds > 0 {
        (first_mover_triggered as f64 / total_rounds_f) * 100.0
    } else {
        0.0
    };
    let first_mover_lowest_score_rate = if total_rounds > 0 {
        (first_mover_lowest_score as f64 / total_rounds_f) * 100.0
    } else {
        0.0
    };

    // Averages
    let avg_turns_per_round = if total_rounds > 0 {
        total_turns as f64 / total_rounds_f
    } else {
        0.0
    };

    let avg_eliminations_per_round = if total_rounds > 0 {
        total_eliminations as f64 / total_rounds_f
    } else {
        0.0
    };

    let draw_pile_exhaustion_rate = if total_rounds > 0 {
        (draw_pile_exhausted_rounds as f64 / total_rounds_f) * 100.0
    } else {
        0.0
    };

    let avg_cards_remaining: Vec<f64> = remaining_sums
        .iter()
        .map(|&s| s as f64 / total_rounds_f)
        .collect();

    let avg_score_per_round = if total_rounds > 0 {
        total_score_all as f64 / total_rounds_f / player_count as f64
    } else {
        0.0
    };

    let went_out_first_rate = if total_rounds > 0 {
        (went_out_first_rounds as f64 / total_rounds_f) * 100.0
    } else {
        0.0
    };

    let cleared_all_rate = if total_rounds > 0 {
        (cleared_all_rounds as f64 / total_rounds_f) * 100.0
    } else {
        0.0
    };

    // Deck health
    let effective_deck_usage = if total_deck_size_all > 0 {
        (total_cards_drawn_all as f64 / total_deck_size_all as f64) * 100.0
    } else {
        0.0
    };

    let round_completion_rate = if total_rounds > 0 {
        (rounds_completed_naturally as f64 / total_rounds_f) * 100.0
    } else {
        0.0
    };

    let total_draw_remaining: f64 = results.iter()
        .flat_map(|g| g.round_results.iter())
        .map(|r| r.draw_pile_remaining as f64)
        .sum();
    let avg_draw_pile_remaining = if total_rounds > 0 {
        total_draw_remaining / total_rounds_f
    } else {
        0.0
    };

    // Per-player summaries
    let player_summaries: Vec<PlayerSummary> = (0..player_count)
        .map(|i| PlayerSummary {
            avg_score: avg_total_score[i],
            win_rate: win_rates[i],
            avg_eliminations: elim_sums[i] as f64 / total_rounds_f,
            avg_cards_remaining: avg_cards_remaining[i],
            went_out_first_count: went_out_first_counts[i],
            cleared_all_count: cleared_all_counts[i],
        })
        .collect();

    // Score histograms: bin each player's total scores
    let score_histograms: Vec<Vec<(i32, u32)>> = (0..player_count)
        .map(|i| {
            let scores = &score_lists[i];
            if scores.is_empty() {
                return Vec::new();
            }
            let s_min = *scores.iter().min().unwrap();
            let s_max = *scores.iter().max().unwrap();
            let range = (s_max - s_min).max(1);

            // Use ~20-30 bins, but at least 1-wide
            let bin_width = (range as f64 / 25.0).ceil().max(1.0) as i32;
            let bin_start = (s_min / bin_width) * bin_width - bin_width;
            let bin_end = ((s_max / bin_width) + 1) * bin_width + bin_width;
            let num_bins = ((bin_end - bin_start) / bin_width) as usize;

            let mut counts = vec![0u32; num_bins];
            for &score in scores {
                let idx = ((score - bin_start) / bin_width) as usize;
                if idx < counts.len() {
                    counts[idx] += 1;
                }
            }

            counts.iter().enumerate()
                .map(|(idx, &count)| {
                    let center = bin_start + (idx as i32) * bin_width + bin_width / 2;
                    (center, count)
                })
                .collect()
        })
        .collect();

    SimulationSummary {
        id,
        run_name,
        timestamp,
        config: config.clone(),
        num_games,
        avg_total_score,
        median_total_score,
        stddev_total_score,
        min_total_score,
        max_total_score,
        win_rates,
        first_mover_triggered_rate,
        first_mover_lowest_score_rate,
        avg_turns_per_round,
        avg_eliminations_per_round,
        draw_pile_exhaustion_rate,
        avg_cards_remaining,
        avg_score_per_round,
        went_out_first_rate,
        cleared_all_rate,
        effective_deck_usage,
        round_completion_rate,
        avg_draw_pile_remaining,
        player_summaries,
        score_histograms,
    }
}
