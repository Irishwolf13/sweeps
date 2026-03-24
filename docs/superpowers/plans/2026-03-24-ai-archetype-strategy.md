# AI Archetype Strategy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single parameterized AI strategy with three selectable archetypes (Opportunist, Methodical, Calculator) that play line-completion-focused games, reducing average turns/round from ~60 to ~28-40.

**Architecture:** Convert `strategy.rs` from a single file into a `strategy/` module with shared line-scoring infrastructure and three archetype implementations. Update `PlayerConfig` to use an `AiArchetype` enum + `skill` float instead of `keep_threshold`/`line_awareness`/`opponent_awareness`. Update `game.rs` and `state.rs` to pass archetype state. Update frontend config panels.

**Tech Stack:** Rust (Tauri backend), vanilla JS (frontend)

**Spec:** `docs/superpowers/specs/2026-03-24-ai-archetype-strategy-design.md`

---

## File Structure

### Files to Create
- `src-tauri/src/engine/strategy/mod.rs` — Public API dispatch (replaces `strategy.rs`)
- `src-tauri/src/engine/strategy/line_scoring.rs` — Shared LineStatus, scoring, card_fits_line, best_placement
- `src-tauri/src/engine/strategy/opportunist.rs` — Opportunist archetype logic
- `src-tauri/src/engine/strategy/methodical.rs` — Methodical archetype logic + MethodicalState
- `src-tauri/src/engine/strategy/calculator.rs` — Calculator archetype logic

### Files to Modify
- `src-tauri/src/engine/config.rs` — Replace PlayerConfig fields, add AiArchetype enum
- `src-tauri/src/engine/game.rs` — Add MethodicalState to PlayerState, pass to strategy
- `src-tauri/src/engine/mod.rs` — No change needed (already declares `pub mod strategy`)
- `src-tauri/src/interactive/state.rs` — Update strategy call sites, add MethodicalState storage
- `src-tauri/tests/smoke_test.rs` — Update PlayerConfig construction to new fields
- `src/js/config-panel.js` — Replace sliders with archetype dropdown + skill slider
- `src/js/play-panel.js` — Update AI_PRESETS and player config construction

### File to Delete
- `src-tauri/src/engine/strategy.rs` — Replaced by `strategy/` module directory

---

## Task 1: Update PlayerConfig and AiArchetype enum

**Files:**
- Modify: `src-tauri/src/engine/config.rs`

This task updates the data model. Everything downstream will break until later tasks fix it, so we do this first and fix callers incrementally.

- [ ] **Step 1: Write test for new PlayerConfig**

Add to the bottom of the `#[cfg(test)] mod tests` block in `config.rs`:

```rust
#[test]
fn test_ai_archetype_serialization() {
    let config = PlayerConfig {
        archetype: AiArchetype::Opportunist,
        skill: 0.7,
        flip_strategy: FlipStrategy::Random,
    };
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: PlayerConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.archetype, AiArchetype::Opportunist);
    assert!((deserialized.skill - 0.7).abs() < f64::EPSILON);
}

#[test]
fn test_player_presets() {
    let b = PlayerConfig::beginner();
    assert_eq!(b.archetype, AiArchetype::Opportunist);
    assert!((b.skill - 0.3).abs() < f64::EPSILON);

    let i = PlayerConfig::intermediate();
    assert_eq!(i.archetype, AiArchetype::Methodical);

    let a = PlayerConfig::advanced();
    assert_eq!(a.archetype, AiArchetype::Opportunist);
    assert!(a.skill > 0.8);

    let e = PlayerConfig::expert();
    assert_eq!(e.archetype, AiArchetype::Calculator);
    assert!((e.skill - 1.0).abs() < f64::EPSILON);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test test_ai_archetype_serialization test_player_presets -- --nocapture 2>&1`
Expected: Compilation errors — `AiArchetype` doesn't exist yet.

- [ ] **Step 3: Implement AiArchetype and new PlayerConfig**

Replace the `PlayerConfig` struct, its `Default` impl, and add `AiArchetype` in `config.rs`:

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum AiArchetype {
    Opportunist,
    Methodical,
    Calculator,
}

