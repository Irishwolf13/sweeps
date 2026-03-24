use rand::Rng;

use super::card::Card;
use super::config::PlayerConfig;
use super::grid::{EliminationType, PlayerGrid, SlideDirection};

// ── Public enums ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum DrawSource {
    DrawPile,
    DiscardPile,
}

#[derive(Debug, Clone)]
pub enum TurnAction {
    ReplaceCard { row: usize, col: usize },
    DiscardAndFlip { row: usize, col: usize },
}

// ── Card evaluation helpers ─────────────────────────────────────────────

/// How "keepable" is this card? Based on absolute value vs the player's threshold.
/// Wild is always keepable. Negatives get a slight bonus (rarer in deck).
fn card_abs_value(card: &Card) -> i32 {
    match card {
        Card::Wild => 0,  // Wild is as good as 0
        Card::Number(v) => v.abs(),
    }
}

/// Is this card worth keeping given the player's threshold?
fn is_keepable(card: &Card, keep_threshold: i32) -> bool {
    match card {
        Card::Wild => true,
        Card::Number(v) => v.abs() <= keep_threshold,
    }
}

// ── Grid line analysis ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct LineAnalysis {
    positions: Vec<(usize, usize)>,
    face_up_values: Vec<i32>,
    wild_count: usize,
    face_down_count: usize,
    needed_sum: i32,
    total_slots: usize,
    can_match: bool,
    match_value: Option<i32>,
}

fn analyze_lines(grid: &PlayerGrid) -> Vec<LineAnalysis> {
    let mut lines = Vec::with_capacity(10);

    for r in 0..grid.row_count() {
        let positions: Vec<(usize, usize)> = (0..grid.col_count(r)).map(|c| (r, c)).collect();
        if positions.len() >= 2 {
            lines.push(analyze_one_line(grid, &positions));
        }
    }

    let max_cols = grid.max_cols();
    for c in 0..max_cols {
        let positions: Vec<(usize, usize)> = (0..grid.row_count())
            .filter(|&r| c < grid.col_count(r))
            .map(|r| (r, c))
            .collect();
        if positions.len() >= 2 {
            lines.push(analyze_one_line(grid, &positions));
        }
    }

    if grid.is_square() {
        let n = grid.row_count();
        if n >= 2 {
            let main_diag: Vec<(usize, usize)> = (0..n).map(|i| (i, i)).collect();
            lines.push(analyze_one_line(grid, &main_diag));
            let anti_diag: Vec<(usize, usize)> = (0..n).map(|i| (i, n - 1 - i)).collect();
            lines.push(analyze_one_line(grid, &anti_diag));
        }
    }

    lines
}

fn analyze_one_line(grid: &PlayerGrid, positions: &[(usize, usize)]) -> LineAnalysis {
    let mut face_up_values = Vec::new();
    let mut wild_count = 0usize;
    let mut face_down_count = 0usize;
    let mut face_up_sum = 0i32;

    for &(r, c) in positions {
        match grid.get(r, c) {
            Some(gc) if gc.face_up => match &gc.card {
                Card::Number(v) => {
                    face_up_values.push(*v);
                    face_up_sum += v;
                }
                Card::Wild => wild_count += 1,
            },
            Some(_) => face_down_count += 1,
            None => {}
        }
    }

    let (can_match, match_value) = if face_up_values.is_empty() {
        (true, None)
    } else {
        let first = face_up_values[0];
        let all_same = face_up_values.iter().all(|&v| v == first);
        (all_same, if all_same { Some(first) } else { None })
    };

    LineAnalysis {
        positions: positions.to_vec(),
        face_up_values,
        wild_count,
        face_down_count,
        needed_sum: -face_up_sum,
        total_slots: positions.len(),
        can_match,
        match_value,
    }
}

/// Check if placing `value` at a face-down position in this line would
/// COMPLETE the elimination (all other slots are face-up and sum works out).
fn would_complete_line(value: i32, line: &LineAnalysis, neg_min: i32, pos_max: i32) -> bool {
    // After placing, this face-down becomes known, so unknowns = face_down - 1
    if line.face_down_count != 1 {
        return false; // More than 1 unknown remaining — can't complete
    }
    let new_needed = line.needed_sum - value;
    let wilds = line.wild_count;
    if wilds == 0 {
        return new_needed == 0;
    }
    let min_p = (wilds as i32) * neg_min;
    let max_p = (wilds as i32) * pos_max;
    new_needed >= min_p && new_needed <= max_p
}

