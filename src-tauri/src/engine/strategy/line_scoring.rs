use super::super::card::Card;
use super::super::grid::PlayerGrid;

// ── LineStatus ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct LineStatus {
    pub positions: Vec<(usize, usize)>,
    pub face_up_count: usize,
    pub face_down_count: usize,
    pub current_sum: i32,
    pub wild_count: usize,
    pub gap: i32,
    pub gap_achievable: bool,
    pub cards_needed: usize,
    pub matching_value: Option<i32>,
    pub matching_viable: bool,
}

// ── Core scoring functions ────────────────────────────────────────────────

/// Score all lines (rows, columns, diagonals) in the grid.
/// Returns each line's status and a score from 0-100.
pub fn score_all_lines(grid: &PlayerGrid, neg_min: i32, pos_max: i32) -> Vec<(LineStatus, f64)> {
    let mut results = Vec::new();

    // Rows
    for r in 0..grid.row_count() {
        let cols = grid.col_count(r);
        if cols < 2 { continue; }
        let positions: Vec<(usize, usize)> = (0..cols).map(|c| (r, c)).collect();
        let status = analyze_line(grid, &positions, neg_min, pos_max);
        let score = score_line(&status);
        results.push((status, score));
    }

    // Columns
    let max_cols = grid.max_cols();
    for c in 0..max_cols {
        let positions: Vec<(usize, usize)> = (0..grid.row_count())
            .filter(|&r| c < grid.col_count(r))
            .map(|r| (r, c))
            .collect();
        if positions.len() < 2 { continue; }
        let status = analyze_line(grid, &positions, neg_min, pos_max);
        let score = score_line(&status);
        results.push((status, score));
    }

    // Diagonals (only if square)
    if grid.is_square() {
        let n = grid.row_count();
        if n >= 2 {
            let main_diag: Vec<(usize, usize)> = (0..n).map(|i| (i, i)).collect();
            let status = analyze_line(grid, &main_diag, neg_min, pos_max);
            let score = score_line(&status);
            results.push((status, score));

            let anti_diag: Vec<(usize, usize)> = (0..n).map(|i| (i, n - 1 - i)).collect();
            let status = analyze_line(grid, &anti_diag, neg_min, pos_max);
            let score = score_line(&status);
            results.push((status, score));
        }
    }

    results
}

/// How well does placing a card with this value help a specific line?
/// Returns 0-100. 100 = completes the line.
pub fn card_fits_line(card_value: i32, line: &LineStatus, neg_min: i32, pos_max: i32) -> f64 {
    if line.face_down_count == 0 {
        // Line is fully visible. Card can only help by replacing an existing card.
        // This function evaluates adding to a face-down slot, so return 0.
        return 0.0;
    }

    // After placing this card in a face-down slot:
    let new_gap = line.gap - card_value;
    let remaining_unknowns = line.face_down_count - 1;

    // Would complete the line?
    if remaining_unknowns == 0 {
        let wilds = line.wild_count;
        if wilds == 0 && new_gap == 0 { return 100.0; }
        if wilds > 0 {
            let min_p = (wilds as i32) * neg_min;
            let max_p = (wilds as i32) * pos_max;
            if new_gap >= min_p && new_gap <= max_p { return 100.0; }
        }
        return 0.0; // Wouldn't complete
    }

    // Partial progress: check if line stays viable
    let total_unknowns = remaining_unknowns + line.wild_count;
    let min_p = (total_unknowns as i32) * neg_min;
    let max_p = (total_unknowns as i32) * pos_max;
    if new_gap < min_p || new_gap > max_p {
        return 0.0; // Line becomes hopeless
    }

    // Score based on how close we are AND how well this card fits
    let total_slots = line.positions.len();
    let known_after = total_slots - remaining_unknowns;
    let progress = known_after as f64 / total_slots as f64;

    // Matching bonus
    if line.matching_viable {
        if let Some(mv) = line.matching_value {
            if card_value == mv {
                return 40.0 + progress * 40.0;
            }
        }
    }

    // Gap-aware scoring: how close is new_gap to 0 relative to the achievable range?
    // A card that brings gap closer to 0 scores much higher than one that barely keeps it viable.
    let range = (max_p - min_p) as f64;
    let gap_closeness = if range > 0.0 {
        // 1.0 when new_gap == 0 (perfect), 0.0 when at edge of range
        1.0 - (new_gap.abs() as f64 / (range / 2.0)).min(1.0)
    } else {
        if new_gap == 0 { 1.0 } else { 0.0 }
    };

    // Base score from progress + bonus from gap quality
    let base = 10.0 + progress * 30.0;
    let gap_bonus = gap_closeness * 30.0;
    base + gap_bonus
}

