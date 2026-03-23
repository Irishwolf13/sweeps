use serde::{Deserialize, Serialize};

use crate::simulation::stats::SimulationSummary;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MetricDiff {
    pub name: String,
    pub run_a: f64,
    pub run_b: f64,
    pub delta: f64,
    pub percent_change: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ComparisonResult {
    pub run_a_id: String,
    pub run_a_name: String,
    pub run_b_id: String,
    pub run_b_name: String,
    pub diffs: Vec<MetricDiff>,
}

fn diff(name: &str, a: f64, b: f64) -> MetricDiff {
    let delta = b - a;
    let percent_change = if a.abs() > 0.0001 {
        (delta / a) * 100.0
    } else if b.abs() > 0.0001 {
        100.0
    } else {
        0.0
    };
    MetricDiff {
        name: name.to_string(),
        run_a: a,
        run_b: b,
        delta,
        percent_change,
    }
}

pub fn compare_runs(a: &SimulationSummary, b: &SimulationSummary) -> ComparisonResult {
    let mut diffs = Vec::new();

    // Game-level stats
    diffs.push(diff("Avg Turns/Round", a.avg_turns_per_round, b.avg_turns_per_round));
    diffs.push(diff("Avg Eliminations/Round", a.avg_eliminations_per_round, b.avg_eliminations_per_round));
    diffs.push(diff("Avg Score/Round", a.avg_score_per_round, b.avg_score_per_round));
    diffs.push(diff("First Mover Advantage", a.first_mover_advantage, b.first_mover_advantage));

    // Deck health
    diffs.push(diff("Draw Pile Exhaustion %", a.draw_pile_exhaustion_rate, b.draw_pile_exhaustion_rate));
    diffs.push(diff("Effective Deck Usage %", a.effective_deck_usage, b.effective_deck_usage));
    diffs.push(diff("Round Completion %", a.round_completion_rate, b.round_completion_rate));

    // Scoring
    diffs.push(diff("Went Out First %", a.went_out_first_rate, b.went_out_first_rate));
    diffs.push(diff("Cleared All %", a.cleared_all_rate, b.cleared_all_rate));

    // Win rates per player
    let max_players = a.win_rates.len().max(b.win_rates.len());
    for i in 0..max_players {
        let va = a.win_rates.get(i).copied().unwrap_or(0.0);
        let vb = b.win_rates.get(i).copied().unwrap_or(0.0);
        diffs.push(diff(&format!("Player {} Win Rate %", i + 1), va, vb));
    }

    // Per-player stats
    let max_ps = a.player_summaries.len().max(b.player_summaries.len());
    for i in 0..max_ps {
        let pa = a.player_summaries.get(i);
        let pb = b.player_summaries.get(i);

        let prefix = format!("P{}", i + 1);

        diffs.push(diff(
            &format!("{} Avg Score", prefix),
            pa.map(|p| p.avg_score).unwrap_or(0.0),
            pb.map(|p| p.avg_score).unwrap_or(0.0),
        ));
        diffs.push(diff(
            &format!("{} Avg Eliminations", prefix),
            pa.map(|p| p.avg_eliminations).unwrap_or(0.0),
            pb.map(|p| p.avg_eliminations).unwrap_or(0.0),
        ));
        diffs.push(diff(
            &format!("{} Avg Cards Remaining", prefix),
            pa.map(|p| p.avg_cards_remaining).unwrap_or(0.0),
            pb.map(|p| p.avg_cards_remaining).unwrap_or(0.0),
        ));
    }

    ComparisonResult {
        run_a_id: a.id.clone(),
        run_a_name: a.run_name.clone(),
        run_b_id: b.id.clone(),
        run_b_name: b.run_name.clone(),
        diffs,
    }
}