/// Check if placing `value` at a face-up position in this line would
/// COMPLETE the elimination (replacing an existing face-up card).
fn would_complete_line_replacing(
    old_value: i32, new_value: i32, line: &LineAnalysis, neg_min: i32, pos_max: i32
) -> bool {
    if line.face_down_count != 0 {
        return false; // Still unknowns — can't complete
    }
    // Remove old, add new
    let new_needed = line.needed_sum + old_value - new_value;
    let wilds = line.wild_count;
    if wilds == 0 {
        return new_needed == 0;
    }
    let min_p = (wilds as i32) * neg_min;
    let max_p = (wilds as i32) * pos_max;
    new_needed >= min_p && new_needed <= max_p
}

/// Score how much placing `value` helps a line toward elimination.
/// Returns 0-100. 100+ = would complete it.
fn value_helps_line(value: i32, line: &LineAnalysis, neg_min: i32, pos_max: i32) -> f64 {
    let unknowns_after = if line.face_down_count > 0 { line.face_down_count - 1 } else { 0 };
    let new_needed = line.needed_sum - value;

    // Would complete?
    if unknowns_after == 0 {
        let wilds = line.wild_count;
        if wilds == 0 && new_needed == 0 { return 100.0; }
        if wilds > 0 {
            let min_p = (wilds as i32) * neg_min;
            let max_p = (wilds as i32) * pos_max;
            if new_needed >= min_p && new_needed <= max_p { return 100.0; }
        }
    }

    // Matching elimination
    if line.can_match {
        if let Some(mv) = line.match_value {
            if value == mv {
                let known = line.total_slots - line.face_down_count;
                return 40.0 + known as f64 * 15.0;
            }
        }
    }

    // Partial: keeps line viable
    let remaining = unknowns_after + line.wild_count;
    if remaining > 0 {
        let min_p = (remaining as i32) * neg_min;
        let max_p = (remaining as i32) * pos_max;
        if new_needed >= min_p && new_needed <= max_p {
            let known_count = line.total_slots - unknowns_after;
            return 10.0 + known_count as f64 * 8.0;
        }
    }

    0.0
}

// ── Public strategy functions ─────────────────────────────────────────────

/// Choose between Draw Pile and Discard Pile.
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

    // Everyone takes a Wild
    if matches!(card, Card::Wild) {
        return DrawSource::DiscardPile;
    }

    // Check if this card would complete a line (always take it!)
    if rng.gen_bool(config.skill /* TODO(task-3): replace with archetype logic */.max(0.05)) {
        let lines = analyze_lines(grid);
        let card_value = match card { Card::Number(v) => *v, Card::Wild => 0 };
        for line in &lines {
            for &(r, c) in &line.positions {
                if let Some(gc) = grid.get(r, c) {
                    if !gc.face_up {
                        if would_complete_line(card_value, line, neg_min, pos_max) {
                            return DrawSource::DiscardPile;
                        }
                    }
                }
            }
        }
    }

    // Keep threshold check
    if is_keepable(card, (config.skill * 10.0) as i32 /* TODO(task-3): replace with archetype logic */) {
        return DrawSource::DiscardPile;
    }

    // Mediocre card: check if it meaningfully helps a line
    if rng.gen_bool(config.skill /* TODO(task-3): replace with archetype logic */.max(0.05)) {
        let lines = analyze_lines(grid);
        let card_value = match card { Card::Number(v) => *v, Card::Wild => 0 };
        let mut best_help = 0.0f64;
        for line in &lines {
            best_help = best_help.max(value_helps_line(card_value, line, neg_min, pos_max));
        }
        if best_help >= 40.0 {
            return DrawSource::DiscardPile;
        }
    }

    DrawSource::DrawPile
}

