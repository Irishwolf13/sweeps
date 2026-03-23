use std::sync::atomic::AtomicU32;
use std::sync::Arc;

use number_sweep_sim::engine::config::{DeckConfig, FlipStrategy, GameConfig, PlayerConfig, ScoringMode};
use number_sweep_sim::engine::game::play_game;
use number_sweep_sim::simulation::runner::run_simulation;
use number_sweep_sim::history::{store, compare};

#[test]
fn test_play_100_games_no_panics() {
    let config = GameConfig::default();
    let mut rng = rand::thread_rng();

    let mut total_turns = 0u64;
    let mut score_sums = vec![0i64; config.player_count as usize];
    let mut wins = vec![0u32; config.player_count as usize];
    let num_games = 100u32;

    for _ in 0..num_games {
        let result = play_game(&config, &mut rng);
        total_turns += result.total_turns as u64;
        wins[result.winner] += 1;
        for (i, &s) in result.player_scores.iter().enumerate() {
            score_sums[i] += s as i64;
        }
    }

    println!("\n=== Smoke Test: {} games ===", num_games);
    println!(
        "Avg turns per game: {:.1}",
        total_turns as f64 / num_games as f64
    );
    for i in 0..config.player_count as usize {
        println!(
            "Player {}: avg score = {:.1}, wins = {}",
            i + 1,
            score_sums[i] as f64 / num_games as f64,
            wins[i]
        );
    }
    println!();

    // Sanity assertions
    assert!(total_turns > 0, "games should have turns");
    assert_eq!(
        wins.iter().sum::<u32>(),
        num_games,
        "every game should have a winner"
    );
}

#[test]
fn test_two_player_games() {
    let mut config = GameConfig::default();
    config.player_count = 2;
    config.players.truncate(2);
    let mut rng = rand::thread_rng();

    for _ in 0..20 {
        let result = play_game(&config, &mut rng);
        assert_eq!(result.player_scores.len(), 2);
        assert!(result.winner < 2);
        assert_eq!(result.round_results.len(), 4);
    }
}

#[test]
fn test_no_special_eliminations() {
    let mut config = GameConfig::default();
    config.allow_matching_elimination = false;
    config.allow_diagonal_elimination = false;
    let mut rng = rand::thread_rng();

    for _ in 0..20 {
        let result = play_game(&config, &mut rng);
        assert_eq!(result.round_results.len(), 4);
        assert!(result.total_turns > 0);
    }
}

#[test]
fn test_expert_scoring_mode() {
    use number_sweep_sim::engine::config::ScoringMode;

    let mut config = GameConfig::default();
    config.scoring_mode = ScoringMode::Expert;
    let mut rng = rand::thread_rng();

    for _ in 0..20 {
        let result = play_game(&config, &mut rng);
        assert_eq!(result.round_results.len(), 4);
    }
}

#[test]
fn test_skilled_vs_unskilled() {
    let mut config = GameConfig::default();
    config.player_count = 2;

    // Player 1: highly skilled
    let skilled = PlayerConfig {
        keep_threshold: 4,
        line_awareness: 1.0,
        opponent_awareness: 0.8,
        flip_strategy: FlipStrategy::Random,
    };

    // Player 2: unskilled (random)
    let unskilled = PlayerConfig {
        keep_threshold: 2,
        line_awareness: 0.0,
        opponent_awareness: 0.0,
        flip_strategy: FlipStrategy::Random,
    };

    config.players = vec![skilled, unskilled];
    let mut rng = rand::thread_rng();

    let num_games = 500u32;
    let mut skilled_wins = 0u32;
    let mut total_turns = 0u64;
    let mut skilled_score_sum = 0i64;
    let mut unskilled_score_sum = 0i64;

    for _ in 0..num_games {
        let result = play_game(&config, &mut rng);
        if result.winner == 0 {
            skilled_wins += 1;
        }
        total_turns += result.total_turns as u64;
        skilled_score_sum += result.player_scores[0] as i64;
        unskilled_score_sum += result.player_scores[1] as i64;
    }

    let skilled_win_pct = (skilled_wins as f64 / num_games as f64) * 100.0;
    let avg_turns = total_turns as f64 / num_games as f64;

    println!("\n=== Skilled vs Unskilled: {} games ===", num_games);
    println!("Skilled wins: {} ({:.1}%)", skilled_wins, skilled_win_pct);
    println!(
        "Avg score — Skilled: {:.1}, Unskilled: {:.1}",
        skilled_score_sum as f64 / num_games as f64,
        unskilled_score_sum as f64 / num_games as f64
    );
    println!("Avg turns per game: {:.1}", avg_turns);
    println!();

    // Skilled player should win more than 50% of the time
    assert!(
        skilled_wins > num_games / 2,
        "Skilled player should win majority: won {} of {} ({:.1}%)",
        skilled_wins,
        num_games,
        skilled_win_pct
    );
}

