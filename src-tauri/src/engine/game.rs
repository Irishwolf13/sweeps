use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};

use super::card::{build_deck, Card};
use super::config::{AiArchetype, GameConfig, GameMode, ScoringMode, StartingOrder};
use super::grid::{EliminationType, PlayerGrid};
use super::strategy::{self, DrawSource, MethodicalState, TurnAction};

// ── Result types ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameResult {
    pub player_scores: Vec<i32>,
    pub round_results: Vec<RoundResult>,
    pub winner: usize,
    pub total_turns: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoundResult {
    pub round_number: u8,
    pub turns: u32,
    pub player_round_scores: Vec<i32>,
    pub went_out_first: Option<usize>,
    pub cleared_all: Vec<usize>,
    pub draw_pile_exhausted: bool,
    pub eliminations_per_player: Vec<u32>,
    pub cards_remaining_per_player: Vec<usize>,
    // Deck health tracking
    pub total_cards_drawn: u32,
    pub total_deck_size: u32,
    pub round_completed_naturally: bool,
    pub draw_pile_remaining: u32,
}

// ── Player state within a round ───────────────────────────────────────────

#[derive(Clone, Debug)]
struct PlayerState {
    grid: PlayerGrid,
    went_out_first: bool,
    cleared_all: bool,
    eliminations: u32,
}

// ── Round state ───────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct RoundState {
    draw_pile: Vec<Card>,
    discard_pile: Vec<Card>,
    players: Vec<PlayerState>,
    methodical_states: Vec<Option<MethodicalState>>,
    current_player: usize,
    turn_number: u32,
    round_ended: bool,
    trigger_player: Option<usize>,
    final_turns_given: usize,
    player_count: usize,
    total_cards_drawn: u32,
    total_deck_size: u32,
}

// ── Public API ────────────────────────────────────────────────────────────

/// Play a complete game. Rounds = player_count * round_multiplier.
pub fn play_game(config: &GameConfig, rng: &mut impl Rng) -> GameResult {
    let mut cumulative_scores = vec![0i32; config.player_count as usize];
    let mut round_results = Vec::new();
    let mut total_turns = 0u32;
    let total_rounds = config.total_rounds();

    for round_num in 0..total_rounds {
        let starting = determine_starting_player(config, round_num, &cumulative_scores);
        let result = play_round(config, round_num, starting, rng);
        total_turns += result.turns;
        for (i, &s) in result.player_round_scores.iter().enumerate() {
            cumulative_scores[i] += s;
        }
        round_results.push(result);
    }

    let winner = cumulative_scores
        .iter()
        .enumerate()
        .min_by_key(|(_, &s)| s)
        .map(|(i, _)| i)
        .unwrap_or(0);

    GameResult {
        player_scores: cumulative_scores,
        round_results,
        winner,
        total_turns,
    }
}

// ── Starting player logic ─────────────────────────────────────────────────

/// Determine which player starts a round based on config.
fn determine_starting_player(
    config: &GameConfig,
    round_number: u8,
    cumulative_scores: &[i32],
) -> usize {
    let player_count = config.player_count as usize;

    match config.starting_order {
        StartingOrder::RoundRobin => (round_number as usize) % player_count,
        StartingOrder::WorstScoreFirst => {
            if round_number == 0 || cumulative_scores.is_empty() {
                // Round 0: no scores yet, fall back to round-robin
                0
            } else {
                // Worst = highest score (lower is better in this game).
                // Ties broken by lowest player index.
                cumulative_scores
                    .iter()
                    .enumerate()
                    .max_by_key(|(i, &score)| (score, -((*i) as i32)))
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            }
        }
    }
}

// ── Round logic ───────────────────────────────────────────────────────────

