use rand::Rng;

use super::line_scoring::{score_all_lines, card_fits_line, best_placement, best_flip_target};
use super::{DrawSource, TurnAction, should_play_smart};
use super::super::card::Card;
use super::super::config::PlayerConfig;
use super::super::grid::PlayerGrid;

/// Opportunist: Line-first reactive play. No memory between turns.
pub fn choose_draw_source(
    config: &PlayerConfig,
    discard_top: Option<&Card>,
    grid: &PlayerGrid,
    neg_min: i32,
    pos_max: i32,
    rng: &mut impl Rng,
) -> DrawSource {
    let card = match discard_top {
        Some(c) => c,
        None => return DrawSource::DrawPile,
    };

    // Skill check: fall back to random
    if !should_play_smart(config.skill, rng) {
        return if rng.gen_bool(0.5) { DrawSource::DiscardPile } else { DrawSource::DrawPile };
    }

    // Always take a Wild
    if matches!(card, Card::Wild) {
        return DrawSource::DiscardPile;
    }

    let card_value = match card { Card::Number(v) => *v, Card::Wild => 0 };
    let lines = score_all_lines(grid, neg_min, pos_max);

    // Check if discard completes ANY line
    for (line, _score) in &lines {
        if card_fits_line(card_value, line, neg_min, pos_max) >= 100.0 {
            return DrawSource::DiscardPile;
        }
    }

    // Check if discard significantly helps the hottest line
    let hottest = lines.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    if let Some((hot_line, _)) = hottest {
        if card_fits_line(card_value, hot_line, neg_min, pos_max) >= 50.0 {
            return DrawSource::DiscardPile;
        }
    }

    // Always take a 0 (universally useful for sum-to-zero)
    if card_value == 0 {
        return DrawSource::DiscardPile;
    }

    DrawSource::DrawPile
}

pub fn choose_action(
    config: &PlayerConfig,
    drawn_card: &Card,
    grid: &PlayerGrid,
    neg_min: i32,
    pos_max: i32,
    rng: &mut impl Rng,
) -> TurnAction {
    let face_down = grid.face_down_positions();

    // Skill check: fall back to simple heuristic
    if !should_play_smart(config.skill, rng) {
        return fallback_action(drawn_card, grid, rng);
    }

    // Compute best placement
    let (pos, score) = best_placement(drawn_card, grid, neg_min, pos_max);

    // If placement score is meaningful, place it
    if score >= 30.0 {
        return TurnAction::ReplaceCard { row: pos.0, col: pos.1 };
    }

    // Otherwise: discard and flip the most useful face-down card
    if !face_down.is_empty() {
        let lines = score_all_lines(grid, neg_min, pos_max);
        let flip_target = best_flip_target(&face_down, &lines);
        return TurnAction::DiscardAndFlip { row: flip_target.0, col: flip_target.1 };
    }

    // All face-up: must replace something
    TurnAction::ReplaceCard { row: pos.0, col: pos.1 }
}

/// Fallback when skill check fails. pub(super) so Methodical can reference it later.
pub(super) fn fallback_action(drawn_card: &Card, grid: &PlayerGrid, rng: &mut impl Rng) -> TurnAction {
    let card_abs = match drawn_card { Card::Number(v) => v.abs(), Card::Wild => 0 };
    let face_down = grid.face_down_positions();

    if card_abs <= 3 && !face_down.is_empty() {
        // Low card: replace a random face-down
        let idx = rng.gen_range(0..face_down.len());
        TurnAction::ReplaceCard { row: face_down[idx].0, col: face_down[idx].1 }
    } else if !face_down.is_empty() {
        // High card: discard and flip random
        let idx = rng.gen_range(0..face_down.len());
        TurnAction::DiscardAndFlip { row: face_down[idx].0, col: face_down[idx].1 }
    } else {
        // All face-up: replace worst card
        let occupied = grid.occupied_positions();
        let mut worst_pos = occupied[0];
        let mut worst_val = 0i32;
        for &(r, c) in &occupied {
            if let Some(gc) = grid.get(r, c) {
                if gc.face_up && !matches!(gc.card, Card::Wild) {
                    let v = match &gc.card { Card::Number(v) => v.abs(), Card::Wild => 0 };
                    if v >= worst_val { worst_val = v; worst_pos = (r, c); }
                }
            }
        }
        TurnAction::ReplaceCard { row: worst_pos.0, col: worst_pos.1 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::config::AiArchetype;

    fn expert_opportunist() -> PlayerConfig {
        PlayerConfig {
            archetype: AiArchetype::Opportunist,
            skill: 1.0,
            flip_strategy: Default::default(),
        }
    }

    fn make_grid_all_face_up(values: &[i32]) -> PlayerGrid {
        let cards: Vec<Card> = values.iter().map(|&v| Card::Number(v)).collect();
        let mut grid = PlayerGrid::new_no_flips(cards);
        for r in 0..4 { for c in 0..4 { grid.flip_card(r, c); } }
        grid
    }

    #[test]
    fn test_always_takes_wild_from_discard() {
        let config = expert_opportunist();
        let grid = make_grid_all_face_up(&[1,2,3,4, 5,6,7,8, 1,2,3,4, 5,6,7,8]);
        let mut rng = rand::thread_rng();
        for _ in 0..20 {
            let result = choose_draw_source(&config, Some(&Card::Wild), &grid, -5, 8, &mut rng);
            assert_eq!(result, DrawSource::DiscardPile);
        }
    }

    #[test]
    fn test_takes_completing_card_from_discard() {
        let config = expert_opportunist();
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
        let result = choose_draw_source(&config, Some(&Card::Number(0)), &grid, -5, 8, &mut rng);
        assert_eq!(result, DrawSource::DiscardPile);
    }

    #[test]
    fn test_places_card_to_complete_line() {
        let config = expert_opportunist();
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
        let action = choose_action(&config, &Card::Number(0), &grid, -5, 8, &mut rng);
        match action {
            TurnAction::ReplaceCard { row, col } => {
                assert_eq!((row, col), (0, 3), "Should place at face-down slot completing row 0");
            }
            _ => panic!("Should replace, not discard"),
        }
    }
}
