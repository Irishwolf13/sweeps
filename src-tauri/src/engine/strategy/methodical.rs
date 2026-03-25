use rand::Rng;

use super::line_scoring::{score_all_lines, card_fits_line, best_placement, best_flip_target};
use super::{DrawSource, TurnAction, MethodicalState, should_play_smart};
use super::super::card::Card;
use super::super::config::PlayerConfig;
use super::super::grid::PlayerGrid;


pub fn choose_draw_source(
    config: &PlayerConfig,
    discard_top: Option<&Card>,
    grid: &PlayerGrid,
    neg_min: i32,
    pos_max: i32,
    state: &mut MethodicalState,
    rng: &mut impl Rng,
) -> DrawSource {
    let _ = state; // Stateless — state param kept for API compatibility

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

    // Take if it completes any line
    for (line, _) in &lines {
        if card_fits_line(card_value, line, neg_min, pos_max) >= 100.0 {
            return DrawSource::DiscardPile;
        }
    }

    // Always take a 0
    if card_value == 0 {
        return DrawSource::DiscardPile;
    }

    // Take if it meaningfully helps any line
    for (line, _) in &lines {
        if card_fits_line(card_value, line, neg_min, pos_max) >= 40.0 {
            return DrawSource::DiscardPile;
        }
    }

    DrawSource::DrawPile
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
    let _ = state; // Stateless — state param kept for API compatibility

    let face_down = grid.face_down_positions();

    if !should_play_smart(config.skill, rng) {
        return super::opportunist::fallback_action(drawn_card, grid, rng);
    }

    let card_value = match drawn_card { Card::Number(v) => *v, Card::Wild => 0 };
    let lines = score_all_lines(grid, neg_min, pos_max);

    // Priority 1: Complete a line if possible
    // Find the completable line with the highest score
    let mut best_completion: Option<((usize, usize), f64)> = None;
    for (line, line_score) in &lines {
        if card_fits_line(card_value, line, neg_min, pos_max) >= 100.0 {
            // Find the face-down slot in this line
            for &(r, c) in &line.positions {
                if let Some(gc) = grid.get(r, c) {
                    if !gc.face_up {
                        let is_better = best_completion.map_or(true, |(_, best_s)| *line_score > best_s);
                        if is_better {
                            best_completion = Some(((r, c), *line_score));
                        }
                    }
                }
            }
        }
    }
    if let Some(((r, c), _)) = best_completion {
        return TurnAction::ReplaceCard { row: r, col: c };
    }

    // Priority 2: Place if best_placement finds a good spot (threshold 20)
    let (pos, score) = best_placement(drawn_card, grid, neg_min, pos_max);
    if score >= 20.0 {
        return TurnAction::ReplaceCard { row: pos.0, col: pos.1 };
    }

    // Priority 3: Discard and flip the best face-down card
    if !face_down.is_empty() {
        let target = best_flip_target(&face_down, &lines);
        return TurnAction::DiscardAndFlip { row: target.0, col: target.1 };
    }

    // Priority 4: All face-up, place at best spot regardless of score
    TurnAction::ReplaceCard { row: pos.0, col: pos.1 }
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

    fn make_grid_one_away() -> PlayerGrid {
        // Row 0: -3, 1, 2, face_down → needs 0 to sum to zero
        let cards: Vec<Card> = vec![
            Card::Number(-3), Card::Number(1), Card::Number(2), Card::Number(7),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
        ];
        let mut grid = PlayerGrid::new_no_flips(cards);
        grid.flip_card(0, 0); grid.flip_card(0, 1); grid.flip_card(0, 2);
        for r in 1..4 { for c in 0..4 { grid.flip_card(r, c); } }
        grid
    }

    fn make_grid_all_face_up(values: &[i32]) -> PlayerGrid {
        let cards: Vec<Card> = values.iter().map(|&v| Card::Number(v)).collect();
        let mut grid = PlayerGrid::new_no_flips(cards);
        for r in 0..4 { for c in 0..4 { grid.flip_card(r, c); } }
        grid
    }

    // ── Draw source tests ──────────────────────────────────────────────

    #[test]
    fn test_draw_takes_wild_from_discard() {
        let config = expert_methodical();
        let grid = make_grid_all_face_up(&[1,2,3,4, 5,6,7,8, 1,2,3,4, 5,6,7,8]);
        let mut state = MethodicalState::new();
        let mut rng = rand::thread_rng();
        for _ in 0..20 {
            let result = choose_draw_source(&config, Some(&Card::Wild), &grid, -5, 8, &mut state, &mut rng);
            assert_eq!(result, DrawSource::DiscardPile, "Should always take Wild");
        }
    }

    #[test]
    fn test_draw_takes_completing_card() {
        let config = expert_methodical();
        let grid = make_grid_one_away(); // Row 0 needs a 0
        let mut state = MethodicalState::new();
        let mut rng = rand::thread_rng();
        let result = choose_draw_source(&config, Some(&Card::Number(0)), &grid, -5, 8, &mut state, &mut rng);
        assert_eq!(result, DrawSource::DiscardPile, "Should take card that completes a line");
    }

    #[test]
    fn test_draw_takes_zero() {
        let config = expert_methodical();
        // Grid where 0 doesn't complete anything but is still useful
        let grid = make_grid_all_face_up(&[1,2,3,4, 5,6,7,8, 1,2,3,4, 5,6,7,8]);
        let mut state = MethodicalState::new();
        let mut rng = rand::thread_rng();
        let result = choose_draw_source(&config, Some(&Card::Number(0)), &grid, -5, 8, &mut state, &mut rng);
        assert_eq!(result, DrawSource::DiscardPile, "Should always take a 0");
    }

    #[test]
    fn test_draw_takes_helpful_card() {
        let config = expert_methodical();
        // Row 0: -3, 1, face_down, face_down → gap=2, 2 unknowns
        // Placing -1 → new_gap=3, 1 remaining unknown, range [-5,8] → viable, score ~48
        let cards: Vec<Card> = vec![
            Card::Number(-3), Card::Number(1), Card::Number(2), Card::Number(7),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
        ];
        let mut grid = PlayerGrid::new_no_flips(cards);
        grid.flip_card(0, 0); grid.flip_card(0, 1);
        for r in 1..4 { for c in 0..4 { grid.flip_card(r, c); } }

        let mut state = MethodicalState::new();
        let mut rng = rand::thread_rng();
        let result = choose_draw_source(&config, Some(&Card::Number(-1)), &grid, -5, 8, &mut state, &mut rng);
        assert_eq!(result, DrawSource::DiscardPile, "Should take card that helps a line");
    }

    #[test]
    fn test_draw_rejects_unhelpful_card() {
        let config = expert_methodical();
        let grid = make_grid_all_face_up(&[1,2,3,4, 5,6,7,8, 1,2,3,4, 5,6,7,8]);
        let mut state = MethodicalState::new();
        let mut rng = rand::thread_rng();
        // 8 is a high card that won't help any line much on a board with no face-down cards
        let result = choose_draw_source(&config, Some(&Card::Number(8)), &grid, -5, 8, &mut state, &mut rng);
        assert_eq!(result, DrawSource::DrawPile, "Should reject unhelpful card");
    }

    #[test]
    fn test_draw_no_discard_available() {
        let config = expert_methodical();
        let grid = make_grid_all_face_up(&[1,2,3,4, 5,6,7,8, 1,2,3,4, 5,6,7,8]);
        let mut state = MethodicalState::new();
        let mut rng = rand::thread_rng();
        let result = choose_draw_source(&config, None, &grid, -5, 8, &mut state, &mut rng);
        assert_eq!(result, DrawSource::DrawPile, "Should draw from pile when no discard");
    }

    #[test]
    fn test_draw_skill_zero_is_random() {
        // skill 0.0 means should_play_smart always returns false → coin flip
        let config = PlayerConfig {
            archetype: AiArchetype::Methodical,
            skill: 0.0,
            flip_strategy: Default::default(),
        };
        let grid = make_grid_one_away();
        let mut state = MethodicalState::new();
        let mut rng = rand::thread_rng();
        let mut saw_draw = false;
        let mut saw_discard = false;
        for _ in 0..100 {
            let result = choose_draw_source(&config, Some(&Card::Number(0)), &grid, -5, 8, &mut state, &mut rng);
            match result {
                DrawSource::DrawPile => saw_draw = true,
                DrawSource::DiscardPile => saw_discard = true,
            }
        }
        assert!(saw_draw && saw_discard, "Skill 0 should produce both draw and discard randomly");
    }

    // ── Action tests ───────────────────────────────────────────────────

    #[test]
    fn test_action_places_completing_card() {
        let config = expert_methodical();
        let grid = make_grid_one_away(); // Row 0: -3,1,2,face_down → needs 0
        let mut state = MethodicalState::new();
        let mut rng = rand::thread_rng();

        let action = choose_action(&config, &Card::Number(0), &grid, -5, 8, &mut state, &mut rng);
        match action {
            TurnAction::ReplaceCard { row, col } => assert_eq!((row, col), (0, 3)),
            _ => panic!("Should place completing card at (0,3)"),
        }
    }

    #[test]
    fn test_action_picks_highest_scoring_completion() {
        // Two rows both need 0 to complete.
        // Row 0: -3, 1, 2, face_down → gap=0, needs 0
        // Row 1: -2, 1, 1, face_down → gap=0, needs 0
        // Both completable — we just verify a completion happens.
        let cards: Vec<Card> = vec![
            Card::Number(-3), Card::Number(1), Card::Number(2), Card::Number(7),
            Card::Number(-2), Card::Number(1), Card::Number(1), Card::Number(8),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
        ];
        let mut grid = PlayerGrid::new_no_flips(cards);
        grid.flip_card(0, 0); grid.flip_card(0, 1); grid.flip_card(0, 2);
        grid.flip_card(1, 0); grid.flip_card(1, 1); grid.flip_card(1, 2);
        for r in 2..4 { for c in 0..4 { grid.flip_card(r, c); } }

        let config = expert_methodical();
        let mut state = MethodicalState::new();
        let mut rng = rand::thread_rng();

        let action = choose_action(&config, &Card::Number(0), &grid, -5, 8, &mut state, &mut rng);
        match action {
            TurnAction::ReplaceCard { row, col } => {
                assert!(
                    (row == 0 && col == 3) || (row == 1 && col == 3),
                    "Should place at a completing position, got ({}, {})", row, col
                );
            }
            _ => panic!("Should place completing card"),
        }
    }

    #[test]
    fn test_action_places_helpful_card() {
        let config = expert_methodical();
        // Grid with face-down cards where a Wild is useful
        let cards: Vec<Card> = vec![
            Card::Number(-3), Card::Number(1), Card::Number(2), Card::Number(7),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
        ];
        let mut grid = PlayerGrid::new_no_flips(cards);
        grid.flip_card(0, 0); grid.flip_card(0, 1); grid.flip_card(0, 2);
        for r in 1..4 { for c in 0..4 { grid.flip_card(r, c); } }

        let mut state = MethodicalState::new();
        let mut rng = rand::thread_rng();

        // best_placement for a Wild should find a good spot (score >= 20)
        let action = choose_action(&config, &Card::Wild, &grid, -5, 8, &mut state, &mut rng);
        assert!(matches!(action, TurnAction::ReplaceCard { .. }), "Wild should always be placed");
    }

    #[test]
    fn test_action_discards_unhelpful_card_and_flips() {
        let config = expert_methodical();
        // Grid with all face-up except one face-down. An 8 won't help much.
        let cards: Vec<Card> = vec![
            Card::Number(1), Card::Number(2), Card::Number(3), Card::Number(4),
            Card::Number(5), Card::Number(6), Card::Number(7), Card::Number(8),
            Card::Number(1), Card::Number(2), Card::Number(3), Card::Number(4),
            Card::Number(5), Card::Number(6), Card::Number(7), Card::Number(8),
        ];
        let mut grid = PlayerGrid::new_no_flips(cards);
        // Flip all except (3,3)
        for r in 0..4 {
            for c in 0..4 {
                if !(r == 3 && c == 3) { grid.flip_card(r, c); }
            }
        }

        let mut state = MethodicalState::new();
        let mut rng = rand::thread_rng();

        let action = choose_action(&config, &Card::Number(8), &grid, -5, 8, &mut state, &mut rng);
        assert!(matches!(action, TurnAction::DiscardAndFlip { .. }), "Should discard unhelpful card and flip");
    }

    #[test]
    fn test_action_all_face_up_places_anyway() {
        let config = expert_methodical();
        let grid = make_grid_all_face_up(&[1,2,3,4, 5,6,7,8, 1,2,3,4, 5,6,7,8]);
        let mut state = MethodicalState::new();
        let mut rng = rand::thread_rng();

        let action = choose_action(&config, &Card::Number(0), &grid, -5, 8, &mut state, &mut rng);
        assert!(matches!(action, TurnAction::ReplaceCard { .. }), "Must place when all face-up");
    }

    #[test]
    fn test_action_skill_zero_uses_fallback() {
        // skill 0.0 → should_play_smart always false → fallback_action
        // fallback places low cards (abs <= 3) and discards high cards
        let config = PlayerConfig {
            archetype: AiArchetype::Methodical,
            skill: 0.0,
            flip_strategy: Default::default(),
        };
        let cards: Vec<Card> = vec![
            Card::Number(1), Card::Number(2), Card::Number(3), Card::Number(4),
            Card::Number(5), Card::Number(6), Card::Number(7), Card::Number(8),
            Card::Number(1), Card::Number(2), Card::Number(3), Card::Number(4),
            Card::Number(5), Card::Number(6), Card::Number(7), Card::Number(8),
        ];
        let mut grid = PlayerGrid::new_no_flips(cards);
        grid.flip_card(0, 0); grid.flip_card(0, 1);

        let mut state = MethodicalState::new();
        let mut rng = rand::thread_rng();

        // High card with face-down available → fallback should discard and flip
        let action = choose_action(&config, &Card::Number(8), &grid, -5, 8, &mut state, &mut rng);
        assert!(matches!(action, TurnAction::DiscardAndFlip { .. }),
            "Skill 0 with high card should use fallback (discard and flip)");

        // Low card with face-down available → fallback should place it
        let action = choose_action(&config, &Card::Number(0), &grid, -5, 8, &mut state, &mut rng);
        assert!(matches!(action, TurnAction::ReplaceCard { .. }),
            "Skill 0 with low card should use fallback (place it)");
    }
}