/// Choose what to do with a drawn card.
///
/// Priority order:
/// 1. ALWAYS check if the card completes a line — if so, place it there.
/// 2. If keepable (|val| <= threshold or Wild): replace a face-down card,
///    unless a face-up card is much worse.
/// 3. If not keepable but helps a near-complete line: place it there.
/// 4. Otherwise: discard & flip a face-down card.
pub fn choose_action(
    config: &PlayerConfig,
    drawn_card: &Card,
    grid: &PlayerGrid,
    neg_min: i32,
    pos_max: i32,
    rng: &mut impl Rng,
) -> TurnAction {
    let face_down = grid.face_down_positions();
    let occupied = grid.occupied_positions();

    // Edge cases
    if face_down.is_empty() {
        return pick_best_replace(config, drawn_card, grid, &occupied, neg_min, pos_max, rng);
    }
    if occupied.is_empty() {
        let &(r, c) = &face_down[rng.gen_range(0..face_down.len())];
        return TurnAction::DiscardAndFlip { row: r, col: c };
    }

    let drawn_value = match drawn_card {
        Card::Number(v) => *v,
        Card::Wild => 0,
    };
    let is_wild = matches!(drawn_card, Card::Wild);
    let lines = analyze_lines(grid);

    // ═══ PRIORITY 1: Does this card complete a line? ═══════════════════
    // Even mediocre players spot an obvious completion most of the time.
    if rng.gen_bool(config.skill /* TODO(task-3): replace with archetype logic */.max(0.15)) {
        // Check face-down positions: placing drawn card there completes a line
        for &(r, c) in &face_down {
            for line in &lines {
                if line.positions.contains(&(r, c)) {
                    if would_complete_line(drawn_value, line, neg_min, pos_max) {
                        return TurnAction::ReplaceCard { row: r, col: c };
                    }
                }
            }
        }

        // Check face-up positions: replacing a face-up card completes a line
        for &(r, c) in &occupied {
            if let Some(gc) = grid.get(r, c) {
                if gc.face_up {
                    let old_val = match &gc.card {
                        Card::Number(v) => *v,
                        Card::Wild => continue, // Never replace a Wild
                    };
                    for line in &lines {
                        if line.positions.contains(&(r, c)) {
                            if would_complete_line_replacing(old_val, drawn_value, line, neg_min, pos_max) {
                                return TurnAction::ReplaceCard { row: r, col: c };
                            }
                        }
                    }
                }
            }
        }
    }

    // ═══ PRIORITY 2: Keepable card → replace face-down or bad face-up ══
    if is_wild || is_keepable(drawn_card, (config.skill * 10.0) as i32 /* TODO(task-3): replace with archetype logic */) {
        // Check if there's a face-up card that's really bad
        let worst = find_worst_face_up(grid, &occupied);
        if let Some((wr, wc, worst_abs)) = worst {
            // Replace it if it's much worse than our drawn card
            // (e.g. drawn a 2, grid has a 10 → replace the 10)
            if worst_abs > ((config.skill * 10.0) as i32 /* TODO(task-3): replace with archetype logic */ + 3).max(drawn_value.abs() + 3) {
                // But never replace a Wild
                if let Some(gc) = grid.get(wr, wc) {
                    if !matches!(gc.card, Card::Wild) {
                        return TurnAction::ReplaceCard { row: wr, col: wc };
                    }
                }
            }
        }

        // Default: replace a face-down card
        if !face_down.is_empty() {
            let target = pick_face_down_target(config, &face_down, &lines, neg_min, pos_max, drawn_value, rng);
            return TurnAction::ReplaceCard { row: target.0, col: target.1 };
        }
    }

    // ═══ PRIORITY 3: Not keepable, but helps a near-complete line? ══════
    if rng.gen_bool(config.skill /* TODO(task-3): replace with archetype logic */.max(0.05)) {
        let mut best_score = 0.0f64;
        let mut best_pos: Option<(usize, usize)> = None;

        // Check all positions (face-down and face-up)
        for &(r, c) in &occupied {
            let s = score_placement(grid, r, c, drawn_card, &lines, neg_min, pos_max);
            if s > best_score {
                best_score = s;
                best_pos = Some((r, c));
            }
        }

        // Place if it significantly helps a line (but not a full completion — that was caught above)
        if best_score >= 50.0 {
            if let Some((r, c)) = best_pos {
                // Don't replace a Wild
                if let Some(gc) = grid.get(r, c) {
                    if !gc.face_up || !matches!(gc.card, Card::Wild) {
                        return TurnAction::ReplaceCard { row: r, col: c };
                    }
                }
            }
        }
    }

    // ═══ PRIORITY 4: Discard & flip ════════════════════════════════════
    let flip_target = pick_flip_target(config, &face_down, &lines, rng);
    TurnAction::DiscardAndFlip { row: flip_target.0, col: flip_target.1 }
}