fn play_round(config: &GameConfig, round_number: u8, starting_player: usize, rng: &mut impl Rng) -> RoundResult {
    let player_count = config.player_count as usize;

    // Build and shuffle deck
    let mut deck = build_deck(&config.deck);
    deck.shuffle(rng);

    // Deal 16 cards to each player
    let mut players = Vec::with_capacity(player_count);
    for i in 0..player_count {
        let hand: Vec<Card> = deck.drain(..16).collect();
        let grid = PlayerGrid::new(hand, &config.players[i].flip_strategy, rng);
        players.push(PlayerState {
            grid,
            went_out_first: false,
            cleared_all: false,
            eliminations: 0,
        });
    }

    let methodical_states: Vec<Option<MethodicalState>> = (0..player_count)
        .map(|i| match config.players[i].archetype {
            AiArchetype::Methodical => Some(MethodicalState::new()),
            _ => None,
        })
        .collect();

    // Remaining cards form draw pile, flip top for discard
    let mut discard_pile = Vec::new();
    if let Some(top) = deck.pop() {
        discard_pile.push(top);
    }

    let total_deck_size = deck.len() as u32;
    let mut state = RoundState {
        draw_pile: deck,
        discard_pile,
        players,
        methodical_states,
        current_player: starting_player,
        turn_number: 0,
        round_ended: false,
        trigger_player: None,
        final_turns_given: 0,
        player_count,
        total_cards_drawn: 0,
        total_deck_size,
    };

    // Main turn loop
    while !is_round_over(&state, config) {
        state.turn_number += 1;

        if state.turn_number > config.max_turns_per_round {
            break;
        }

        play_turn(config, &mut state, rng);

        // Advance to next player
        state.current_player = (state.current_player + 1) % state.player_count;
    }

    // Score the round
    let scores = score_round(config, &state);
    let draw_pile_exhausted =
        state.draw_pile.is_empty() && state.discard_pile.len() <= 1;

    RoundResult {
        round_number,
        turns: state.turn_number,
        player_round_scores: scores,
        went_out_first: state.trigger_player,
        cleared_all: state
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.cleared_all)
            .map(|(i, _)| i)
            .collect(),
        draw_pile_exhausted,
        eliminations_per_player: state.players.iter().map(|p| p.eliminations).collect(),
        cards_remaining_per_player: state
            .players
            .iter()
            .map(|p| p.grid.remaining_card_count())
            .collect(),
        total_cards_drawn: state.total_cards_drawn,
        total_deck_size: state.total_deck_size,
        round_completed_naturally: state.trigger_player.is_some(),
        draw_pile_remaining: state.draw_pile.len() as u32,
    }
}

fn is_round_over(state: &RoundState, _config: &GameConfig) -> bool {
    if !state.round_ended {
        return false;
    }
    // After trigger, each other player gets one more turn
    state.final_turns_given >= state.player_count - 1
}

// ── Turn logic ────────────────────────────────────────────────────────────

fn play_turn(config: &GameConfig, state: &mut RoundState, rng: &mut impl Rng) {
    let player_idx = state.current_player;
    let player_config = &config.players[player_idx];

    // Skip players who have already cleared all cards
    if state.players[player_idx].grid.remaining_card_count() == 0 {
        if state.round_ended {
            state.final_turns_given += 1;
        }
        return;
    }

    // Track if this is a final turn
    if state.round_ended {
        state.final_turns_given += 1;
    }

    // 1. Choose draw source
    let ctx = config.elimination_context();
    let discard_top = state.discard_pile.last().cloned();
    let source = strategy::choose_draw_source(player_config, discard_top.as_ref(), &state.players[player_idx].grid, &ctx, &mut state.methodical_states[player_idx], rng);

    // 2. Draw a card
    let drawn = match draw_card(state, source, rng) {
        Some(card) => card,
        None => return, // No cards anywhere, skip turn
    };

    // 3. Handle the drawn card
    handle_normal_draw(config, state, player_idx, drawn, rng);

    // 4. Check for eliminations (may cascade)
    check_and_apply_eliminations(config, state, player_idx, rng);

    // 5. Check round end trigger
    check_round_end_trigger(state, player_idx, &config.game_mode);
}

fn draw_card(state: &mut RoundState, source: DrawSource, rng: &mut impl Rng) -> Option<Card> {
    match source {
        DrawSource::DiscardPile => {
            if let Some(card) = state.discard_pile.pop() {
                Some(card)
            } else {
                // Fall back to draw pile
                draw_from_pile(state, rng)
            }
        }
        DrawSource::DrawPile => draw_from_pile(state, rng),
    }
}

fn draw_from_pile(state: &mut RoundState, rng: &mut impl Rng) -> Option<Card> {
    if let Some(card) = state.draw_pile.pop() {
        state.total_cards_drawn += 1;
        return Some(card);
    }

    // Draw pile empty — reshuffle discard pile
    reshuffle_discard(state, rng);

    let card = state.draw_pile.pop();
    if card.is_some() {
        state.total_cards_drawn += 1;
    }
    card
}

