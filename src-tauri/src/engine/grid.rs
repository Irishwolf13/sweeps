use rand::seq::SliceRandom;
use rand::Rng;

use super::card::Card;

#[derive(Clone, Debug)]
pub struct GridCell {
    pub card: Card,
    pub face_up: bool,
}

/// A player's card grid. Starts as 4x4 but rows may have different widths
/// after diagonal elimination and reshaping.
#[derive(Clone, Debug)]
pub struct PlayerGrid {
    /// cells[row][col], None means the position has been eliminated
    cells: Vec<Vec<Option<GridCell>>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EliminationType {
    Row(usize),
    Column(usize),
    MainDiagonal,
    AntiDiagonal,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EliminationReason {
    SumToZero,
    AllMatching,
}

#[derive(Clone, Debug)]
pub struct Elimination {
    pub kind: EliminationType,
    pub positions: Vec<(usize, usize)>,
    pub reason: EliminationReason,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SlideDirection {
    Horizontal,
    Vertical,
}

impl PlayerGrid {
    /// Create a new 4x4 grid from 16 cards, all face-down, then flip
    /// `initial_face_up` random cards.
    pub fn new(cards: Vec<Card>, initial_face_up: usize, rng: &mut impl Rng) -> Self {
        assert!(cards.len() == 16, "Grid requires exactly 16 cards");

        let mut cells: Vec<Vec<Option<GridCell>>> = Vec::with_capacity(4);
        let mut idx = 0;
        for _ in 0..4 {
            let mut row = Vec::with_capacity(4);
            for _ in 0..4 {
                row.push(Some(GridCell {
                    card: cards[idx].clone(),
                    face_up: false,
                }));
                idx += 1;
            }
            cells.push(row);
        }

        let mut grid = PlayerGrid { cells };

        // Flip random cards face-up
        let mut positions: Vec<(usize, usize)> = (0..4)
            .flat_map(|r| (0..4).map(move |c| (r, c)))
            .collect();
        positions.shuffle(rng);
        for &(r, c) in positions.iter().take(initial_face_up) {
            if let Some(ref mut cell) = grid.cells[r][c] {
                cell.face_up = true;
            }
        }

        grid
    }

    pub fn row_count(&self) -> usize {
        self.cells.len()
    }

    pub fn col_count(&self, row: usize) -> usize {
        if row < self.cells.len() {
            self.cells[row].len()
        } else {
            0
        }
    }

    /// Maximum column width across all rows.
    pub fn max_cols(&self) -> usize {
        self.cells.iter().map(|r| r.len()).max().unwrap_or(0)
    }

    /// True if the grid is a perfect square (all rows same width, row_count == that width).
    pub fn is_square(&self) -> bool {
        let rows = self.row_count();
        if rows == 0 {
            return false;
        }
        let cols = self.cells[0].len();
        if rows != cols {
            return false;
        }
        self.cells.iter().all(|r| r.len() == cols)
    }

    pub fn get(&self, row: usize, col: usize) -> Option<&GridCell> {
        self.cells
            .get(row)
            .and_then(|r| r.get(col))
            .and_then(|c| c.as_ref())
    }

    pub fn get_mut(&mut self, row: usize, col: usize) -> Option<&mut GridCell> {
        self.cells
            .get_mut(row)
            .and_then(|r| r.get_mut(col))
            .and_then(|c| c.as_mut())
    }

    /// Replace a card at the given position. Sets face_up = true.
    /// Returns the old card, or None if position was empty.
    pub fn replace_card(&mut self, row: usize, col: usize, new_card: Card) -> Option<Card> {
        if let Some(slot) = self.cells.get_mut(row).and_then(|r| r.get_mut(col)) {
            let old = slot.take();
            *slot = Some(GridCell {
                card: new_card,
                face_up: true,
            });
            old.map(|c| c.card)
        } else {
            None
        }
    }

    /// Flip a face-down card face-up. Returns true if it was flipped.
    pub fn flip_card(&mut self, row: usize, col: usize) -> bool {
        if let Some(ref mut cell) = self.cells.get_mut(row).and_then(|r| r.get_mut(col)) {
            if let Some(ref mut gc) = cell {
                if !gc.face_up {
                    gc.face_up = true;
                    return true;
                }
            }
        }
        false
    }

    /// Count of non-None cells in the grid.
    pub fn remaining_card_count(&self) -> usize {
        self.cells
            .iter()
            .flat_map(|r| r.iter())
            .filter(|c| c.is_some())
            .count()
    }

    /// True if all remaining cards are face-up.
    pub fn all_face_up(&self) -> bool {
        self.cells
            .iter()
            .flat_map(|r| r.iter())
            .filter_map(|c| c.as_ref())
            .all(|gc| gc.face_up)
    }

    /// Positions of face-down cards.
    pub fn face_down_positions(&self) -> Vec<(usize, usize)> {
        let mut positions = Vec::new();
        for (r, row) in self.cells.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                if let Some(gc) = cell {
                    if !gc.face_up {
                        positions.push((r, c));
                    }
                }
            }
        }
        positions
    }

    /// Positions of all non-None cells.
    pub fn occupied_positions(&self) -> Vec<(usize, usize)> {
        let mut positions = Vec::new();
        for (r, row) in self.cells.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                if cell.is_some() {
                    positions.push((r, c));
                }
            }
        }
        positions
    }

    /// Find all valid eliminations in the grid.
    /// A line is eligible only if all cells are present (Some) and face-up.
    pub fn find_eliminations(
        &self,
        allow_matching: bool,
        allow_diagonal: bool,
        neg_min: i32,
        pos_max: i32,
    ) -> Vec<Elimination> {
        let mut eliminations = Vec::new();

        // Check rows
        for r in 0..self.row_count() {
            let cols = self.col_count(r);
            if cols < 2 {
                continue;
            }
            let positions: Vec<(usize, usize)> = (0..cols).map(|c| (r, c)).collect();
            let cards = self.collect_line_cards(&positions);
            if let Some(cards) = cards {
                if let Some(reason) =
                    check_elimination(&cards, allow_matching, neg_min, pos_max)
                {
                    eliminations.push(Elimination {
                        kind: EliminationType::Row(r),
                        positions,
                        reason,
                    });
                }
            }
        }

        // Check columns
        let max_cols = self.max_cols();
        for c in 0..max_cols {
            let positions: Vec<(usize, usize)> = (0..self.row_count())
                .filter(|&r| c < self.col_count(r))
                .map(|r| (r, c))
                .collect();
            if positions.len() < 2 {
                continue;
            }
            // A column is only eligible if ALL rows that have this column index
            // have a card present there
            let cards = self.collect_line_cards(&positions);
            if let Some(cards) = cards {
                if let Some(reason) =
                    check_elimination(&cards, allow_matching, neg_min, pos_max)
                {
                    eliminations.push(Elimination {
                        kind: EliminationType::Column(c),
                        positions,
                        reason,
                    });
                }
            }
        }

        // Check diagonals (only if grid is square)
        if allow_diagonal && self.is_square() {
            let n = self.row_count();
            if n >= 2 {
                // Main diagonal
                let positions: Vec<(usize, usize)> = (0..n).map(|i| (i, i)).collect();
                let cards = self.collect_line_cards(&positions);
                if let Some(cards) = cards {
                    if let Some(reason) =
                        check_elimination(&cards, allow_matching, neg_min, pos_max)
                    {
                        eliminations.push(Elimination {
                            kind: EliminationType::MainDiagonal,
                            positions,
                            reason,
                        });
                    }
                }

                // Anti-diagonal
                let positions: Vec<(usize, usize)> =
                    (0..n).map(|i| (i, n - 1 - i)).collect();
                let cards = self.collect_line_cards(&positions);
                if let Some(cards) = cards {
                    if let Some(reason) =
                        check_elimination(&cards, allow_matching, neg_min, pos_max)
                    {
                        eliminations.push(Elimination {
                            kind: EliminationType::AntiDiagonal,
                            positions,
                            reason,
                        });
                    }
                }
            }
        }

        eliminations
    }

    /// Collect cards from the given positions. Returns None if any position
    /// is empty (None cell) or face-down.
    fn collect_line_cards(&self, positions: &[(usize, usize)]) -> Option<Vec<&Card>> {
        let mut cards = Vec::with_capacity(positions.len());
        for &(r, c) in positions {
            match self.get(r, c) {
                Some(gc) if gc.face_up => cards.push(&gc.card),
                _ => return None,
            }
        }
        Some(cards)
    }

    /// Remove cards at the given positions. Returns the removed cards.
    pub fn eliminate(&mut self, positions: &[(usize, usize)]) -> Vec<Card> {
        let mut removed = Vec::with_capacity(positions.len());
        for &(r, c) in positions {
            if let Some(slot) = self.cells.get_mut(r).and_then(|row| row.get_mut(c)) {
                if let Some(cell) = slot.take() {
                    removed.push(cell.card);
                }
            }
        }
        removed
    }

    /// After a diagonal elimination, reshape the grid by sliding the two
    /// halves together. The grid becomes rectangular (not square).
    ///
    /// For a main diagonal elimination on an NxN grid, the remaining cells
    /// form two triangular groups. Sliding them together produces a rectangular
    /// grid where rows have N-1 or N cells depending on direction.
    pub fn reshape_after_diagonal(
        &mut self,
        _eliminated_kind: &EliminationType,
        direction: SlideDirection,
    ) {
        // Collect all remaining cards in row-major order
        let _remaining: Vec<GridCell> = Vec::new();

        // For simplicity: collect all remaining cells preserving row structure,
        // then rebuild the grid by removing None gaps.
        let mut new_cells: Vec<Vec<Option<GridCell>>> = Vec::new();

        match direction {
            SlideDirection::Horizontal => {
                // Keep same number of rows, remove None gaps within each row
                for row in &self.cells {
                    let compacted: Vec<Option<GridCell>> = row
                        .iter()
                        .filter(|c| c.is_some())
                        .cloned()
                        .collect();
                    if !compacted.is_empty() {
                        new_cells.push(compacted);
                    }
                }
            }
            SlideDirection::Vertical => {
                // Keep same number of columns, remove None gaps within each column
                let max_cols = self.max_cols();
                // Collect columns
                let mut columns: Vec<Vec<Option<GridCell>>> = vec![Vec::new(); max_cols];
                for row in &self.cells {
                    for (c, cell) in row.iter().enumerate() {
                        if cell.is_some() {
                            columns[c].push(cell.clone());
                        }
                    }
                }
                // Rebuild rows from columns
                let max_col_len = columns.iter().map(|c| c.len()).max().unwrap_or(0);
                for r in 0..max_col_len {
                    let mut row = Vec::new();
                    for col in &columns {
                        if r < col.len() {
                            row.push(col[r].clone());
                        }
                    }
                    if !row.is_empty() {
                        new_cells.push(row);
                    }
                }
            }
        }

        // Remove any completely empty rows
        new_cells.retain(|row| row.iter().any(|c| c.is_some()));

        self.cells = new_cells;
    }

    /// Remove all empty rows and compact columns (remove None gaps within rows).
    /// This ensures the grid stays tight after any elimination.
    pub fn cleanup(&mut self) {
        // Remove empty rows
        self.cells.retain(|row| row.iter().any(|c| c.is_some()));

        // Compact each row: remove None gaps so columns squeeze together
        for row in &mut self.cells {
            row.retain(|c| c.is_some());
        }
    }
}

