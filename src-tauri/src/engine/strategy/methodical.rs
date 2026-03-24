use rand::Rng;

use super::line_scoring::{score_all_lines, card_fits_line, best_placement, LineStatus};
use super::{DrawSource, TurnAction, MethodicalState, Phase, should_play_smart};
use super::super::card::Card;
use super::super::config::PlayerConfig;
use super::super::grid::PlayerGrid;

/// Compute the face-down ratio threshold for transitioning out of Scout.
/// High skill = shorter scouting (threshold ~0.5), low skill = longer scouting (threshold ~0.75).
fn scout_threshold(skill: f64) -> f64 {
    0.75 - skill * 0.25
}

/// Select the 1-2 best target lines for the Build phase.
fn select_targets(lines: &[(LineStatus, f64)]) -> Vec<usize> {
    let mut indexed: Vec<(usize, f64)> = lines.iter().enumerate()
        .filter(|(_, (status, score))| *score > 5.0 && status.gap_achievable)
        .map(|(i, (_, score))| (i, *score))
        .collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    indexed.iter().take(2).map(|(i, _)| *i).collect()
}

/// Check if any target line is now hopeless and needs re-evaluation.
fn targets_still_valid(state: &MethodicalState, lines: &[(LineStatus, f64)]) -> bool {
    state.target_lines.iter().all(|&idx| {
        idx < lines.len() && lines[idx].0.gap_achievable && lines[idx].1 > 5.0
    })
}

pub fn choose_draw_source(
    config: &PlayerConfig,
    discard_top: Option<&Card>,
    grid: &PlayerGrid,
    neg_min: i32,
    pos_max: i32,
    state: &mut MethodicalState,
    rng: &mut impl Rng,
) -> DrawSource {
    let card = match discard_top {
        Some(c) => c,
        None => return DrawSource::DrawPile,
    };

    if !should_play_smart(config.skill, rng) {
        return if rng.gen_bool(0.5) { DrawSource::DiscardPile } else { DrawSource::DrawPile };
    }

    // Always take Wild
    if matches!(card, Card::Wild) {
        return DrawSource::DiscardPile;
    }

    let card_value = match card { Card::Number(v) => *v, Card::Wild => 0 };
    let lines = score_all_lines(grid, neg_min, pos_max);

    match state.phase {
        Phase::Scout => {
            // Only take Wilds (caught above) and 0s during scouting
            if card_value == 0 { DrawSource::DiscardPile } else { DrawSource::DrawPile }
        }
        Phase::Build => {
            // Take if it helps a target line
            for &idx in &state.target_lines {
                if idx < lines.len() {
                    if card_fits_line(card_value, &lines[idx].0, neg_min, pos_max) >= 50.0 {
                        return DrawSource::DiscardPile;
                    }
                }
            }
            DrawSource::DrawPile
        }
        Phase::Close => {
            // Only take if it completes a target line
            for &idx in &state.target_lines {
                if idx < lines.len() {
                    if card_fits_line(card_value, &lines[idx].0, neg_min, pos_max) >= 100.0 {
                        return DrawSource::DiscardPile;
                    }
                }
            }
            // Also take if it completes ANY line (opportunistic in Close)
            for (line, _) in &lines {
                if card_fits_line(card_value, line, neg_min, pos_max) >= 100.0 {
                    return DrawSource::DiscardPile;
                }
            }
            DrawSource::DrawPile
        }
    }
}