/// Choose which eliminated card to place on the discard pile.
pub fn choose_discard_from_eliminated(
    config: &PlayerConfig,
    eliminated_cards: &[Card],
    rng: &mut impl Rng,
) -> usize {
    if eliminated_cards.len() <= 1 {
        return 0;
    }

    // Discard highest absolute value, never Wild
    let mut best_idx = 0;
    let mut best_score = i32::MIN;
    for (i, card) in eliminated_cards.iter().enumerate() {
        let score = match card {
            Card::Number(v) => v.abs(),
            Card::Wild => -100,
        };
        if score > best_score {
            best_score = score;
            best_idx = i;
        }
    }
    best_idx
}

/// Choose which eliminated card to discard, considering the next player's grid.
pub fn choose_discard_with_opponent(
    config: &PlayerConfig,
    eliminated_cards: &[Card],
    next_player_grid: Option<&PlayerGrid>,
    neg_min: i32,
    pos_max: i32,
    rng: &mut impl Rng,
) -> usize {
    if eliminated_cards.len() <= 1 {
        return 0;
    }

    let base_idx = choose_discard_from_eliminated(config, eliminated_cards, rng);

    if config.skill /* TODO(task-3): replace with archetype logic */ <= 0.0 || !rng.gen_bool(config.skill /* TODO(task-3): replace with archetype logic */) {
        return base_idx;
    }

    let next_grid = match next_player_grid {
        Some(g) => g,
        None => return base_idx,
    };

    let next_lines = analyze_lines(next_grid);
    let chosen_value = match &eliminated_cards[base_idx] {
        Card::Number(v) => *v,
        Card::Wild => return base_idx,
    };

    // Check if our chosen discard helps the opponent complete a line
    let mut helps_opponent = false;
    for line in &next_lines {
        if value_helps_line(chosen_value, line, neg_min, pos_max) >= 80.0 {
            helps_opponent = true;
            break;
        }
    }

    if !helps_opponent {
        return base_idx;
    }

    // Find alternative that doesn't help opponent as much
    let mut best_alt_idx = base_idx;
    let mut best_alt_abs = i32::MIN;
    for (i, card) in eliminated_cards.iter().enumerate() {
        if i == base_idx { continue; }
        let val = match card {
            Card::Number(v) => *v,
            Card::Wild => continue,
        };
        let mut max_help = 0.0f64;
        for line in &next_lines {
            max_help = max_help.max(value_helps_line(val, line, neg_min, pos_max));
        }
        if max_help < 60.0 && val.abs() > best_alt_abs {
            best_alt_abs = val.abs();
            best_alt_idx = i;
        }
    }

    best_alt_idx
}

/// Choose slide direction after diagonal elimination.
pub fn choose_slide_direction(
    config: &PlayerConfig,
    grid: &PlayerGrid,
    _eliminated_kind: &EliminationType,
    rng: &mut impl Rng,
) -> SlideDirection {
    if !rng.gen_bool(config.skill /* TODO(task-3): replace with archetype logic */.max(0.2)) {
        return if rng.gen_bool(0.5) {
            SlideDirection::Horizontal
        } else {
            SlideDirection::Vertical
        };
    }

    let mut grid_h = grid.clone();
    grid_h.reshape_after_diagonal(&EliminationType::MainDiagonal, SlideDirection::Horizontal);
    grid_h.cleanup();
    let score_h = score_grid_potential(&grid_h);

    let mut grid_v = grid.clone();
    grid_v.reshape_after_diagonal(&EliminationType::MainDiagonal, SlideDirection::Vertical);
    grid_v.cleanup();
    let score_v = score_grid_potential(&grid_v);

    if score_h >= score_v { SlideDirection::Horizontal } else { SlideDirection::Vertical }
}

// ── Internal helpers ──────────────────────────────────────────────────────

/// Find the worst face-up card (highest absolute value). Returns (row, col, abs_value).
fn find_worst_face_up(grid: &PlayerGrid, occupied: &[(usize, usize)]) -> Option<(usize, usize, i32)> {
    let mut worst: Option<(usize, usize, i32)> = None;
    for &(r, c) in occupied {
        if let Some(gc) = grid.get(r, c) {
            if gc.face_up {
                let abs = card_abs_value(&gc.card);
                if worst.is_none() || abs > worst.unwrap().2 {
                    worst = Some((r, c, abs));
                }
            }
        }
    }
    worst
}