#[test]
fn test_simulation_runner() {
    let config = GameConfig::default();
    let progress = Arc::new(AtomicU32::new(0));
    let num_games = 1000;

    let summary = run_simulation(&config, num_games, "Test Run".to_string(), progress.clone(), false);

    println!("\n=== Simulation Runner: {} games ===", num_games);
    println!("Avg turns/round: {:.1}", summary.avg_turns_per_round);
    println!("Win rates: {:?}", summary.win_rates);
    println!("First mover advantage: {:.1}%", summary.first_mover_advantage);
    println!("Draw pile exhaustion: {:.1}%", summary.draw_pile_exhaustion_rate);
    println!("Effective deck usage: {:.1}%", summary.effective_deck_usage);
    println!("Round completion rate: {:.1}%", summary.round_completion_rate);
    println!("Went out first rate: {:.1}%", summary.went_out_first_rate);
    println!("Cleared all rate: {:.1}%", summary.cleared_all_rate);
    println!("Avg eliminations/round: {:.2}", summary.avg_eliminations_per_round);
    for (i, ps) in summary.player_summaries.iter().enumerate() {
        println!(
            "Player {}: avg_score={:.1} win={:.1}% elims={:.2} remaining={:.1}",
            i + 1, ps.avg_score, ps.win_rate, ps.avg_eliminations, ps.avg_cards_remaining
        );
    }
    println!();

    // Assertions
    assert_eq!(summary.num_games, num_games);
    assert_eq!(summary.player_summaries.len(), 4);
    assert!(summary.avg_turns_per_round > 0.0);
    assert!(summary.win_rates.iter().all(|&r| r > 0.0));
    assert!(summary.effective_deck_usage > 0.0);
    assert!(summary.round_completion_rate > 0.0);
    assert_eq!(summary.run_name, "Test Run");
    assert!(!summary.id.is_empty());
    assert!(!summary.timestamp.is_empty());

    // Progress should match
    assert_eq!(
        progress.load(std::sync::atomic::Ordering::Relaxed),
        num_games
    );
}

#[test]
fn test_simulation_performance() {
    let config = GameConfig::default();
    let progress = Arc::new(AtomicU32::new(0));
    let num_games = 10_000;

    let start = std::time::Instant::now();
    let summary = run_simulation(&config, num_games, "Perf Test".to_string(), progress, false);
    let elapsed = start.elapsed();

    println!(
        "\n=== Performance: {} games in {:.2}s ({:.0} games/sec) ===\n",
        num_games,
        elapsed.as_secs_f64(),
        num_games as f64 / elapsed.as_secs_f64()
    );

    assert_eq!(summary.num_games, num_games);
    // Should complete in under 30 seconds even in debug mode
    assert!(
        elapsed.as_secs() < 30,
        "10k games took {}s, expected < 30s",
        elapsed.as_secs()
    );
}

#[test]
fn test_custom_card_range() {
    // Use a smaller range: -3 to 7
    let mut config = GameConfig::default();
    config.deck = DeckConfig {
        neg_min: -3,
        pos_max: 7,
        card_quantities: vec![
            (-3, 10), (-2, 10), (-1, 15), (0, 20),
            (1, 15), (2, 15), (3, 15), (4, 15), (5, 15), (6, 15), (7, 15),
        ],
        wild_count: 15,
    };
    assert!(config.deck.validate(config.player_count).is_ok());

    let progress = Arc::new(AtomicU32::new(0));
    let summary = run_simulation(&config, 100, "Custom Range".to_string(), progress, false);

    assert_eq!(summary.num_games, 100);
    assert!(summary.avg_turns_per_round > 0.0);
    println!("\n=== Custom Range (-3 to 7): avg turns={:.1}, deck usage={:.1}% ===\n",
        summary.avg_turns_per_round, summary.effective_deck_usage);
}

#[test]
fn test_no_special_cards() {
    let mut config = GameConfig::default();
    config.deck.wild_count = 0;
    assert!(config.deck.validate(config.player_count).is_ok());

    let mut rng = rand::thread_rng();
    for _ in 0..20 {
        let result = play_game(&config, &mut rng);
        assert_eq!(result.round_results.len(), 4);
    }
}

#[test]
fn test_minimal_deck() {
    // Minimum viable deck for 2 players: 2*16 + 20 = 52 cards
    let mut config = GameConfig::default();
    config.player_count = 2;
    config.players.truncate(2);
    config.deck = DeckConfig {
        neg_min: -2,
        pos_max: 5,
        card_quantities: vec![
            (-2, 4), (-1, 8), (0, 8), (1, 6), (2, 6), (3, 6), (4, 6), (5, 6),
        ],
        wild_count: 4,
    };
    assert!(config.deck.validate(config.player_count).is_ok());

    let mut rng = rand::thread_rng();
    for _ in 0..20 {
        let result = play_game(&config, &mut rng);
        assert_eq!(result.round_results.len(), 4);
    }
}

#[test]
fn test_persistence_cycle() {
    // Run → Save → List → Get → Compare → Delete
    let config = GameConfig::default();
    let progress = Arc::new(AtomicU32::new(0));

    let summary_a = run_simulation(&config, 50, "Persistence Test A".to_string(), Arc::clone(&progress), false);
    // run_simulation auto-saves via commands, but we test store directly
    store::save_run(&summary_a).expect("save A");

    progress.store(0, std::sync::atomic::Ordering::Relaxed);
    let summary_b = run_simulation(&config, 50, "Persistence Test B".to_string(), progress, false);
    store::save_run(&summary_b).expect("save B");

    // List
    let runs = store::list_runs().expect("list");
    assert!(runs.len() >= 2);

    // Get
    let loaded = store::get_run(&summary_a.id).expect("get A");
    assert_eq!(loaded.run_name, "Persistence Test A");
    assert_eq!(loaded.num_games, 50);

    // Compare
    let comparison = compare::compare_runs(&summary_a, &summary_b);
    assert!(!comparison.diffs.is_empty());
    assert_eq!(comparison.run_a_name, "Persistence Test A");
    assert_eq!(comparison.run_b_name, "Persistence Test B");

    // Export
    let csv = store::export_run_csv(&summary_a.id).expect("export");
    assert!(csv.contains("Persistence Test A"));
    assert!(csv.contains("Avg Turns/Round"));

    // Delete
    store::delete_run(&summary_a.id).expect("delete A");
    store::delete_run(&summary_b.id).expect("delete B");

    // Verify deleted
    assert!(store::get_run(&summary_a.id).is_err());
}