fn reshuffle_discard(state: &mut RoundState, rng: &mut impl Rng) {
    if state.discard_pile.len() <= 1 {
        return;
    }
    let top = state.discard_pile.pop();
    state.draw_pile.extend(state.discard_pile.drain(..));
    state.draw_pile.shuffle(rng);
    if let Some(t) = top {
        state.discard_pile.push(t);
    }
}

fn handle_normal_draw(
    config: &GameConfig,
    state: &mut RoundState,
    player_idx: usize,
    drawn: Card,
    rng: &mut impl Rng,
) {
    let player_config = &config.players[player_idx];
    let ctx = config.elimination_context();
    let action = strategy::choose_action(player_config, &drawn, &state.players[player_idx].grid, &ctx, &mut state.methodical_states[player_idx], rng);

    match action {
        TurnAction::ReplaceCard { row, col } => {
            if let Some(old_card) = state.players[player_idx].grid.replace_card(row, col, drawn) {
                state.discard_pile.push(old_card);
            }
        }
        TurnAction::DiscardAndFlip { row, col } => {
            state.discard_pile.push(drawn);
            state.players[player_idx].grid.flip_card(row, col);
        }
    }
}

// ── Elimination ───────────────────────────────────────────────────────────

fn check_and_apply_eliminations(
    config: &GameConfig,
    state: &mut RoundState,
    player_idx: usize,
    rng: &mut impl Rng,
) {
    loop {
        let ctx = config.elimination_context();
        let eliminations = state.players[player_idx].grid.find_eliminations(
            config.allow_matching_elimination,
            config.allow_diagonal_elimination,
            &ctx,
        );

        if eliminations.is_empty() {
            break;
        }

        // Merge ALL simultaneous eliminations (e.g., row + column through same Wild)
        let mut all_positions: Vec<(usize, usize)> = Vec::new();
        let mut has_diagonal = false;
        let mut diagonal_kind = None;
        for elim in &eliminations {
            for pos in &elim.positions {
                if !all_positions.contains(pos) {
                    all_positions.push(*pos);
                }
            }
            if matches!(elim.kind, EliminationType::MainDiagonal | EliminationType::AntiDiagonal) {
                has_diagonal = true;
                diagonal_kind = Some(elim.kind.clone());
            }
        }

        let removed = state.players[player_idx].grid.eliminate(&all_positions);
        state.players[player_idx].eliminations += eliminations.len() as u32;

        // Player chooses which card goes to discard (considering next player)
        if !removed.is_empty() {
            let next_player = (player_idx + 1) % state.player_count;
            let next_grid = Some(&state.players[next_player].grid);
            let discard_idx = strategy::choose_discard_with_opponent(
                &config.players[player_idx], &removed, next_grid,
                &ctx, rng,
            );
            state.discard_pile.push(removed[discard_idx].clone());
        }

        // Reshape grid after diagonal elimination
        if has_diagonal {
            if let Some(ref kind) = diagonal_kind {
                let direction = strategy::choose_slide_direction(&config.players[player_idx], &state.players[player_idx].grid, kind, &ctx, rng);
                state.players[player_idx]
                    .grid
                    .reshape_after_diagonal(kind, direction);
            }
        }

        // Clean up empty rows
        state.players[player_idx].grid.cleanup();

        // Invalidate methodical targets after grid dimensions change
        state.methodical_states[player_idx].as_mut().map(|s| s.invalidate_targets());

        // Check if player cleared all cards
        if state.players[player_idx].grid.remaining_card_count() == 0 {
            state.players[player_idx].cleared_all = true;
            break;
        }
    }
}

// ── Round end detection ───────────────────────────────────────────────────

fn check_round_end_trigger(state: &mut RoundState, player_idx: usize, game_mode: &GameMode) {
    if state.round_ended {
        return;
    }

    let grid = &state.players[player_idx].grid;
    let remaining = grid.remaining_card_count();

    let triggered = match game_mode {
        GameMode::Numbers => (remaining <= 4 && grid.all_face_up()) || remaining == 0,
        GameMode::Shapes => remaining == 0,
    };
    if triggered {
        state.round_ended = true;
        state.trigger_player = Some(player_idx);
        state.players[player_idx].went_out_first = true;
    }
}

// ── Scoring ───────────────────────────────────────────────────────────────