/// Pick the best face-down position to place a good drawn card.
/// Prefers positions in lines closest to completion.
fn pick_face_down_target(
    config: &PlayerConfig,
    face_down: &[(usize, usize)],
    lines: &[LineAnalysis],
    neg_min: i32,
    pos_max: i32,
    drawn_value: i32,
    rng: &mut impl Rng,
) -> (usize, usize) {
    if face_down.is_empty() {
        return (0, 0);
    }

    if !rng.gen_bool(config.skill /* TODO(task-3): replace with archetype logic */.max(0.1)) {
        return face_down[rng.gen_range(0..face_down.len())];
    }

    let mut best_pos = face_down[0];
    let mut best_score = f64::NEG_INFINITY;

    for &(r, c) in face_down {
        let mut score = 0.0f64;
        for line in lines {
            if !line.positions.contains(&(r, c)) { continue; }
            let mut modified = line.clone();
            if modified.face_down_count > 0 { modified.face_down_count -= 1; }
            let help = value_helps_line(drawn_value, &modified, neg_min, pos_max);
            score += help;
            // Bonus for lines with fewer unknowns
            let known_ratio = (modified.total_slots - modified.face_down_count) as f64
                / modified.total_slots as f64;
            score += known_ratio * 5.0;
        }
        if score > best_score {
            best_score = score;
            best_pos = (r, c);
        }
    }

    best_pos
}

/// Pick the best face-down card to flip when discarding.
/// Prefers cards in lines closest to completion (gather info where it matters).
fn pick_flip_target(
    config: &PlayerConfig,
    face_down: &[(usize, usize)],
    lines: &[LineAnalysis],
    rng: &mut impl Rng,
) -> (usize, usize) {
    if face_down.is_empty() {
        return (0, 0);
    }

    if !rng.gen_bool(config.skill /* TODO(task-3): replace with archetype logic */.max(0.1)) {
        return face_down[rng.gen_range(0..face_down.len())];
    }

    let mut best_pos = face_down[0];
    let mut best_score = f64::NEG_INFINITY;

    for &(r, c) in face_down {
        let mut score = 0.0f64;
        for line in lines {
            if !line.positions.contains(&(r, c)) { continue; }
            let known = line.total_slots - line.face_down_count;
            score += (known as f64 + 1.0) / (line.face_down_count as f64 + 1.0) * 10.0;
        }
        if score > best_score {
            best_score = score;
            best_pos = (r, c);
        }
    }

    best_pos
}

/// Score placing a card at a specific position for line potential.
fn score_placement(
    grid: &PlayerGrid,
    row: usize,
    col: usize,
    new_card: &Card,
    lines: &[LineAnalysis],
    neg_min: i32,
    pos_max: i32,
) -> f64 {
    let new_value = match new_card {
        Card::Number(v) => *v,
        Card::Wild => 0,
    };

    let mut score = 0.0f64;
    if matches!(new_card, Card::Wild) { score += 20.0; }

    for line in lines {
        if !line.positions.contains(&(row, col)) { continue; }

        let mut modified = line.clone();
        let current_value = grid.get(row, col).and_then(|gc| {
            if gc.face_up {
                Some(match &gc.card { Card::Number(v) => *v, Card::Wild => 0 })
            } else {
                None
            }
        });

        if let Some(cv) = current_value {
            if let Some(pos) = modified.face_up_values.iter().position(|&v| v == cv) {
                modified.face_up_values.remove(pos);
                modified.needed_sum += cv;
            }
        } else if modified.face_down_count > 0 {
            modified.face_down_count -= 1;
        }

        score += value_helps_line(new_value, &modified, neg_min, pos_max);
    }

    // Bonus for replacing a bad face-up card with a better one
    if let Some(gc) = grid.get(row, col) {
        if gc.face_up {
            let old_abs = card_abs_value(&gc.card);
            let new_abs = card_abs_value(new_card);
            if new_abs < old_abs {
                score += (old_abs - new_abs) as f64 * 2.0;
            }
        }
    }

    score
}