/// Check if a line of cards qualifies for elimination.
/// Returns the reason if it does, or None if not.
fn check_elimination(
    cards: &[&Card],
    allow_matching: bool,
    neg_min: i32,
    pos_max: i32,
) -> Option<EliminationReason> {
    if cards.is_empty() {
        return None;
    }

    // Check sum-to-zero
    if check_sum_to_zero(cards, neg_min, pos_max) {
        return Some(EliminationReason::SumToZero);
    }

    // Check all-matching
    if allow_matching && check_all_matching(cards) {
        return Some(EliminationReason::AllMatching);
    }

    None
}

/// Check if the cards can sum to zero, considering Wild cards can take
/// any integer value in [neg_min, pos_max].
fn check_sum_to_zero(cards: &[&Card], neg_min: i32, pos_max: i32) -> bool {
    let mut number_sum: i32 = 0;
    let mut wild_count: u32 = 0;
    for card in cards {
        match card {
            Card::Number(v) => number_sum += v,
            Card::Wild => wild_count += 1,
        }
    }

    if wild_count == 0 {
        return number_sum == 0;
    }

    // Wilds need to contribute -number_sum total.
    // Each wild can be [neg_min, pos_max].
    let needed = -number_sum;
    let min_possible = (wild_count as i32) * neg_min;
    let max_possible = (wild_count as i32) * pos_max;

    needed >= min_possible && needed <= max_possible
}