/// Find the best position to place a card, considering net impact on all lines.
/// Returns ((row, col), net_score). Considers both face-down and face-up positions.
pub fn best_placement(
    card: &Card,
    grid: &PlayerGrid,
    neg_min: i32,
    pos_max: i32,
) -> ((usize, usize), f64) {
    let card_value = match card {
        Card::Number(v) => *v,
        Card::Wild => 0,
    };
    let is_wild = matches!(card, Card::Wild);

    let lines = score_all_lines(grid, neg_min, pos_max);
    let occupied = grid.occupied_positions();

    let mut best_pos = occupied.first().copied().unwrap_or((0, 0));
    let mut best_score = f64::NEG_INFINITY;

    for &(r, c) in &occupied {
        // Don't replace a Wild with a non-Wild
        if !is_wild {
            if let Some(gc) = grid.get(r, c) {
                if gc.face_up && matches!(gc.card, Card::Wild) {
                    continue;
                }
            }
        }

        let mut score = 0.0f64;
        let mut completes_a_line = false;

        for (line, _current_score) in &lines {
            if !line.positions.contains(&(r, c)) { continue; }

            let is_face_down = grid.get(r, c).map_or(false, |gc| !gc.face_up);

            if is_face_down {
                // Placing in a face-down slot: evaluate how card fits
                let fit = card_fits_line(card_value, line, neg_min, pos_max);
                if fit >= 100.0 { completes_a_line = true; }
                score += fit;
            } else {
                // Replacing a face-up card: evaluate improvement
                let old_value = grid.get(r, c).map_or(0, |gc| match &gc.card {
                    Card::Number(v) => *v,
                    Card::Wild => 0,
                });
                // Simple heuristic: how much closer does this get the line sum to zero?
                let old_gap_contribution = old_value;
                let new_gap_contribution = card_value;
                let gap_improvement = (line.gap + old_gap_contribution - new_gap_contribution).abs() as f64;
                let gap_distance = (line.gap).abs() as f64;
                if gap_improvement < gap_distance {
                    score += 20.0 + (gap_distance - gap_improvement) * 5.0;
                }
            }
        }

        // Bonus: replacing a high-value face-up card with a low-value card
        if let Some(gc) = grid.get(r, c) {
            if gc.face_up {
                let old_abs = match &gc.card { Card::Number(v) => v.abs(), Card::Wild => 0 };
                let new_abs = card_value.abs();
                if new_abs < old_abs {
                    score += (old_abs - new_abs) as f64 * 2.0;
                }
            }
        }

        // Line completion is always the top priority: add a large bonus to dominate
        if completes_a_line {
            score += 500.0;
        }

        if score > best_score {
            best_score = score;
            best_pos = (r, c);
        }
    }

    (best_pos, best_score)
}

/// What specific card values would complete this line?
/// Only meaningful when face_down_count == 1. Returns empty otherwise.
pub fn needed_cards(line: &LineStatus, neg_min: i32, pos_max: i32) -> Vec<i32> {
    if line.face_down_count != 1 { return Vec::new(); }

    let wilds = line.wild_count;
    if wilds == 0 {
        // Need exactly -gap
        let needed = -line.gap;
        if needed >= neg_min && needed <= pos_max {
            return vec![needed];
        }
        return Vec::new();
    }

    // With wilds, a range of values could work
    // The placed card + wilds need to sum to -current_sum
    // placed_value + wild_sum = -current_sum → placed_value = gap - wild_sum
    let mut values = Vec::new();
    let wild_min = (wilds as i32) * neg_min;
    let wild_max = (wilds as i32) * pos_max;
    for wild_sum in wild_min..=wild_max {
        let needed = line.gap - wild_sum;
        if needed >= neg_min && needed <= pos_max && !values.contains(&needed) {
            values.push(needed);
        }
    }
    values
}

// ── Internal helpers ──────────────────────────────────────────────────────