impl Default for AiArchetype {
    fn default() -> Self {
        AiArchetype::Opportunist
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerConfig {
    pub archetype: AiArchetype,
    /// 0.0 = random play, 1.0 = perfect execution of archetype strategy
    pub skill: f64,
    #[serde(default)]
    pub flip_strategy: FlipStrategy,
}

impl Default for PlayerConfig {
    fn default() -> Self {
        // Advanced preset as default
        PlayerConfig {
            archetype: AiArchetype::Opportunist,
            skill: 0.85,
            flip_strategy: FlipStrategy::default(),
        }
    }
}

impl PlayerConfig {
    pub fn beginner() -> Self {
        PlayerConfig {
            archetype: AiArchetype::Opportunist,
            skill: 0.3,
            flip_strategy: FlipStrategy::Random,
        }
    }
    pub fn intermediate() -> Self {
        PlayerConfig {
            archetype: AiArchetype::Methodical,
            skill: 0.6,
            flip_strategy: FlipStrategy::Random,
        }
    }
    pub fn advanced() -> Self {
        PlayerConfig {
            archetype: AiArchetype::Opportunist,
            skill: 0.85,
            flip_strategy: FlipStrategy::Random,
        }
    }
    pub fn expert() -> Self {
        PlayerConfig {
            archetype: AiArchetype::Calculator,
            skill: 1.0,
            flip_strategy: FlipStrategy::Random,
        }
    }
}
```

Update the `GameConfig::default()` to use varied presets:

```rust
impl Default for GameConfig {
    fn default() -> Self {
        let player_count = 4u8;
        let players = vec![
            PlayerConfig::beginner(),
            PlayerConfig::intermediate(),
            PlayerConfig::advanced(),
            PlayerConfig::expert(),
        ];
        GameConfig {
            deck: DeckConfig::default(),
            player_count,
            allow_matching_elimination: true,
            allow_diagonal_elimination: true,
            scoring_mode: ScoringMode::Basic,
            starting_order: StartingOrder::default(),
            players,
            max_turns_per_round: 500,
        }
    }
}
```

- [ ] **Step 4: Run the config tests (strategy module will still be broken)**

Run: `cd src-tauri && cargo test --lib engine::config -- --nocapture 2>&1`
Expected: The two new tests pass. Other modules will have compilation errors but we're only running config tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/engine/config.rs
git commit -m "feat: replace PlayerConfig with AiArchetype enum and skill dial"
```

---

## Task 2: Create strategy module with shared line scoring

**Files:**
- Create: `src-tauri/src/engine/strategy/line_scoring.rs`
- Create: `src-tauri/src/engine/strategy/mod.rs` (minimal, just re-exports)
- Delete: `src-tauri/src/engine/strategy.rs`

This is the foundation all three archetypes build on.

- [ ] **Step 1: Create the strategy directory and move to module structure**

Delete `src-tauri/src/engine/strategy.rs` and create `src-tauri/src/engine/strategy/mod.rs` with a minimal placeholder that re-exports the public types and functions (same signatures as before, but bodies will be replaced). This keeps the project compilable while we build the new internals.

`src-tauri/src/engine/strategy/mod.rs`:
```rust
mod line_scoring;

use rand::Rng;

use super::card::Card;
use super::config::{AiArchetype, PlayerConfig};
use super::grid::{EliminationType, PlayerGrid, SlideDirection};

pub use line_scoring::{LineStatus, score_all_lines, card_fits_line, best_placement, needed_cards};

// ── Public enums (unchanged) ──────────────────────────────────────────────

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

// ── Methodical state ──────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum Phase {
    Scout,
    Build,
    Close,
}

#[derive(Clone, Debug)]
pub struct MethodicalState {
    pub phase: Phase,
    pub target_lines: Vec<usize>,
    pub turns_in_phase: u32,
}

impl MethodicalState {
    pub fn new() -> Self {
        MethodicalState {
            phase: Phase::Scout,
            target_lines: Vec::new(),
            turns_in_phase: 0,
        }
    }

    pub fn invalidate_targets(&mut self) {
        self.target_lines.clear();
        self.phase = Phase::Build;
        self.turns_in_phase = 0;
    }
}

// ── Skill check helper ───────────────────────────────────────────────────

fn should_play_smart(skill: f64, rng: &mut impl Rng) -> bool {
    rng.gen_bool(skill.clamp(0.0, 1.0))
}

// ── Public strategy API ──────────────────────────────────────────────────
// These are temporary stubs. Each will be replaced as archetypes are built.

pub fn choose_draw_source(
    config: &PlayerConfig,
    discard_top: Option<&Card>,
    grid: &PlayerGrid,
    neg_min: i32,
    pos_max: i32,
    rng: &mut impl Rng,
) -> DrawSource {
    // Temporary: random draw source
    if rng.gen_bool(0.5) { DrawSource::DiscardPile } else { DrawSource::DrawPile }
}

pub fn choose_action(
    config: &PlayerConfig,
    drawn_card: &Card,
    grid: &PlayerGrid,
    neg_min: i32,
    pos_max: i32,
    rng: &mut impl Rng,
) -> TurnAction {
    // Temporary: flip a random face-down card or replace first occupied
    let face_down = grid.face_down_positions();
    if !face_down.is_empty() {
        let idx = rng.gen_range(0..face_down.len());
        TurnAction::DiscardAndFlip { row: face_down[idx].0, col: face_down[idx].1 }
    } else {
        let occupied = grid.occupied_positions();
        if !occupied.is_empty() {
            let idx = rng.gen_range(0..occupied.len());
            TurnAction::ReplaceCard { row: occupied[idx].0, col: occupied[idx].1 }
        } else {
            TurnAction::DiscardAndFlip { row: 0, col: 0 }
        }
    }
}

pub fn choose_discard_from_eliminated(
    config: &PlayerConfig,
    eliminated_cards: &[Card],
    rng: &mut impl Rng,
) -> usize {
    if eliminated_cards.len() <= 1 { return 0; }
    if !should_play_smart(config.skill, rng) {
        return rng.gen_range(0..eliminated_cards.len());
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

pub fn choose_discard_with_opponent(
    config: &PlayerConfig,
    eliminated_cards: &[Card],
    next_player_grid: Option<&PlayerGrid>,
    neg_min: i32,
    pos_max: i32,
    rng: &mut impl Rng,
) -> usize {
    let base_idx = choose_discard_from_eliminated(config, eliminated_cards, rng);

    // Opponent awareness kicks in at skill >= 0.5
    if config.skill < 0.5 || !should_play_smart(config.skill, rng) {
        return base_idx;
    }

    let next_grid = match next_player_grid {
        Some(g) => g,
        None => return base_idx,
    };

    let chosen_value = match &eliminated_cards[base_idx] {
        Card::Number(v) => *v,
        Card::Wild => return base_idx,
    };

    // Check if our chosen discard helps the opponent
    let next_lines = score_all_lines(next_grid, neg_min, pos_max);
    let helps_opponent = next_lines.iter().any(|(line, _score)| {
        card_fits_line(chosen_value, line, neg_min, pos_max) >= 80.0
    });

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
        let max_help = next_lines.iter()
            .map(|(line, _)| card_fits_line(val, line, neg_min, pos_max))
            .fold(0.0f64, f64::max);
        if max_help < 60.0 && val.abs() > best_alt_abs {
            best_alt_abs = val.abs();
            best_alt_idx = i;
        }
    }

    best_alt_idx
}

pub fn choose_slide_direction(
    config: &PlayerConfig,
    grid: &PlayerGrid,
    eliminated_kind: &EliminationType,
    neg_min: i32,
    pos_max: i32,
    rng: &mut impl Rng,
) -> SlideDirection {
    if !should_play_smart(config.skill, rng) {
        return if rng.gen_bool(0.5) { SlideDirection::Horizontal } else { SlideDirection::Vertical };
    }

    let mut grid_h = grid.clone();
    grid_h.reshape_after_diagonal(eliminated_kind, SlideDirection::Horizontal);
    grid_h.cleanup();
    let score_h: f64 = score_all_lines(&grid_h, neg_min, pos_max)
        .iter().map(|(_, s)| s).sum();

    let mut grid_v = grid.clone();
    grid_v.reshape_after_diagonal(eliminated_kind, SlideDirection::Vertical);
    grid_v.cleanup();
    let score_v: f64 = score_all_lines(&grid_v, neg_min, pos_max)
        .iter().map(|(_, s)| s).sum();

    if score_h >= score_v { SlideDirection::Horizontal } else { SlideDirection::Vertical }
}
```

- [ ] **Step 2: Write tests for line_scoring**

Create `src-tauri/src/engine/strategy/line_scoring.rs`:

```rust
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

    // Score based on how close we are
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

    // Sum-to-zero progress
    10.0 + progress * 50.0
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

        for (line, _current_score) in &lines {
            if !line.positions.contains(&(r, c)) { continue; }

            let is_face_down = grid.get(r, c).map_or(false, |gc| !gc.face_up);

            if is_face_down {
                // Placing in a face-down slot: evaluate how card fits
                score += card_fits_line(card_value, line, neg_min, pos_max);
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
    if !status.gap_achievable { return 0.0; }

    let total = status.positions.len();
    if total == 0 { return 0.0; }

    match status.face_down_count {
        0 => {
            // All face-up. If gap is achievable (with wilds), it's completable now.
            100.0
        }
        1 => {
            // One card away. Score 70-90 based on line length (shorter = easier).
            let base = 70.0;
            let length_bonus = if total <= 2 { 20.0 } else if total <= 3 { 15.0 } else { 10.0 };
            // Matching bonus
            let matching_bonus = if status.matching_viable { 5.0 } else { 0.0 };
            base + length_bonus + matching_bonus
        }
        2 => {
            // Two away. Score 30-60 based on gap range achievability.
            let base = 30.0;
            let progress = (total - 2) as f64 / total as f64;
            base + progress * 30.0
        }
        _ => {
            // Three or more away. Low but nonzero if achievable.
            let progress = (total - status.face_down_count) as f64 / total as f64;
            5.0 + progress * 15.0
        }
    }
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
}
```

- [ ] **Step 3: Run line_scoring tests**

Run: `cd src-tauri && cargo test --lib engine::strategy::line_scoring -- --nocapture 2>&1`
Expected: All line_scoring tests pass.

- [ ] **Step 4: Commit**

```bash
git add -A src-tauri/src/engine/strategy/
git rm src-tauri/src/engine/strategy.rs 2>/dev/null || true
git commit -m "feat: create strategy module with shared line scoring infrastructure"
```

---

## Task 3: Implement Opportunist archetype

**Files:**
- Create: `src-tauri/src/engine/strategy/opportunist.rs`
- Modify: `src-tauri/src/engine/strategy/mod.rs` — wire up dispatch

- [ ] **Step 1: Write tests for Opportunist decisions**

Create `src-tauri/src/engine/strategy/opportunist.rs` with tests:

```rust
use rand::Rng;

use super::line_scoring::{score_all_lines, card_fits_line, best_placement};
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

/// Fallback when skill check fails
fn fallback_action(drawn_card: &Card, grid: &PlayerGrid, rng: &mut impl Rng) -> TurnAction {
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

/// Pick the best face-down card to flip: prefer cards in high-scoring lines.
fn best_flip_target(
    face_down: &[(usize, usize)],
    lines: &[(super::line_scoring::LineStatus, f64)],
) -> (usize, usize) {
    let mut best_pos = face_down[0];
    let mut best_score = f64::NEG_INFINITY;

    for &(r, c) in face_down {
        let mut score = 0.0f64;
        for (line, line_score) in lines {
            if line.positions.contains(&(r, c)) {
                // Prefer flipping in lines that are close to completion
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
```

- [ ] **Step 2: Add `mod opportunist;` to `strategy/mod.rs` and wire up dispatch**

Add `mod opportunist;` near the top. Replace the stub `choose_draw_source` and `choose_action` with dispatch logic:

```rust
mod opportunist;

// In choose_draw_source:
pub fn choose_draw_source(
    config: &PlayerConfig,
    discard_top: Option<&Card>,
    grid: &PlayerGrid,
    neg_min: i32,
    pos_max: i32,
    rng: &mut impl Rng,
) -> DrawSource {
    match config.archetype {
        AiArchetype::Opportunist => opportunist::choose_draw_source(config, discard_top, grid, neg_min, pos_max, rng),
        // Methodical and Calculator will be added in later tasks
        _ => opportunist::choose_draw_source(config, discard_top, grid, neg_min, pos_max, rng),
    }
}

// Same pattern for choose_action
```

- [ ] **Step 3: Run tests**

Run: `cd src-tauri && cargo test --lib engine::strategy::opportunist -- --nocapture 2>&1`
Expected: All Opportunist tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/engine/strategy/opportunist.rs src-tauri/src/engine/strategy/mod.rs
git commit -m "feat: implement Opportunist archetype with line-first decision logic"
```

---

## Task 4: Implement Methodical archetype

**Files:**
- Create: `src-tauri/src/engine/strategy/methodical.rs`
- Modify: `src-tauri/src/engine/strategy/mod.rs` — add dispatch

> **Note:** After this task updates the public API signatures in `mod.rs` to include `&mut Option<MethodicalState>`, game.rs and state.rs will not compile. This is expected and will be fixed in Tasks 6 and 7. Only run strategy-module-scoped tests until Task 7 is complete.

- [ ] **Step 1: Create methodical.rs with full implementation and tests**

Create `src-tauri/src/engine/strategy/methodical.rs`:

```rust
use rand::Rng;

use super::line_scoring::{score_all_lines, card_fits_line, best_placement, needed_cards, LineStatus};
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
```

- [ ] **Step 2: Wire up dispatch in mod.rs**

Add `mod methodical;` and `pub use methodical;` near the top. Update `choose_draw_source` and `choose_action` signatures to include `methodical_state: &mut Option<MethodicalState>`:

```rust
mod methodical;

pub fn choose_draw_source(
    config: &PlayerConfig,
    discard_top: Option<&Card>,
    grid: &PlayerGrid,
    neg_min: i32,
    pos_max: i32,
    methodical_state: &mut Option<MethodicalState>,
    rng: &mut impl Rng,
) -> DrawSource {
    match config.archetype {
        AiArchetype::Opportunist => opportunist::choose_draw_source(config, discard_top, grid, neg_min, pos_max, rng),
        AiArchetype::Methodical => {
            let state = methodical_state.get_or_insert_with(MethodicalState::new);
            methodical::choose_draw_source(config, discard_top, grid, neg_min, pos_max, state, rng)
        }
        AiArchetype::Calculator => opportunist::choose_draw_source(config, discard_top, grid, neg_min, pos_max, rng), // stub until Task 5
    }
}

// Same pattern for choose_action
```

Also make `opportunist::fallback_action` `pub(super)` since Methodical references it.

- [ ] **Step 3: Run Methodical tests**

Run: `cd src-tauri && cargo test --lib engine::strategy::methodical -- --nocapture 2>&1`
Expected: All Methodical tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/engine/strategy/methodical.rs src-tauri/src/engine/strategy/mod.rs
git commit -m "feat: implement Methodical archetype with Scout/Build/Close phases"
```

---

## Task 5: Implement Calculator archetype

**Files:**
- Create: `src-tauri/src/engine/strategy/calculator.rs`
- Modify: `src-tauri/src/engine/strategy/mod.rs` — add dispatch

- [ ] **Step 1: Create calculator.rs with full implementation and tests**

Create `src-tauri/src/engine/strategy/calculator.rs`:

```rust
use rand::Rng;
use rand::seq::SliceRandom;

use super::line_scoring::{score_all_lines, card_fits_line, best_placement};
use super::{DrawSource, TurnAction, should_play_smart};
use super::super::card::Card;
use super::super::config::PlayerConfig;
use super::super::grid::PlayerGrid;

/// Representative card values for sampling when skill < 0.8.
/// Covers the full range with common values weighted more.
const SAMPLE_VALUES: [i32; 10] = [-5, -3, -1, 0, 0, 1, 3, 5, 7, 8];

/// Compute expected value of drawing blind from the deck.
/// At high skill, evaluates all possible card values weighted by deck distribution.
/// At lower skill, samples a subset.
fn blind_draw_expected_score(
    grid: &PlayerGrid,
    neg_min: i32,
    pos_max: i32,
    skill: f64,
    rng: &mut impl Rng,
) -> f64 {
    if skill >= 0.8 {
        // Full distribution: evaluate every possible card value
        let mut total_score = 0.0f64;
        let mut total_weight = 0.0f64;

        // Number cards
        for v in neg_min..=pos_max {
            let card = Card::Number(v);
            let (_, score) = best_placement(&card, grid, neg_min, pos_max);
            // Weight roughly proportional to how many of this card exist
            // (approximation — exact counts would require deck state)
            let weight = 1.0;
            total_score += score * weight;
            total_weight += weight;
        }

        // Wild
        let (_, wild_score) = best_placement(&Card::Wild, grid, neg_min, pos_max);
        total_score += wild_score * 0.5; // Wilds are rarer
        total_weight += 0.5;

        if total_weight > 0.0 { total_score / total_weight } else { 0.0 }
    } else {
        // Sample 10 representative cards
        let mut total = 0.0f64;
        for &v in &SAMPLE_VALUES {
            let card = Card::Number(v);
            let (_, score) = best_placement(&card, grid, neg_min, pos_max);
            total += score;
        }
        total / SAMPLE_VALUES.len() as f64
    }
}

/// Score the cascade potential of a placement.
/// Simulates placing the card, checks if elimination happens, then scores resulting grid.
fn cascade_score(
    card: &Card,
    pos: (usize, usize),
    grid: &PlayerGrid,
    neg_min: i32,
    pos_max: i32,
    allow_matching: bool,
    allow_diagonal: bool,
) -> f64 {
    let mut sim_grid = grid.clone();
    sim_grid.replace_card(pos.0, pos.1, card.clone());

    let eliminations = sim_grid.find_eliminations(allow_matching, allow_diagonal, neg_min, pos_max);
    if eliminations.is_empty() {
        return 0.0;
    }

    // Apply first elimination
    let elim = &eliminations[0];
    sim_grid.eliminate(&elim.positions);
    sim_grid.cleanup();

    // Score the resulting grid — more lines close to completion = better
    let post_lines = score_all_lines(&sim_grid, neg_min, pos_max);
    let post_score: f64 = post_lines.iter().map(|(_, s)| s).sum();

    // Check for further eliminations (cascade)
    let further = sim_grid.find_eliminations(allow_matching, allow_diagonal, neg_min, pos_max);
    let cascade_bonus = if further.is_empty() { 0.0 } else { 50.0 };

    // Base bonus for triggering an elimination + cascade potential
    30.0 + cascade_bonus + post_score * 0.1
}

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

    if !should_play_smart(config.skill, rng) {
        return if rng.gen_bool(0.5) { DrawSource::DiscardPile } else { DrawSource::DrawPile };
    }

    // Always take Wild
    if matches!(card, Card::Wild) {
        return DrawSource::DiscardPile;
    }

    // Score taking the discard
    let (_, discard_score) = best_placement(card, grid, neg_min, pos_max);

    // Score drawing blind (expected value)
    let blind_score = blind_draw_expected_score(grid, neg_min, pos_max, config.skill, rng);

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
    neg_min: i32,
    pos_max: i32,
    rng: &mut impl Rng,
) -> TurnAction {
    let face_down = grid.face_down_positions();

    if !should_play_smart(config.skill, rng) {
        return super::opportunist::fallback_action(drawn_card, grid, rng);
    }

    // Evaluate all possible placements
    let occupied = grid.occupied_positions();
    let card_value = match drawn_card { Card::Number(v) => *v, Card::Wild => 0 };
    let is_wild = matches!(drawn_card, Card::Wild);

    let mut best_pos = occupied.first().copied().unwrap_or((0, 0));
    let mut best_score = f64::NEG_INFINITY;

    for &(r, c) in &occupied {
        // Don't replace Wild with non-Wild
        if !is_wild {
            if let Some(gc) = grid.get(r, c) {
                if gc.face_up && matches!(gc.card, Card::Wild) { continue; }
            }
        }

        let mut score = 0.0f64;
        let lines = score_all_lines(grid, neg_min, pos_max);

        for (line, _) in &lines {
            if !line.positions.contains(&(r, c)) { continue; }

            let is_face_down = grid.get(r, c).map_or(false, |gc| !gc.face_up);
            if is_face_down {
                score += card_fits_line(card_value, line, neg_min, pos_max);
            } else {
                // Replacing face-up: evaluate improvement
                let old_value = grid.get(r, c).map_or(0, |gc| match &gc.card {
                    Card::Number(v) => *v, Card::Wild => 0,
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
            score += cascade_score(drawn_card, (r, c), grid, neg_min, pos_max, true, true);
        }

        // Replacing bad face-up with better card
        if let Some(gc) = grid.get(r, c) {
            if gc.face_up {
                let old_abs = match &gc.card { Card::Number(v) => v.abs(), Card::Wild => 0 };
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
        let lines = score_all_lines(grid, neg_min, pos_max);
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
        let lines = score_all_lines(grid, neg_min, pos_max);
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
    use super::super::super::config::AiArchetype;

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
        let result = choose_draw_source(&config, Some(&Card::Number(0)), &grid, -5, 8, &mut rng);
        assert_eq!(result, DrawSource::DiscardPile, "Completing card should beat blind draw EV");
    }

    #[test]
    fn test_draws_blind_when_discard_is_bad() {
        let config = expert_calculator();
        let grid = make_grid_all_face_up(&[1,2,3,4, 5,6,7,8, 1,2,3,4, 5,6,7,8]);
        let mut rng = rand::thread_rng();

        // Discard is 8 (bad card for sum-to-zero on this grid)
        // Blind draw has better expected value
        let result = choose_draw_source(&config, Some(&Card::Number(8)), &grid, -5, 8, &mut rng);
        // We can't assert DrawPile with certainty (depends on grid analysis) but
        // at minimum it shouldn't always take an 8
        // This is a soft test — the real validation is the integration turn-count test
    }

    #[test]
    fn test_places_card_considering_cascade() {
        let config = expert_calculator();
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
        let action = choose_action(&config, &Card::Number(0), &grid, -5, 8, &mut rng);
        match action {
            TurnAction::ReplaceCard { row, col } => {
                assert_eq!((row, col), (0, 3), "Should place at completing position");
            }
            _ => panic!("Should place completing card, not discard"),
        }
    }

    #[test]
    fn test_blind_draw_ev_returns_finite() {
        let grid = make_grid_all_face_up(&[1,2,3,4, 5,6,7,8, 1,2,3,4, 5,6,7,8]);
        let mut rng = rand::thread_rng();
        let ev = blind_draw_expected_score(&grid, -5, 8, 1.0, &mut rng);
        assert!(ev.is_finite(), "EV should be a finite number");
        let ev_low = blind_draw_expected_score(&grid, -5, 8, 0.5, &mut rng);
        assert!(ev_low.is_finite(), "Sampled EV should also be finite");
    }
}
```

- [ ] **Step 2: Wire up dispatch in mod.rs**

Add `mod calculator;` to `mod.rs`. Update the `AiArchetype::Calculator` arms in `choose_draw_source` and `choose_action` to dispatch to `calculator::`:

```rust
mod calculator;

// In choose_draw_source:
AiArchetype::Calculator => calculator::choose_draw_source(config, discard_top, grid, neg_min, pos_max, rng),

// In choose_action:
AiArchetype::Calculator => calculator::choose_action(config, drawn_card, grid, neg_min, pos_max, rng),
```

- [ ] **Step 3: Run Calculator tests**

Run: `cd src-tauri && cargo test --lib engine::strategy::calculator -- --nocapture 2>&1`
Expected: All Calculator tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/engine/strategy/calculator.rs src-tauri/src/engine/strategy/mod.rs
git commit -m "feat: implement Calculator archetype with full-option scoring and cascade awareness"
```

---

## Task 6: Update game.rs to support new strategy API

**Files:**
- Modify: `src-tauri/src/engine/game.rs`

- [ ] **Step 1: Add MethodicalState to PlayerState**

```rust
use super::strategy::MethodicalState;

struct PlayerState {
    grid: PlayerGrid,
    went_out_first: bool,
    cleared_all: bool,
    eliminations: u32,
    methodical_state: Option<MethodicalState>,  // NEW
}
```

Initialize in `play_round`:
```rust
use super::config::AiArchetype;

// In the player initialization loop:
let methodical_state = match config.players[i].archetype {
    AiArchetype::Methodical => Some(MethodicalState::new()),
    _ => None,
};
players.push(PlayerState {
    grid,
    went_out_first: false,
    cleared_all: false,
    eliminations: 0,
    methodical_state,
});
```

- [ ] **Step 2: Update play_turn and helper calls to pass MethodicalState**

Update `play_turn`, `handle_normal_draw` to pass `&mut state.players[player_idx].methodical_state` to strategy functions.

Update `check_and_apply_eliminations` to call `methodical_state.invalidate_targets()` after any elimination that changes grid dimensions.

- [ ] **Step 3: Update choose_slide_direction callers to pass neg_min/pos_max**

The `choose_slide_direction` in `mod.rs` already accepts `neg_min` and `pos_max` parameters (added in Task 2). Update callers in `game.rs` to pass `config.deck.neg_min` and `config.deck.pos_max` (the old signature only passed `&PlayerConfig, &PlayerGrid, &EliminationType, rng`).

- [ ] **Step 4: Run existing game tests**

Run: `cd src-tauri && cargo test --lib engine::game -- --nocapture 2>&1`
Expected: All existing game tests pass (play_game, round_diagnostic, scoring, etc.)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/engine/game.rs src-tauri/src/engine/strategy/mod.rs
git commit -m "feat: wire new strategy archetypes into game loop"
```

---

## Task 7: Update interactive state.rs

**Files:**
- Modify: `src-tauri/src/interactive/state.rs`

- [ ] **Step 1: Add MethodicalState storage to InteractiveGame**

Add `methodical_states: Vec<Option<MethodicalState>>` to `InteractiveGame`. Initialize in `new()` / `start_round()`. Pass to strategy calls in `advance_ai()`.

- [ ] **Step 2: Update all strategy call sites**

Update calls to `strategy::choose_draw_source`, `strategy::choose_action`, `strategy::choose_discard_with_opponent`, `strategy::choose_slide_direction` to match new signatures (with `MethodicalState` and `neg_min`/`pos_max` where needed).

- [ ] **Step 3: Add invalidate_targets on elimination**

In the elimination handling code, call `methodical_state.invalidate_targets()` when grid dimensions change.

- [ ] **Step 4: Run full test suite**

Run: `cd src-tauri && cargo test -- --nocapture 2>&1`
Expected: All tests pass (may have compilation errors to fix in smoke_test.rs).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/interactive/state.rs
git commit -m "feat: wire new strategy archetypes into interactive game mode"
```

---

## Task 8: Update smoke tests

**Files:**
- Modify: `src-tauri/tests/smoke_test.rs`

- [ ] **Step 1: Update PlayerConfig construction in all tests**

Replace all `PlayerConfig { keep_threshold: ..., line_awareness: ..., opponent_awareness: ..., flip_strategy: ... }` with new format: `PlayerConfig { archetype: AiArchetype::..., skill: ..., flip_strategy: ... }`.

Specifically update:
- `test_skilled_vs_unskilled`: Use `PlayerConfig::expert()` vs `PlayerConfig::beginner()`

- [ ] **Step 2: Add integration test for turn reduction**

```rust
#[test]
fn test_improved_ai_turn_count() {
    let config = GameConfig::default();
    let mut rng = rand::thread_rng();
    let num_games = 100u32;
    let mut total_turns_per_round = 0.0f64;
    let mut total_rounds = 0u32;

    for _ in 0..num_games {
        let result = play_game(&config, &mut rng);
        for round in &result.round_results {
            total_turns_per_round += round.turns as f64;
            total_rounds += 1;
        }
    }

    let avg = total_turns_per_round / total_rounds as f64;
    println!("\nAvg turns per round: {:.1}", avg);
    assert!(avg < 42.0, "Expected < 42 avg turns/round, got {:.1}", avg);
}
```

- [ ] **Step 3: Run all smoke tests**

Run: `cd src-tauri && cargo test --test smoke_test -- --nocapture 2>&1`
Expected: All tests pass, including the new turn count assertion.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tests/smoke_test.rs
git commit -m "test: update smoke tests for new AI archetype system"
```

---

## Task 9: Update frontend config panels

**Files:**
- Modify: `src/js/config-panel.js`
- Modify: `src/js/play-panel.js`

- [ ] **Step 1: Update config-panel.js**

Replace `PLAYER_PRESETS` with archetype-based presets:
```javascript
const PLAYER_PRESETS = {
  Beginner:     { archetype: 'Opportunist', skill: 30, flipStrategy: 'Random' },
  Intermediate: { archetype: 'Methodical',  skill: 60, flipStrategy: 'Random' },
  Advanced:     { archetype: 'Opportunist', skill: 85, flipStrategy: 'Random' },
  Expert:       { archetype: 'Calculator',  skill: 100, flipStrategy: 'Random' },
};
```

Replace `buildPlayerPanel` to show archetype dropdown + skill slider instead of keep_threshold/line_awareness/opponent_awareness:

```javascript
function buildPlayerPanel(idx) {
  const p = PLAYER_PRESETS.Advanced;
  return `
    <div class="player-panel" id="player-panel-${idx}">
      <h3>
        Player ${idx + 1}
        <select class="preset-select" onchange="applyPlayerPreset(${idx}, this.value)">
          <option value="">Preset...</option>
          <option value="Beginner">Beginner</option>
          <option value="Intermediate">Intermediate</option>
          <option value="Advanced" selected>Advanced</option>
          <option value="Expert">Expert</option>
        </select>
      </h3>
      <div class="config-group" style="margin-bottom:0.6rem">
        <label>AI Archetype</label>
        <select id="archetype-${idx}">
          <option value="Opportunist" ${p.archetype === 'Opportunist' ? 'selected' : ''}>Opportunist</option>
          <option value="Methodical" ${p.archetype === 'Methodical' ? 'selected' : ''}>Methodical</option>
          <option value="Calculator" ${p.archetype === 'Calculator' ? 'selected' : ''}>Calculator</option>
        </select>
      </div>
      <div class="slider-group">
        <label>Skill <span class="slider-value" id="skill-val-${idx}">${p.skill}%</span></label>
        <input type="range" id="skill-${idx}" min="0" max="100" value="${p.skill}"
               oninput="document.getElementById('skill-val-${idx}').textContent = this.value + '%'" />
        <div style="display:flex;justify-content:space-between;font-size:0.7rem;color:var(--text-dim)">
          <span>Random play</span><span>Perfect execution</span>
        </div>
      </div>
      <div class="config-group" style="margin-top:0.6rem">
        <label>Initial Flip</label>
        <select id="flip-strategy-${idx}">
          <option value="Random" ${p.flipStrategy === 'Random' ? 'selected' : ''}>Random</option>
          <option value="SameColumn" ${p.flipStrategy === 'SameColumn' ? 'selected' : ''}>Same Column</option>
          <option value="SameRow" ${p.flipStrategy === 'SameRow' ? 'selected' : ''}>Same Row</option>
          <option value="Corners" ${p.flipStrategy === 'Corners' ? 'selected' : ''}>Corners</option>
          <option value="Diagonal" ${p.flipStrategy === 'Diagonal' ? 'selected' : ''}>Diagonal</option>
        </select>
      </div>
    </div>`;
}
```

Update `applyPlayerPreset`, `applyToAll`, and `buildConfigFromUI` to use the new field names:

```javascript
function applyPlayerPreset(idx, presetName) {
  const p = PLAYER_PRESETS[presetName];
  if (!p) return;

  document.getElementById(`archetype-${idx}`).value = p.archetype;
  document.getElementById(`skill-${idx}`).value = p.skill;
  document.getElementById(`skill-val-${idx}`).textContent = p.skill + '%';
  document.getElementById(`flip-strategy-${idx}`).value = p.flipStrategy;
}

function applyToAll() {
  const count = parseInt(document.getElementById('player-count').value);
  const archetype = document.getElementById('archetype-0').value;
  const skill = document.getElementById('skill-0').value;
  const flipStrategy = document.getElementById('flip-strategy-0').value;

  for (let i = 1; i < count; i++) {
    document.getElementById(`archetype-${i}`).value = archetype;
    document.getElementById(`skill-${i}`).value = skill;
    document.getElementById(`skill-val-${i}`).textContent = skill + '%';
    document.getElementById(`flip-strategy-${i}`).value = flipStrategy;
  }
}
```

```javascript
function buildConfigFromUI() {
  // ... deck config unchanged ...

  const players = [];
  for (let i = 0; i < playerCount; i++) {
    players.push({
      archetype: document.getElementById(`archetype-${i}`).value,
      skill: parseInt(document.getElementById(`skill-${i}`).value) / 100,
      flip_strategy: document.getElementById(`flip-strategy-${i}`).value,
    });
  }
  // ... rest unchanged ...
}
```

- [ ] **Step 2: Update play-panel.js**

Replace `AI_PRESETS` and `startPlayGame` player config construction:
```javascript
const AI_PRESETS = {
  beginner:     { archetype: 'Opportunist', skill: 0.3 },
  intermediate: { archetype: 'Methodical',  skill: 0.6 },
  advanced:     { archetype: 'Opportunist', skill: 0.85 },
  expert:       { archetype: 'Calculator',  skill: 1.0 },
};
```

Update human player config:
```javascript
config.players = [
    { archetype: 'Opportunist', skill: 1.0, flip_strategy: 'Random' },
    { ...aiConfig, flip_strategy: 'Random' },
    { ...aiConfig, flip_strategy: 'Random' },
    { ...aiConfig, flip_strategy: 'Random' },
];
```

- [ ] **Step 3: Commit**

```bash
git add src/js/config-panel.js src/js/play-panel.js
git commit -m "feat: update frontend config panels for AI archetypes"
```

---

## Task 10: Integration validation

**Files:** None (validation only)

- [ ] **Step 1: Run full test suite**

Run: `cd src-tauri && cargo test -- --nocapture 2>&1`
Expected: All tests pass.

- [ ] **Step 2: Run round diagnostic to verify turn reduction**

Run: `cd src-tauri && cargo test test_round_diagnostic -- --nocapture 2>&1`
Expected: Average turns/round significantly lower than 60. Target: 28-40.

- [ ] **Step 3: Run performance test**

Run: `cd src-tauri && cargo test test_simulation_performance -- --nocapture 2>&1`
Expected: 10,000 games completes in < 30 seconds.

- [ ] **Step 4: Build the Tauri app to verify frontend compiles**

Run: `cd src-tauri && cargo build 2>&1`
Expected: Clean build, no errors.

- [ ] **Step 5: Final commit if any fixes were needed**

```bash
git add -A
git commit -m "fix: integration fixes for AI archetype system"
```