/// Check if all cards are identical (Wild matches anything).
fn check_all_matching(cards: &[&Card]) -> bool {
    let mut target_value: Option<&Card> = None;

    for &card in cards {
        if matches!(card, Card::Wild) {
            continue;
        }
        match target_value {
            Some(existing) if existing != card => return false,
            None => target_value = Some(card),
            _ => {}
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_grid_from_numbers(values: &[i32]) -> PlayerGrid {
        assert_eq!(values.len(), 16);
        let cards: Vec<Card> = values.iter().map(|&v| Card::Number(v)).collect();
        let mut cells = Vec::new();
        for r in 0..4 {
            let mut row = Vec::new();
            for c in 0..4 {
                row.push(Some(GridCell {
                    card: cards[r * 4 + c].clone(),
                    face_up: true,
                }));
            }
            cells.push(row);
        }
        PlayerGrid { cells }
    }

    #[test]
    fn test_row_sum_to_zero() {
        // Row 0: -3 + 1 + 2 + 0 = 0
        let grid = make_grid_from_numbers(&[-3, 1, 2, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 1, 2]);
        let elims = grid.find_eliminations(true, false, -5, 10);
        assert!(!elims.is_empty());
        assert_eq!(elims[0].kind, EliminationType::Row(0));
        assert_eq!(elims[0].reason, EliminationReason::SumToZero);
    }

    #[test]
    fn test_column_sum_to_zero() {
        // Col 0: 1 + (-1) + 2 + (-2) = 0
        let grid = make_grid_from_numbers(&[1, 9, 9, 9, -1, 9, 9, 9, 2, 9, 9, 9, -2, 9, 9, 9]);
        let elims = grid.find_eliminations(true, false, -5, 10);
        assert!(!elims.is_empty());
        assert_eq!(elims[0].kind, EliminationType::Column(0));
        assert_eq!(elims[0].reason, EliminationReason::SumToZero);
    }

    #[test]
    fn test_all_matching_row() {
        // Row 0: all 5s
        let grid = make_grid_from_numbers(&[5, 5, 5, 5, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 1, 2]);
        let elims = grid.find_eliminations(true, false, -5, 10);
        assert!(!elims.is_empty());
        let matching_elim = elims
            .iter()
            .find(|e| e.reason == EliminationReason::AllMatching);
        assert!(matching_elim.is_some());
    }

    #[test]
    fn test_matching_disabled() {
        // Row 0: all 5s, but matching is disabled
        let grid = make_grid_from_numbers(&[5, 5, 5, 5, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 1, 2]);
        let elims = grid.find_eliminations(false, false, -5, 10);
        // Should not find the matching elimination (5+5+5+5=20, not 0)
        let matching_elim = elims
            .iter()
            .find(|e| e.reason == EliminationReason::AllMatching);
        assert!(matching_elim.is_none());
    }

    #[test]
    fn test_face_down_blocks_elimination() {
        // Row 0 sums to 0 but one card is face-down
        let mut grid =
            make_grid_from_numbers(&[-3, 1, 2, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 1, 2]);
        if let Some(cell) = grid.cells[0][1].as_mut() {
            cell.face_up = false;
        }
        let elims = grid.find_eliminations(true, false, -5, 10);
        let row0 = elims.iter().find(|e| e.kind == EliminationType::Row(0));
        assert!(row0.is_none());
    }

    #[test]
    fn test_wild_enables_sum_to_zero() {
        // Row 0: Wild, 3, -1, -2 → Wild needs to be 0 to sum to 0. Valid since 0 is in [-5, 10]
        let mut grid =
            make_grid_from_numbers(&[0, 3, -1, -2, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 1, 2]);
        grid.cells[0][0] = Some(GridCell {
            card: Card::Wild,
            face_up: true,
        });
        let elims = grid.find_eliminations(true, false, -5, 10);
        let row0 = elims.iter().find(|e| e.kind == EliminationType::Row(0));
        assert!(row0.is_some());
        assert_eq!(row0.unwrap().reason, EliminationReason::SumToZero);
    }

    #[test]
    fn test_wild_enables_matching() {
        // Row 0: Wild, 5, 5, 5 → Wild matches 5
        let mut grid =
            make_grid_from_numbers(&[0, 5, 5, 5, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 1, 2]);
        grid.cells[0][0] = Some(GridCell {
            card: Card::Wild,
            face_up: true,
        });
        let elims = grid.find_eliminations(true, false, -5, 10);
        let matching = elims
            .iter()
            .find(|e| e.reason == EliminationReason::AllMatching && e.kind == EliminationType::Row(0));
        assert!(matching.is_some());
    }

    #[test]
    fn test_is_square() {
        let grid = make_grid_from_numbers(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 1, 2, 3, 4, 5, 6]);
        assert!(grid.is_square());
    }

    #[test]
    fn test_remaining_and_eliminate() {
        let mut grid =
            make_grid_from_numbers(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 1, 2, 3, 4, 5, 6]);
        assert_eq!(grid.remaining_card_count(), 16);

        let removed = grid.eliminate(&[(0, 0), (0, 1), (0, 2), (0, 3)]);
        assert_eq!(removed.len(), 4);
        assert_eq!(grid.remaining_card_count(), 12);
    }

    #[test]
    fn test_diagonal_check_only_on_square() {
        // Diagonal: 1 + (-1) + 0 + 0 = 0
        let grid = make_grid_from_numbers(&[1, 9, 9, 9, 9, -1, 9, 9, 9, 9, 0, 9, 9, 9, 9, 0]);
        let elims = grid.find_eliminations(true, true, -5, 10);
        let diag = elims
            .iter()
            .find(|e| matches!(e.kind, EliminationType::MainDiagonal));
        assert!(diag.is_some());

        // Disable diagonals
        let elims = grid.find_eliminations(true, false, -5, 10);
        let diag = elims
            .iter()
            .find(|e| matches!(e.kind, EliminationType::MainDiagonal));
        assert!(diag.is_none());
    }

    #[test]
    fn test_reshape_horizontal() {
        let mut grid =
            make_grid_from_numbers(&[0, 2, 3, 4, 5, 0, 7, 8, 9, 10, 0, 1, 2, 3, 4, 0]);
        // Eliminate main diagonal (positions (0,0), (1,1), (2,2), (3,3))
        grid.eliminate(&[(0, 0), (1, 1), (2, 2), (3, 3)]);
        assert_eq!(grid.remaining_card_count(), 12);

        grid.reshape_after_diagonal(&EliminationType::MainDiagonal, SlideDirection::Horizontal);

        // After horizontal slide, each row should have its None gaps removed
        assert_eq!(grid.remaining_card_count(), 12);
        // Row 0 had [None, 2, 3, 4] → now [2, 3, 4] (3 cells)
        assert_eq!(grid.col_count(0), 3);
    }
}