fn analyze_line(grid: &PlayerGrid, positions: &[(usize, usize)], neg_min: i32, pos_max: i32) -> LineStatus {
    let mut face_up_count = 0usize;
    let mut face_down_count = 0usize;
    let mut current_sum = 0i32;
    let mut wild_count = 0usize;
    let mut number_values: Vec<i32> = Vec::new();

    for &(r, c) in positions {
        match grid.get(r, c) {
            Some(gc) if gc.face_up => {
                face_up_count += 1;
                match &gc.card {
                    Card::Number(v) => {
                        current_sum += v;
                        number_values.push(*v);
                    }
                    Card::Wild => wild_count += 1,
                }
            }
            Some(_) => face_down_count += 1,
            None => {} // eliminated position
        }
    }

    let gap = -current_sum;
    let total_unknowns = face_down_count + wild_count;
    let gap_achievable = if total_unknowns == 0 {
        gap == 0
    } else {
        let min_p = (total_unknowns as i32) * neg_min;
        let max_p = (total_unknowns as i32) * pos_max;
        gap >= min_p && gap <= max_p
    };

    let (matching_viable, matching_value) = if number_values.is_empty() {
        (true, None) // All wilds or all face-down: matching still possible
    } else {
        let first = number_values[0];
        let all_same = number_values.iter().all(|&v| v == first);
        (all_same, if all_same { Some(first) } else { None })
    };

    LineStatus {
        positions: positions.to_vec(),
        face_up_count,
        face_down_count,
        current_sum,
        wild_count,
        gap,
        gap_achievable,
        cards_needed: face_down_count,
        matching_value,
        matching_viable,
    }
}

fn score_line(status: &LineStatus) -> f64 {
    let total = status.positions.len();
    if total == 0 { return 0.0; }

    match status.face_down_count {
        0 => {
            // All face-up. Completable via sum-to-zero OR via all-matching.
            if status.gap_achievable {
                return 100.0;
            }
            // Not completable via sum-to-zero; check matching path.
            // All face-up values must already be identical (matching_viable handles this).
            if status.matching_viable && status.matching_value.is_some() {
                return 100.0;
            }
            0.0
        }
        1 => {
            // One card away. Score 70-90 based on line length (shorter = easier).
            let base = 70.0;
            let length_bonus = if total <= 2 { 20.0 } else if total <= 3 { 15.0 } else { 10.0 };
            // Matching bonus: viable via all-matching even when sum-to-zero is hopeless.
            let matching_bonus = if status.matching_viable { 10.0 } else { 0.0 };

            if !status.gap_achievable {
                // Sum-to-zero is hopeless; only the matching path can save this line.
                if status.matching_viable {
                    return base + length_bonus + matching_bonus;
                }
                return 0.0;
            }
            base + length_bonus + matching_bonus
        }
        2 => {
            // Two away. Score 30-60 based on gap range achievability.
            if !status.gap_achievable && !status.matching_viable {
                return 0.0;
            }
            let base = 30.0;
            let progress = (total - 2) as f64 / total as f64;
            // Small matching bonus for short lines where all-matching is realistic.
            let matching_bonus = if status.matching_viable && total <= 3 { 10.0 } else { 0.0 };
            base + progress * 30.0 + matching_bonus
        }
        _ => {
            if !status.gap_achievable && !status.matching_viable {
                return 0.0;
            }
            // Three or more away. Low but nonzero if achievable.
            let progress = (total - status.face_down_count) as f64 / total as f64;
            5.0 + progress * 15.0
        }
    }
}