fn score_round(config: &GameConfig, state: &RoundState) -> Vec<i32> {
    state
        .players
        .iter()
        .map(|p| {
            let mut score = match config.scoring_mode {
                ScoringMode::Basic => p.grid.remaining_card_count() as i32,
                ScoringMode::Expert => {
                    // Sum of absolute values of remaining cards
                    p.grid
                        .occupied_positions()
                        .iter()
                        .map(|&(r, c)| {
                            p.grid
                                .get(r, c)
                                .map(|gc| gc.card.score_value())
                                .unwrap_or(0)
                        })
                        .sum::<i32>()
                }
            };

            // Going out first: bonus of -2 (Numbers mode only)
            if config.game_mode == GameMode::Numbers && p.went_out_first {
                score -= 2;
            }

            score
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::config::GameConfig;

    #[test]
    fn test_play_single_game() {
        let config = GameConfig::default();
        let mut rng = rand::thread_rng();
        let result = play_game(&config, &mut rng);

        assert_eq!(result.player_scores.len(), 4);
        assert_eq!(result.round_results.len(), 4);
        assert!(result.total_turns > 0);
        assert!(result.winner < 4);
    }

    #[test]
    fn test_round_diagnostic() {
        let config = GameConfig::default();
        let mut rng = rand::thread_rng();

        for game in 0..20 {
            let result = play_game(&config, &mut rng);
            for (r, round) in result.round_results.iter().enumerate() {
                println!("Game {} Round {}: turns={}, trigger={:?}",
                    game+1, r+1, round.turns, round.went_out_first);
                for i in 0..4 {
                    println!("  P{}: remaining={}, elims={}",
                        i+1,
                        round.cards_remaining_per_player[i],
                        round.eliminations_per_player[i]);
                }
            }
            println!();
        }
    }

    #[test]
    fn test_score_diagnostic() {
        let config = GameConfig::default();
        let mut rng = rand::thread_rng();

        let mut max_score = i32::MIN;
        for game in 0..100 {
            let result = play_game(&config, &mut rng);
            for (i, &score) in result.player_scores.iter().enumerate() {
                if score > max_score {
                    max_score = score;
                }
                if score > 64 {
                    println!("Game {}: Player {} total score = {}", game, i+1, score);
                    for (r, round) in result.round_results.iter().enumerate() {
                        println!("  Round {}: score={}, remaining={}",
                            r+1,
                            round.player_round_scores[i],
                            round.cards_remaining_per_player[i]);
                    }
                }
            }
        }
        println!("\nMax total score across 100 games: {}", max_score);
    }

    #[test]
    fn test_play_10_games_no_panics() {
        let config = GameConfig::default();
        let mut rng = rand::thread_rng();

        for _ in 0..10 {
            let result = play_game(&config, &mut rng);
            assert_eq!(result.round_results.len(), 4);
            assert!(result.total_turns > 0);
        }
    }

    #[test]
    fn test_scoring_basic_mode() {
        let config = GameConfig::default();
        let mut rng = rand::thread_rng();
        let result = play_game(&config, &mut rng);

        // In basic mode, scores should be reasonable (not astronomically high)
        for &score in &result.player_scores {
            // 4 rounds, max 16 cards per round = 64 theoretical max
            // Minus bonuses
            assert!(score < 100, "Score {} seems unreasonably high", score);
        }
    }

    #[test]
    fn test_expert_scoring() {
        let mut config = GameConfig::default();
        config.scoring_mode = ScoringMode::Expert;
        let mut rng = rand::thread_rng();
        let result = play_game(&config, &mut rng);

        assert_eq!(result.round_results.len(), 4);
        assert!(result.total_turns > 0);
    }

    #[test]
    fn test_two_players() {
        let mut config = GameConfig::default();
        config.player_count = 2;
        config.players.truncate(2);
        let mut rng = rand::thread_rng();
        let result = play_game(&config, &mut rng);

        assert_eq!(result.player_scores.len(), 2);
        assert!(result.winner < 2);
    }

    #[test]
    fn test_no_diagonal_no_matching() {
        let mut config = GameConfig::default();
        config.allow_diagonal_elimination = false;
        config.allow_matching_elimination = false;
        let mut rng = rand::thread_rng();
        let result = play_game(&config, &mut rng);

        assert_eq!(result.round_results.len(), 4);
    }

    #[test]
    fn test_determine_starting_player_round_robin() {
        let config = GameConfig::default();
        assert_eq!(determine_starting_player(&config, 0, &[]), 0);
        assert_eq!(determine_starting_player(&config, 1, &[0, 0, 0, 0]), 1);
        assert_eq!(determine_starting_player(&config, 2, &[0, 0, 0, 0]), 2);
    }

    #[test]
    fn test_determine_starting_player_worst_first() {
        let mut config = GameConfig::default();
        config.starting_order = StartingOrder::WorstScoreFirst;

        // Round 0: always player 0 (no scores)
        assert_eq!(determine_starting_player(&config, 0, &[]), 0);

        // Player 2 has highest (worst) score
        assert_eq!(determine_starting_player(&config, 1, &[5, 3, 10, 7]), 2);

        // Tie: players 0 and 3 both have 8 — lowest index wins
        assert_eq!(determine_starting_player(&config, 2, &[8, 3, 5, 8]), 0);
    }

    #[test]
    fn test_play_game_worst_score_first_no_panic() {
        let mut config = GameConfig::default();
        config.starting_order = StartingOrder::WorstScoreFirst;
        let mut rng = rand::thread_rng();
        let result = play_game(&config, &mut rng);
        assert_eq!(result.round_results.len(), 4);
        assert!(result.total_turns > 0);
    }

    #[test]
    fn test_shapes_game_runs_to_completion() {
        use crate::engine::config::{DeckConfig, GameMode};
        let mut config = GameConfig::default();
        config.game_mode = GameMode::Shapes;
        config.deck = DeckConfig::shapes_scaled(4);
        config.allow_matching_elimination = true;
        config.allow_cancellation = true;
        config.shade_matters = true;
        config.allow_diagonal_elimination = false;
        let mut rng = rand::thread_rng();
        let result = play_game(&config, &mut rng);
        assert_eq!(result.player_scores.len(), 4);
        assert!(result.total_turns > 0);
        for round in &result.round_results {
            for &score in &round.player_round_scores {
                assert!(score >= 0, "Shapes scores should never be negative (no bonus)");
            }
        }
    }

    #[test]
    fn test_shapes_beginner_no_wilds() {
        use crate::engine::config::{DeckConfig, GameMode};
        let mut config = GameConfig::default();
        config.game_mode = GameMode::Shapes;
        config.deck = DeckConfig::shapes_scaled(4);
        config.shade_matters = false;
        config.allow_cancellation = false;
        config.allow_diagonal_elimination = false;
        if let DeckConfig::Shapes { ref mut wild_count, ref mut wild_shaded_count, ref mut wild_unshaded_count, .. } = config.deck {
            *wild_count = 0; *wild_shaded_count = 0; *wild_unshaded_count = 0;
        }
        let mut rng = rand::thread_rng();
        let result = play_game(&config, &mut rng);
        assert!(result.total_turns > 0);
    }

    #[test]
    fn test_shapes_100_games_all_tiers() {
        use crate::engine::config::{DeckConfig, GameMode};
        let mut rng = rand::thread_rng();
        let tiers: Vec<(&str, bool, bool, bool)> = vec![
            ("beginner", false, false, false),
            ("intermediate", true, false, false),
            ("advanced", true, true, false),
            ("expert", true, true, true),
        ];
        for (tier_name, shade_matters, allow_cancel, allow_diag) in &tiers {
            let mut config = GameConfig::default();
            config.game_mode = GameMode::Shapes;
            config.deck = DeckConfig::shapes_scaled(4);
            config.shade_matters = *shade_matters;
            config.allow_cancellation = *allow_cancel;
            config.allow_diagonal_elimination = *allow_diag;
            config.allow_matching_elimination = true;
            if !shade_matters {
                if let DeckConfig::Shapes { ref mut wild_count, ref mut wild_shaded_count, ref mut wild_unshaded_count, .. } = config.deck {
                    *wild_count = 0; *wild_shaded_count = 0; *wild_unshaded_count = 0;
                }
            }
            for game_num in 0..100 {
                let result = play_game(&config, &mut rng);
                assert_eq!(result.player_scores.len(), 4);
                assert!(result.total_turns > 0,
                    "Tier {}: game {} had 0 turns", tier_name, game_num);
                for &score in &result.player_scores {
                    assert!(score >= 0,
                        "Tier {}: game {} had negative score {}", tier_name, game_num, score);
                }
            }
        }
    }
}