pub fn choose_action(
    config: &PlayerConfig,
    drawn_card: &Card,
    grid: &PlayerGrid,
    neg_min: i32,
    pos_max: i32,
    state: &mut MethodicalState,
    rng: &mut impl Rng,
) -> TurnAction {
    let face_down = grid.face_down_positions();
    let total_cards = grid.remaining_card_count();
    state.turns_in_phase += 1;

    if !should_play_smart(config.skill, rng) {
        return super::opportunist::fallback_action(drawn_card, grid, rng);
    }

    let lines = score_all_lines(grid, neg_min, pos_max);

    // Phase transitions
    update_phase(state, &face_down, total_cards, &lines, config.skill);

    match state.phase {
        Phase::Scout => {
            // Only keep Wilds and 0s
            let card_value = match drawn_card { Card::Number(v) => *v, Card::Wild => 0 };
            let is_wild = matches!(drawn_card, Card::Wild);

            if is_wild || card_value == 0 {
                // Place in a face-down slot in the most promising line
                if !face_down.is_empty() {
                    let target = best_scout_flip(&face_down, &lines);
                    return TurnAction::ReplaceCard { row: target.0, col: target.1 };
                }
            }
            // Discard and flip a card in a line with most face-up neighbors
            if !face_down.is_empty() {
                let target = best_scout_flip(&face_down, &lines);
                return TurnAction::DiscardAndFlip { row: target.0, col: target.1 };
            }
            // Fallback: all face-up
            let (pos, _) = best_placement(drawn_card, grid, neg_min, pos_max);
            TurnAction::ReplaceCard { row: pos.0, col: pos.1 }
        }
        Phase::Build => {
            // Serve target lines
            let (pos, score) = best_placement(drawn_card, grid, neg_min, pos_max);

            // Check if placement helps a target line specifically
            let card_value = match drawn_card { Card::Number(v) => *v, Card::Wild => 0 };
            let mut helps_target = false;
            for &idx in &state.target_lines {
                if idx < lines.len() {
                    let fit = card_fits_line(card_value, &lines[idx].0, neg_min, pos_max);
                    if fit >= 30.0 { helps_target = true; break; }
                }
            }

            if helps_target && score >= 20.0 {
                return TurnAction::ReplaceCard { row: pos.0, col: pos.1 };
            }

            // Doesn't help targets — discard and flip in target line
            if !face_down.is_empty() {
                let target = best_target_flip(&face_down, &lines, &state.target_lines);
                return TurnAction::DiscardAndFlip { row: target.0, col: target.1 };
            }

            // All face-up, use best placement regardless
            TurnAction::ReplaceCard { row: pos.0, col: pos.1 }
        }
        Phase::Close => {
            // Only place cards that complete a line
            let card_value = match drawn_card { Card::Number(v) => *v, Card::Wild => 0 };

            // Check target lines first
            for &idx in &state.target_lines {
                if idx < lines.len() {
                    if card_fits_line(card_value, &lines[idx].0, neg_min, pos_max) >= 100.0 {
                        // Find the face-down position in this line
                        for &(r, c) in &lines[idx].0.positions {
                            if let Some(gc) = grid.get(r, c) {
                                if !gc.face_up {
                                    return TurnAction::ReplaceCard { row: r, col: c };
                                }
                            }
                        }
                    }
                }
            }

            // Check ALL lines for completion
            for (line, _) in &lines {
                if card_fits_line(card_value, line, neg_min, pos_max) >= 100.0 {
                    for &(r, c) in &line.positions {
                        if let Some(gc) = grid.get(r, c) {
                            if !gc.face_up {
                                return TurnAction::ReplaceCard { row: r, col: c };
                            }
                        }
                    }
                }
            }

            // Doesn't complete anything — discard and flip
            if !face_down.is_empty() {
                let target = best_target_flip(&face_down, &lines, &state.target_lines);
                return TurnAction::DiscardAndFlip { row: target.0, col: target.1 };
            }

            // All face-up, must place somewhere
            let (pos, _) = best_placement(drawn_card, grid, neg_min, pos_max);
            TurnAction::ReplaceCard { row: pos.0, col: pos.1 }
        }
    }
}

fn update_phase(
    state: &mut MethodicalState,
    face_down: &[(usize, usize)],
    total_cards: usize,
    lines: &[(LineStatus, f64)],
    skill: f64,
) {
    let face_down_ratio = if total_cards == 0 { 0.0 } else { face_down.len() as f64 / total_cards as f64 };

    match state.phase {
        Phase::Scout => {
            if face_down_ratio <= scout_threshold(skill) {
                state.phase = Phase::Build;
                state.turns_in_phase = 0;
                state.target_lines = select_targets(lines);
            }
        }
        Phase::Build => {
            // Re-evaluate targets if they became hopeless
            if !targets_still_valid(state, lines) {
                state.target_lines = select_targets(lines);
            }
            // Transition to Close if any target is 1 card away
            for &idx in &state.target_lines {
                if idx < lines.len() && lines[idx].0.face_down_count == 1 && lines[idx].1 >= 70.0 {
                    state.phase = Phase::Close;
                    state.turns_in_phase = 0;
                    return;
                }
            }
        }
        Phase::Close => {
            // If no target is close anymore, go back to Build
            let any_close = state.target_lines.iter().any(|&idx| {
                idx < lines.len() && lines[idx].0.face_down_count <= 1 && lines[idx].1 >= 70.0
            });
            if !any_close {
                state.phase = Phase::Build;
                state.turns_in_phase = 0;
                state.target_lines = select_targets(lines);
            }
        }
    }
}

/// Best face-down card to flip during Scout: prefer cards sharing lines with face-up cards.
fn best_scout_flip(
    face_down: &[(usize, usize)],
    lines: &[(LineStatus, f64)],
) -> (usize, usize) {
    let mut best_pos = face_down[0];
    let mut best_score = 0.0f64;

    for &(r, c) in face_down {
        let mut score = 0.0f64;
        for (line, _) in lines {
            if line.positions.contains(&(r, c)) {
                // Prefer lines with more face-up cards (concentrate info gathering)
                score += line.face_up_count as f64;
            }
        }
        if score > best_score {
            best_score = score;
            best_pos = (r, c);
        }
    }
    best_pos
}

