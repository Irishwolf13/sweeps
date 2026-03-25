use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::engine::game::GameResult;
use crate::simulation::stats::SimulationSummary;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RunMeta {
    pub id: String,
    pub run_name: String,
    pub timestamp: String,
    pub num_games: u32,
    pub player_count: u8,
}

pub fn runs_dir() -> Result<PathBuf, String> {
    let base = dirs::data_local_dir().ok_or("Could not find local data directory")?;
    let dir = base.join("sweep-sim").join("runs");
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create runs directory: {}", e))?;
    Ok(dir)
}

pub fn save_run(summary: &SimulationSummary) -> Result<(), String> {
    let dir = runs_dir()?;
    let path = dir.join(format!("{}.json", summary.id));
    let json =
        serde_json::to_string_pretty(summary).map_err(|e| format!("Serialize error: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Write error: {}", e))?;
    Ok(())
}

pub fn list_runs() -> Result<Vec<RunMeta>, String> {
    let dir = runs_dir()?;
    let mut runs = Vec::new();

    let entries = fs::read_dir(&dir).map_err(|e| format!("Read dir error: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Entry error: {}", e))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        // Skip raw game data files
        if path.file_name().and_then(|n| n.to_str()).map_or(false, |n| n.ends_with("_raw.json")) {
            continue;
        }

        let data = match fs::read_to_string(&path) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let summary: SimulationSummary = match serde_json::from_str(&data) {
            Ok(s) => s,
            Err(_) => continue,
        };

        runs.push(RunMeta {
            id: summary.id,
            run_name: summary.run_name,
            timestamp: summary.timestamp,
            num_games: summary.num_games,
            player_count: summary.config.player_count,
        });
    }

    // Sort by timestamp descending (newest first)
    runs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    Ok(runs)
}

pub fn get_run(run_id: &str) -> Result<SimulationSummary, String> {
    let dir = runs_dir()?;
    let path = dir.join(format!("{}.json", run_id));
    let data = fs::read_to_string(&path).map_err(|e| format!("Read error: {}", e))?;
    let summary: SimulationSummary =
        serde_json::from_str(&data).map_err(|e| format!("Deserialize error: {}", e))?;
    Ok(summary)
}

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

pub fn has_detailed_data(run_id: &str) -> Result<bool, String> {
    let dir = runs_dir()?;
    let raw_path = dir.join(format!("{}_raw.json", run_id));
    Ok(raw_path.exists())
}

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

pub fn export_run_csv(run_id: &str) -> Result<String, String> {
    let summary = get_run(run_id)?;
    let mut csv = String::from("Metric,Value\n");

    csv += &format!("Run Name,{}\n", summary.run_name);
    csv += &format!("Timestamp,{}\n", summary.timestamp);
    csv += &format!("Num Games,{}\n", summary.num_games);
    csv += &format!("Player Count,{}\n", summary.config.player_count);
    csv += &format!("Scoring Mode,{:?}\n", summary.config.scoring_mode);
    csv += &format!(
        "Matching Elimination,{}\n",
        summary.config.allow_matching_elimination
    );
    csv += &format!(
        "Diagonal Elimination,{}\n",
        summary.config.allow_diagonal_elimination
    );
    csv += "\n";

    csv += &format!("Avg Turns/Round,{:.1}\n", summary.avg_turns_per_round);
    csv += &format!(
        "Avg Eliminations/Round,{:.2}\n",
        summary.avg_eliminations_per_round
    );
    csv += &format!("Avg Score/Round,{:.1}\n", summary.avg_score_per_round);
    csv += &format!(
        "First Mover Advantage,{:.1}%\n",
        summary.first_mover_advantage
    );
    csv += &format!(
        "Draw Pile Exhaustion,{:.1}%\n",
        summary.draw_pile_exhaustion_rate
    );
    csv += &format!(
        "Effective Deck Usage,{:.1}%\n",
        summary.effective_deck_usage
    );
    csv += &format!(
        "Round Completion Rate,{:.1}%\n",
        summary.round_completion_rate
    );
    csv += &format!("Went Out First Rate,{:.1}%\n", summary.went_out_first_rate);
    csv += &format!("Cleared All Rate,{:.1}%\n", summary.cleared_all_rate);
    csv += "\n";

    for (i, ps) in summary.player_summaries.iter().enumerate() {
        csv += &format!("Player {} Avg Score,{:.1}\n", i + 1, ps.avg_score);
        csv += &format!("Player {} Win Rate,{:.1}%\n", i + 1, ps.win_rate);
        csv += &format!(
            "Player {} Avg Eliminations,{:.2}\n",
            i + 1,
            ps.avg_eliminations
        );
        csv += &format!(
            "Player {} Avg Cards Remaining,{:.1}\n",
            i + 1,
            ps.avg_cards_remaining
        );
    }

    Ok(csv)
}