/// Pick the best face-down card to flip: prefer cards in high-scoring lines.
pub fn best_flip_target(
    face_down: &[(usize, usize)],
    lines: &[(LineStatus, f64)],
) -> (usize, usize) {
    let mut best_pos = face_down[0];
    let mut best_score = f64::NEG_INFINITY;

    for &(r, c) in face_down {
        let mut score = 0.0f64;
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

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::grid::PlayerGrid;
    use super::super::super::card::Card;

    fn make_grid_all_face_up(values: &[i32]) -> PlayerGrid {
        assert_eq!(values.len(), 16);
        let cards: Vec<Card> = values.iter().map(|&v| Card::Number(v)).collect();
        let mut grid = PlayerGrid::new_no_flips(cards);
        for r in 0..4 { for c in 0..4 { grid.flip_card(r, c); } }
        grid
    }

    #[test]
    fn test_score_completable_line() {
        // Row 0: -3 + 1 + 2 + 0 = 0 → completable (all face up, sums to zero)
        let grid = make_grid_all_face_up(&[-3, 1, 2, 0, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5]);
        let lines = score_all_lines(&grid, -5, 8);
        let row0 = &lines[0]; // First line is row 0
        assert!(row0.1 >= 99.0, "Completable line should score ~100, got {}", row0.1);
    }

    #[test]
    fn test_score_one_away_line() {
        // Row 0: -3, 1, 2, face_down → needs 0 to complete
        let cards: Vec<Card> = vec![
            Card::Number(-3), Card::Number(1), Card::Number(2), Card::Number(7),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
        ];
        let mut grid = PlayerGrid::new_no_flips(cards);
        // Flip first 3 in row 0, leave (0,3) face-down
        grid.flip_card(0, 0); grid.flip_card(0, 1); grid.flip_card(0, 2);
        for r in 1..4 { for c in 0..4 { grid.flip_card(r, c); } }

        let lines = score_all_lines(&grid, -5, 8);
        let row0 = &lines[0];
        assert_eq!(row0.0.face_down_count, 1);
        assert!(row0.1 >= 70.0 && row0.1 <= 95.0, "One-away should score 70-90, got {}", row0.1);
    }

    #[test]
    fn test_card_fits_line_completes() {
        // Line needs a 0 to complete (gap = 0, 1 face_down)
        let status = LineStatus {
            positions: vec![(0,0), (0,1), (0,2), (0,3)],
            face_up_count: 3,
            face_down_count: 1,
            current_sum: 0,  // -3 + 1 + 2 = 0
            wild_count: 0,
            gap: 0,
            gap_achievable: true,
            cards_needed: 1,
            matching_value: None,
            matching_viable: false,
        };
        assert_eq!(card_fits_line(0, &status, -5, 8), 100.0);
        assert!(card_fits_line(5, &status, -5, 8) < 100.0);
    }

    #[test]
    fn test_needed_cards_single_unknown() {
        let status = LineStatus {
            positions: vec![(0,0), (0,1), (0,2), (0,3)],
            face_up_count: 3,
            face_down_count: 1,
            current_sum: 3,  // e.g., 1+1+1 = 3, need -3
            wild_count: 0,
            gap: -3,
            gap_achievable: true,
            cards_needed: 1,
            matching_value: None,
            matching_viable: false,
        };
        let needed = needed_cards(&status, -5, 8);
        assert_eq!(needed, vec![3]); // need +3 to make gap 0 → actually need value = -gap = 3
    }

    #[test]
    fn test_best_placement_prefers_line_completion() {
        // Grid where placing a 0 at (0,3) completes row 0
        let cards: Vec<Card> = vec![
            Card::Number(-3), Card::Number(1), Card::Number(2), Card::Number(7),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
            Card::Number(5), Card::Number(5), Card::Number(5), Card::Number(5),
        ];
        let mut grid = PlayerGrid::new_no_flips(cards);
        grid.flip_card(0, 0); grid.flip_card(0, 1); grid.flip_card(0, 2);
        for r in 1..4 { for c in 0..4 { grid.flip_card(r, c); } }

        let (pos, score) = best_placement(&Card::Number(0), &grid, -5, 8);
        assert_eq!(pos, (0, 3), "Should place at the face-down slot that completes row 0");
        assert!(score >= 90.0, "Completing a line should score high, got {}", score);
    }

    #[test]
    fn test_hopeless_line_scores_zero() {
        // All face-up, sum = 20, no wilds → not achievable
        let status = LineStatus {
            positions: vec![(0,0), (0,1), (0,2), (0,3)],
            face_up_count: 4,
            face_down_count: 0,
            current_sum: 20,
            wild_count: 0,
            gap: -20,
            gap_achievable: false,
            cards_needed: 0,
            matching_value: None,
            matching_viable: false,
        };
        assert_eq!(score_line(&status), 0.0);
    }

    #[test]
    fn test_matching_line_scores_high() {
        // Row with 3 matching values + 1 face-down should score well
        // even if sum-to-zero is hopeless
        let status = LineStatus {
            positions: vec![(0,0), (0,1), (0,2), (0,3)],
            face_up_count: 3,
            face_down_count: 1,
            current_sum: 15,  // 5+5+5
            wild_count: 0,
            gap: -15,
            gap_achievable: false,  // sum-to-zero is hopeless
            cards_needed: 1,
            matching_value: Some(5),
            matching_viable: true,
        };
        let score = score_line(&status);
        assert!(score >= 50.0, "Matching-viable line should score well, got {}", score);
    }
}