/// Best face-down card to flip during Build/Close: prefer cards in target lines.
fn best_target_flip(
    face_down: &[(usize, usize)],
    lines: &[(LineStatus, f64)],
    target_lines: &[usize],
) -> (usize, usize) {
    let mut best_pos = face_down[0];
    let mut best_score = f64::NEG_INFINITY;

    for &(r, c) in face_down {
        let mut score = 0.0f64;
        for &idx in target_lines {
            if idx < lines.len() && lines[idx].0.positions.contains(&(r, c)) {
                score += lines[idx].1 * 2.0; // Double weight for target lines
            }
        }
        // Also consider non-target lines
        for (line, line_score) in lines {
            if line.positions.contains(&(r, c)) {
                score += line_score;
            }
        }
        if score > best_score {
            best_score = score;
            best_pos = (r, c);
        }
    }
    best_pos
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::config::AiArchetype;

    fn expert_methodical() -> PlayerConfig {
        PlayerConfig {
            archetype: AiArchetype::Methodical,
            skill: 1.0,
            flip_strategy: Default::default(),
        }
    }

    fn make_mostly_face_down() -> PlayerGrid {
        // 4x4 grid, all face-down except (0,0) and (0,1)
        let cards: Vec<Card> = vec![
            Card::Number(-3), Card::Number(3), Card::Number(2), Card::Number(-2),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
        ];
        let mut grid = PlayerGrid::new_no_flips(cards);
        grid.flip_card(0, 0);
        grid.flip_card(0, 1);
        grid
    }

    #[test]
    fn test_starts_in_scout_phase() {
        let state = MethodicalState::new();
        assert!(matches!(state.phase, Phase::Scout));
    }

    #[test]
    fn test_scout_only_keeps_wilds_and_zeros() {
        let config = expert_methodical();
        let grid = make_mostly_face_down();
        let mut state = MethodicalState::new();
        let mut rng = rand::thread_rng();

        // Non-zero, non-wild: should discard and flip
        let action = choose_action(&config, &Card::Number(5), &grid, -5, 8, &mut state, &mut rng);
        assert!(matches!(action, TurnAction::DiscardAndFlip { .. }));

        // Zero: should replace (keep it)
        let action = choose_action(&config, &Card::Number(0), &grid, -5, 8, &mut state, &mut rng);
        assert!(matches!(action, TurnAction::ReplaceCard { .. }));

        // Wild: should replace (keep it)
        let action = choose_action(&config, &Card::Wild, &grid, -5, 8, &mut state, &mut rng);
        assert!(matches!(action, TurnAction::ReplaceCard { .. }));
    }

    #[test]
    fn test_scout_transitions_to_build() {
        let config = expert_methodical();
        let mut state = MethodicalState::new();

        // Grid with most cards face-up (low face_down_ratio)
        let cards: Vec<Card> = (0..16).map(|_| Card::Number(1)).collect();
        let mut grid = PlayerGrid::new_no_flips(cards);
        for r in 0..4 { for c in 0..3 { grid.flip_card(r, c); } }
        // 4 face-down out of 16 = 0.25 ratio, below threshold

        let mut rng = rand::thread_rng();
        let _ = choose_action(&config, &Card::Number(1), &grid, -5, 8, &mut state, &mut rng);

        assert!(matches!(state.phase, Phase::Build), "Should transition to Build when enough info gathered");
    }

    #[test]
    fn test_close_only_places_completing_cards() {
        let config = expert_methodical();
        let mut state = MethodicalState { phase: Phase::Close, target_lines: vec![0], turns_in_phase: 0 };

        // Row 0: -3, 1, 2, face_down → needs 0 to complete
        let cards: Vec<Card> = vec![
            Card::Number(-3), Card::Number(1), Card::Number(2), Card::Number(7),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
        ];
        let mut grid = PlayerGrid::new_no_flips(cards);
        grid.flip_card(0, 0); grid.flip_card(0, 1); grid.flip_card(0, 2);
        for r in 1..4 { for c in 0..4 { grid.flip_card(r, c); } }

        let mut rng = rand::thread_rng();

        // Non-completing card: should discard
        let action = choose_action(&config, &Card::Number(5), &grid, -5, 8, &mut state, &mut rng);
        assert!(matches!(action, TurnAction::DiscardAndFlip { .. }));

        // Completing card: should place at (0,3)
        let action = choose_action(&config, &Card::Number(0), &grid, -5, 8, &mut state, &mut rng);
        match action {
            TurnAction::ReplaceCard { row, col } => assert_eq!((row, col), (0, 3)),
            _ => panic!("Should place completing card"),
        }
    }

    #[test]
    fn test_invalidate_targets_resets_to_build() {
        let mut state = MethodicalState {
            phase: Phase::Close,
            target_lines: vec![0, 1],
            turns_in_phase: 5,
        };
        state.invalidate_targets();
        assert!(matches!(state.phase, Phase::Build));
        assert!(state.target_lines.is_empty());
        assert_eq!(state.turns_in_phase, 0);
    }
}