/// Pick best replacement when all cards are face-up.
fn pick_best_replace(
    config: &PlayerConfig,
    drawn_card: &Card,
    grid: &PlayerGrid,
    occupied: &[(usize, usize)],
    neg_min: i32,
    pos_max: i32,
    rng: &mut impl Rng,
) -> TurnAction {
    if occupied.is_empty() {
        return TurnAction::ReplaceCard { row: 0, col: 0 };
    }

    let is_wild = matches!(drawn_card, Card::Wild);

    if rng.gen_bool(config.skill /* TODO(task-3): replace with archetype logic */.max(0.1)) {
        let lines = analyze_lines(grid);
        let mut best_score = f64::NEG_INFINITY;
        let mut best_pos = occupied[0];

        for &(r, c) in occupied {
            if !is_wild {
                if let Some(gc) = grid.get(r, c) {
                    if matches!(gc.card, Card::Wild) { continue; }
                }
            }
            let s = score_placement(grid, r, c, drawn_card, &lines, neg_min, pos_max);
            if s > best_score {
                best_score = s;
                best_pos = (r, c);
            }
        }
        TurnAction::ReplaceCard { row: best_pos.0, col: best_pos.1 }
    } else {
        // Replace worst card by absolute value, but not Wild
        let mut worst_pos = occupied[0];
        let mut worst_val = 0i32;
        for &(r, c) in occupied {
            if let Some(gc) = grid.get(r, c) {
                if !is_wild && matches!(gc.card, Card::Wild) { continue; }
                let abs = gc.card.score_value();
                if abs >= worst_val {
                    worst_val = abs;
                    worst_pos = (r, c);
                }
            }
        }
        TurnAction::ReplaceCard { row: worst_pos.0, col: worst_pos.1 }
    }
}

/// Score overall elimination potential of a grid.
fn score_grid_potential(grid: &PlayerGrid) -> f64 {
    let lines = analyze_lines(grid);
    let mut total = 0.0f64;
    for line in &lines {
        total += (line.total_slots as f64) / (line.face_down_count as f64 + 1.0);
    }
    total
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::config::PlayerConfig;

    fn skilled_config() -> PlayerConfig {
        // TODO(task-3): update once Opportunist/Calculator strategy logic is wired
        PlayerConfig::expert()
    }

    fn unskilled_config() -> PlayerConfig {
        // TODO(task-3): update once Opportunist strategy logic is wired
        PlayerConfig::beginner()
    }

    fn make_grid_all_face_up(values: &[i32]) -> PlayerGrid {
        assert_eq!(values.len(), 16);
        let cards: Vec<Card> = values.iter().map(|&v| Card::Number(v)).collect();
        let mut grid = PlayerGrid::new_no_flips(cards);
        for r in 0..4 { for c in 0..4 { grid.flip_card(r, c); } }
        grid
    }

    #[test]
    fn test_wild_always_taken_from_discard() {
        let config = skilled_config();
        let mut rng = rand::thread_rng();
        let grid = make_grid_all_face_up(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 1, 2, 3, 4, 5, 6]);
        for _ in 0..20 {
            let result = choose_draw_source(&config, Some(&Card::Wild), &grid, -5, 10, &mut rng);
            assert_eq!(result, DrawSource::DiscardPile);
        }
    }

    #[test]
    fn test_keepable_card_taken_from_discard() {
        let config = skilled_config(); // threshold = 4
        let mut rng = rand::thread_rng();
        let grid = make_grid_all_face_up(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 1, 2, 3, 4, 5, 6]);
        // |3| <= 4, so should take
        for _ in 0..20 {
            let result = choose_draw_source(&config, Some(&Card::Number(3)), &grid, -5, 10, &mut rng);
            assert_eq!(result, DrawSource::DiscardPile);
        }
    }

    #[test]
    fn test_discard_from_eliminated_avoids_wild() {
        let config = skilled_config();
        let mut rng = rand::thread_rng();
        let cards = vec![Card::Wild, Card::Number(10), Card::Number(1)];
        for _ in 0..20 {
            let idx = choose_discard_from_eliminated(&config, &cards, &mut rng);
            assert_eq!(idx, 1);
        }
    }

    #[test]
    fn test_never_replaces_wild_with_number() {
        let config = skilled_config();
        let mut rng = rand::thread_rng();
        let mut grid = make_grid_all_face_up(&[0, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10]);
        grid.replace_card(0, 0, Card::Wild);
        for _ in 0..50 {
            let action = choose_action(&config, &Card::Number(5), &grid, -5, 10, &mut rng);
            if let TurnAction::ReplaceCard { row, col } = action {
                assert!(!(row == 0 && col == 0), "Should never replace Wild card");
            }
        }
    }

    #[test]
    fn test_analyze_lines_basic() {
        let grid = make_grid_all_face_up(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 1, 2, 3, 4, 5, 6]);
        let lines = analyze_lines(&grid);
        assert_eq!(lines.len(), 10);
        for line in &lines { assert_eq!(line.face_down_count, 0); }
    }
}
