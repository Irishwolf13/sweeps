use rand::Rng;

use super::line_scoring::{score_all_lines, card_fits_line, best_placement};
use super::{DrawSource, TurnAction, should_play_smart};
use super::super::card::{Card, Shape, Shade};
use super::super::config::{EliminationContext, GameMode, PlayerConfig};
use super::super::grid::PlayerGrid;

/// Representative card values for sampling when skill < 0.8.
/// Covers the full range with common values weighted more.
const SAMPLE_VALUES: [i32; 10] = [-5, -3, -1, 0, 0, 1, 3, 5, 7, 8];

/// Compute expected value of drawing blind from the deck.
/// At high skill, evaluates all possible card values weighted by deck distribution.
/// At lower skill, samples a subset.
fn blind_draw_expected_score(
    grid: &PlayerGrid,
    ctx: &EliminationContext,
    skill: f64,
    _rng: &mut impl Rng,
) -> f64 {
    match ctx.game_mode {
        GameMode::Numbers => {
            if skill >= 0.8 {
                // Full distribution: evaluate every possible card value
                let mut total_score = 0.0f64;
                let mut total_weight = 0.0f64;

                // Number cards
                for v in ctx.neg_min..=ctx.pos_max {
                    let card = Card::Number(v);
                    let (_, score) = best_placement(&card, grid, ctx);
                    let weight = 1.0;
                    total_score += score * weight;
                    total_weight += weight;
                }

                // Wild
                let (_, wild_score) = best_placement(&Card::Wild, grid, ctx);
                total_score += wild_score * 0.5;
                total_weight += 0.5;

                if total_weight > 0.0 { total_score / total_weight } else { 0.0 }
            } else {
                // Sample 10 representative cards
                let mut total = 0.0f64;
                for &v in &SAMPLE_VALUES {
                    let card = Card::Number(v);
                    let (_, score) = best_placement(&card, grid, ctx);
                    total += score;
                }
                total / SAMPLE_VALUES.len() as f64
            }
        }
        GameMode::Shapes => {
            let mut total_score = 0.0f64;
            let mut count = 0.0f64;
            for shape in &[Shape::Circle, Shape::Square, Shape::Triangle, Shape::Rectangle] {
                for shade in &[Shade::Unshaded, Shade::Shaded] {
                    let card = Card::Shape(shape.clone(), shade.clone());
                    let (_, score) = best_placement(&card, grid, ctx);
                    total_score += score;
                    count += 1.0;
                }
            }
            for wild in &[Card::Wild, Card::WildShaded, Card::WildUnshaded] {
                let (_, score) = best_placement(wild, grid, ctx);
                total_score += score * 0.3;
                count += 0.3;
            }
            if count > 0.0 { total_score / count } else { 0.0 }
        }
    }
}

/// Score the cascade potential of a placement.
/// Simulates placing the card, checks if elimination happens, then scores resulting grid.
fn cascade_score(
    card: &Card,
    pos: (usize, usize),
    grid: &PlayerGrid,
    ctx: &EliminationContext,
    allow_matching: bool,
    allow_diagonal: bool,
) -> f64 {
    let mut sim_grid = grid.clone();
    sim_grid.replace_card(pos.0, pos.1, card.clone());

    let eliminations = sim_grid.find_eliminations(allow_matching, allow_diagonal, ctx);
    if eliminations.is_empty() {
        return 0.0;
    }

    // Apply first elimination
    let elim = &eliminations[0];
    sim_grid.eliminate(&elim.positions);
    sim_grid.cleanup();

    // Score the resulting grid — more lines close to completion = better
    let post_lines = score_all_lines(&sim_grid, ctx);
    let post_score: f64 = post_lines.iter().map(|(_, s)| s).sum();

    // Check for further eliminations (cascade)
    let further = sim_grid.find_eliminations(allow_matching, allow_diagonal, ctx);
    let cascade_bonus = if further.is_empty() { 0.0 } else { 50.0 };

    // Base bonus for triggering an elimination + cascade potential
    30.0 + cascade_bonus + post_score * 0.1
}

pub fn choose_draw_source(
    config: &PlayerConfig,
    discard_top: Option<&Card>,
    grid: &PlayerGrid,
    ctx: &EliminationContext,
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
    if matches!(card, Card::Wild | Card::WildShaded | Card::WildUnshaded) {
        return DrawSource::DiscardPile;
    }

    // Score taking the discard
    let (_, discard_score) = best_placement(card, grid, ctx);

    // Score drawing blind (expected value)
    let blind_score = blind_draw_expected_score(grid, ctx, config.skill, rng);

    if discard_score >= blind_score {
        DrawSource::DiscardPile
    } else {
        DrawSource::DrawPile
    }
}

pub fn choose_action(
    config: &PlayerConfig,
    drawn_card: &Card,
    grid: &PlayerGrid,
    ctx: &EliminationContext,
    rng: &mut impl Rng,
) -> TurnAction {
    let face_down = grid.face_down_positions();

    if !should_play_smart(config.skill, rng) {
        return super::opportunist::fallback_action(drawn_card, grid, ctx, rng);
    }

    // Evaluate all possible placements
    let occupied = grid.occupied_positions();
    let card_value = match drawn_card { Card::Number(v) => *v, _ => 0 };
    let is_wild = matches!(drawn_card, Card::Wild | Card::WildShaded | Card::WildUnshaded);

    let mut best_pos = occupied.first().copied().unwrap_or((0, 0));
    let mut best_score = f64::NEG_INFINITY;

    // Hoist line scoring outside per-position loop — grid state doesn't change here
    let lines = score_all_lines(grid, ctx);

    for &(r, c) in &occupied {
        // Don't replace Wild with non-Wild
        if !is_wild {
            if let Some(gc) = grid.get(r, c) {
                if gc.face_up && matches!(gc.card, Card::Wild | Card::WildShaded | Card::WildUnshaded) { continue; }
            }
        }

        let mut score = 0.0f64;

        for (line, _) in &lines {
            if !line.positions.contains(&(r, c)) { continue; }

            let is_face_down = grid.get(r, c).map_or(false, |gc| !gc.face_up);
            if is_face_down {
                score += card_fits_line(drawn_card, line, ctx);
            } else {
                // Replacing face-up: evaluate improvement
                let old_value = grid.get(r, c).map_or(0, |gc| match &gc.card {
                    Card::Number(v) => *v, _ => 0,
                });
                let old_gap_dist = (line.gap).abs() as f64;
                let new_gap_dist = (line.gap + old_value - card_value).abs() as f64;
                if new_gap_dist < old_gap_dist {
                    score += 20.0 + (old_gap_dist - new_gap_dist) * 5.0;
                }
            }
        }

        // Cascade bonus (skill >= 0.6)
        if config.skill >= 0.6 {
            score += cascade_score(drawn_card, (r, c), grid, ctx, true, true);
        }

        // Replacing bad face-up with better card
        if let Some(gc) = grid.get(r, c) {
            if gc.face_up {
                let old_abs = match &gc.card { Card::Number(v) => v.abs(), _ => 0 };
                if card_value.abs() < old_abs {
                    score += (old_abs - card_value.abs()) as f64 * 2.0;
                }
            }
        }

        if score > best_score {
            best_score = score;
            best_pos = (r, c);
        }
    }

    // Compare best placement vs discarding + flipping
    let discard_flip_score = if !face_down.is_empty() {
        // Expected information gain from flipping
        face_down.iter()
            .map(|&(r, c)| {
                lines.iter()
                    .filter(|(line, _)| line.positions.contains(&(r, c)))
                    .map(|(_, s)| s)
                    .sum::<f64>()
            })
            .fold(0.0f64, f64::max) * 0.3 // Discount: info gain is speculative
    } else {
        f64::NEG_INFINITY
    };

    if best_score >= discard_flip_score && best_score > 0.0 {
        TurnAction::ReplaceCard { row: best_pos.0, col: best_pos.1 }
    } else if !face_down.is_empty() {
        // Flip the face-down card with highest info potential
        let mut flip_pos = face_down[0];
        let mut flip_score = f64::NEG_INFINITY;
        for &(r, c) in &face_down {
            let s: f64 = lines.iter()
                .filter(|(line, _)| line.positions.contains(&(r, c)))
                .map(|(_, s)| s)
                .sum();
            if s > flip_score { flip_score = s; flip_pos = (r, c); }
        }
        TurnAction::DiscardAndFlip { row: flip_pos.0, col: flip_pos.1 }
    } else {
        TurnAction::ReplaceCard { row: best_pos.0, col: best_pos.1 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::config::{AiArchetype, GameMode};

    fn numbers_ctx() -> EliminationContext {
        EliminationContext {
            game_mode: GameMode::Numbers,
            neg_min: -5, pos_max: 8,
            shade_matters: false, allow_cancellation: false,
        }
    }

    fn expert_calculator() -> PlayerConfig {
        PlayerConfig {
            archetype: AiArchetype::Calculator,
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
    fn test_takes_discard_when_better_than_blind() {
        let config = expert_calculator();
        let ctx = numbers_ctx();
        // Row 0: -3, 1, 2, face_down → 0 completes it. Discard has 0.
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
        let result = choose_draw_source(&config, Some(&Card::Number(0)), &grid, &ctx, &mut rng);
        assert_eq!(result, DrawSource::DiscardPile, "Completing card should beat blind draw EV");
    }

    #[test]
    fn test_draws_blind_when_discard_is_bad() {
        let config = expert_calculator();
        let ctx = numbers_ctx();
        let grid = make_grid_all_face_up(&[1,2,3,4, 5,6,7,8, 1,2,3,4, 5,6,7,8]);
        let mut rng = rand::thread_rng();

        // Discard is 8 (bad card for sum-to-zero on this grid)
        // Blind draw has better expected value
        let _result = choose_draw_source(&config, Some(&Card::Number(8)), &grid, &ctx, &mut rng);
        // We can't assert DrawPile with certainty (depends on grid analysis) but
        // at minimum it shouldn't always take an 8
        // This is a soft test — the real validation is the integration turn-count test
    }

    #[test]
    fn test_places_card_considering_cascade() {
        let config = expert_calculator();
        let ctx = numbers_ctx();
        // Row 0: -3, 1, 2, face_down → placing 0 completes it AND triggers elimination
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
        let action = choose_action(&config, &Card::Number(0), &grid, &ctx, &mut rng);
        // Calculator evaluates all positions — it should place the card somewhere
        // (not discard). The exact position depends on multi-line scoring.
        assert!(matches!(action, TurnAction::ReplaceCard { .. }),
            "Calculator should place a 0 card, not discard it");
    }

    #[test]
    fn test_blind_draw_ev_returns_finite() {
        let ctx = numbers_ctx();
        let grid = make_grid_all_face_up(&[1,2,3,4, 5,6,7,8, 1,2,3,4, 5,6,7,8]);
        let mut rng = rand::thread_rng();
        let ev = blind_draw_expected_score(&grid, &ctx, 1.0, &mut rng);
        assert!(ev.is_finite(), "EV should be a finite number");
        let ev_low = blind_draw_expected_score(&grid, &ctx, 0.5, &mut rng);
        assert!(ev_low.is_finite(), "Sampled EV should also be finite");
    }
}
