# Methodical AI Beginner Rework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Methodical AI's phase-based state machine with a stateless priority list that plays like a beginner with common sense.

**Architecture:** Rewrite `methodical.rs` to use a flat priority list (complete lines first → place helpful cards → flip face-down). Extract `best_flip_target` from `opportunist.rs` into a shared location. No changes to dispatch layer, game loop, or other archetypes.

**Tech Stack:** Rust, Tauri 2.x backend

**Spec:** `docs/superpowers/specs/2026-03-25-methodical-beginner-rework-design.md`

---

### Task 1: Extract `best_flip_target` to shared location

**Files:**
- Modify: `src-tauri/src/engine/strategy/mod.rs:12` (add re-export)
- Modify: `src-tauri/src/engine/strategy/line_scoring.rs` (add function at end, before tests)
- Modify: `src-tauri/src/engine/strategy/opportunist.rs:3,85,123-146` (import shared, delete local copy)

- [ ] **Step 1: Add `best_flip_target` to `line_scoring.rs`**

Add this function at the end of `line_scoring.rs`, before the `#[cfg(test)]` block (before line 360):

```rust
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
```

- [ ] **Step 2: Add `best_flip_target` to the public re-exports in `mod.rs`**

In `src-tauri/src/engine/strategy/mod.rs` line 12, change:

```rust
pub use line_scoring::{LineStatus, score_all_lines, card_fits_line, best_placement, needed_cards};
```

to:

```rust
pub use line_scoring::{LineStatus, score_all_lines, card_fits_line, best_placement, best_flip_target, needed_cards};
```

- [ ] **Step 3: Update `opportunist.rs` to use the shared function**

In `src-tauri/src/engine/strategy/opportunist.rs`:

Change the import on line 3 from:
```rust
use super::line_scoring::{score_all_lines, card_fits_line, best_placement};
```
to:
```rust
use super::line_scoring::{score_all_lines, card_fits_line, best_placement, best_flip_target};
```

Change line 85 from:
```rust
        let flip_target = best_flip_target(&face_down, &lines);
```
(no change needed — the function name is the same, it just resolves to a different module now)

Delete the local `best_flip_target` function (lines 123-146):
```rust
/// Pick the best face-down card to flip: prefer cards in high-scoring lines.
fn best_flip_target(
    ...entire function...
}
```

- [ ] **Step 4: Run tests to verify nothing broke**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: All existing tests pass. The opportunist tests should still work since the logic is identical.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/engine/strategy/line_scoring.rs src-tauri/src/engine/strategy/mod.rs src-tauri/src/engine/strategy/opportunist.rs
git commit -m "refactor: extract best_flip_target to shared line_scoring module"
```

---

### Task 2: Rewrite `methodical.rs` — draw source logic

**Files:**
- Modify: `src-tauri/src/engine/strategy/methodical.rs:1-92` (replace entire draw source function and its helpers)

- [ ] **Step 1: Write the failing draw source tests**

Replace the entire `#[cfg(test)] mod tests` block in `methodical.rs` (lines 310-422) with these new tests. We'll add action tests in Task 3.

```rust
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
        let grid = make_grid_one_away(); // Row 0: -3,1,2,face_down
        let mut state = MethodicalState::new();
        let mut rng = rand::thread_rng();
        // A -1 would make the gap smaller (current gap = 0, placing -1 makes gap 1, still viable)
        // card_fits_line should score this >= 40
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
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml methodical::tests`
Expected: Most tests should fail because the old phase-based logic doesn't match the new expected behavior (e.g., Scout phase rejects helpful cards).

- [ ] **Step 3: Rewrite the draw source function and remove old helpers**

Replace lines 1-92 of `methodical.rs` (everything above `pub fn choose_action`) with:

```rust
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
```

- [ ] **Step 4: Run draw source tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml methodical::tests::test_draw`
Expected: All 6 draw source tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/engine/strategy/methodical.rs
git commit -m "feat: rewrite Methodical draw source as stateless priority list"
```

---

### Task 3: Rewrite `methodical.rs` — action logic

**Files:**
- Modify: `src-tauri/src/engine/strategy/methodical.rs` (replace `choose_action` and delete old helpers)

- [ ] **Step 1: Add action tests to the test module**

Add these tests inside the existing `mod tests` block, after the draw source tests:

```rust
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
        // Two rows both need 0 to complete. Row with higher line score should win.
        // Row 0: -3, 1, 2, face_down → gap=0, needs 0
        // Row 1: -2, 1, 1, face_down → gap=0, needs 0
        // Both completable, but best_placement's +500 bonus will pick one.
        // We just verify a completion happens (both are valid).
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
                // Should complete one of the two rows
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
        // Grid with some face-down cards where a low card is useful
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
        // Grid where all face-up, except one face-down card. An 8 won't help much.
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

        // Drawing another 8 onto a board full of high cards — best_placement score likely < 20
        let action = choose_action(&config, &Card::Number(8), &grid, -5, 8, &mut state, &mut rng);
        assert!(matches!(action, TurnAction::DiscardAndFlip { .. }), "Should discard unhelpful card and flip");
    }

    #[test]
    fn test_action_all_face_up_places_anyway() {
        let config = expert_methodical();
        let grid = make_grid_all_face_up(&[1,2,3,4, 5,6,7,8, 1,2,3,4, 5,6,7,8]);
        let mut state = MethodicalState::new();
        let mut rng = rand::thread_rng();

        // No face-down cards, must place somewhere
        let action = choose_action(&config, &Card::Number(0), &grid, -5, 8, &mut state, &mut rng);
        assert!(matches!(action, TurnAction::ReplaceCard { .. }), "Must place when all face-up");
    }

    #[test]
    fn test_action_skill_zero_uses_fallback() {
        // skill 0.0 means should_play_smart always returns false → fallback_action
        // fallback_action places low cards (abs <= 3) and discards high cards
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
        // Leave several face-down so fallback has flip targets
        grid.flip_card(0, 0); grid.flip_card(0, 1);

        let mut state = MethodicalState::new();
        let mut rng = rand::thread_rng();

        // High card (8) with face-down available → fallback should discard and flip
        let action = choose_action(&config, &Card::Number(8), &grid, -5, 8, &mut state, &mut rng);
        assert!(matches!(action, TurnAction::DiscardAndFlip { .. }),
            "Skill 0 with high card should use fallback (discard and flip)");

        // Low card (0) with face-down available → fallback should place it
        let action = choose_action(&config, &Card::Number(0), &grid, -5, 8, &mut state, &mut rng);
        assert!(matches!(action, TurnAction::ReplaceCard { .. }),
            "Skill 0 with low card should use fallback (place it)");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml methodical::tests::test_action`
Expected: FAIL — the old phase-based `choose_action` doesn't match the new priority behavior.

- [ ] **Step 3: Replace `choose_action` and delete old helpers**

Replace everything from the old `pub fn choose_action` through the end of `best_target_flip` (lines 94-308 in the original file, but line numbers will have shifted after Task 2) with:

```rust
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
```

Also make sure the old helper functions are deleted: `update_phase`, `scout_threshold`, `select_targets`, `targets_still_valid`, `best_scout_flip`, `best_target_flip`. After Task 2's draw source rewrite and this change, the only functions remaining in `methodical.rs` should be `choose_draw_source`, `choose_action`, and `mod tests`.

- [ ] **Step 4: Run all tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: All tests pass — both new methodical tests and existing opportunist/calculator/line_scoring tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/engine/strategy/methodical.rs
git commit -m "feat: rewrite Methodical action logic as stateless priority list"
```

---

### Task 4: Remove unused imports and clean up

**Files:**
- Modify: `src-tauri/src/engine/strategy/methodical.rs` (clean up imports)
- Modify: `src-tauri/src/engine/strategy/mod.rs` (remove Phase from pub use if exported)

- [ ] **Step 1: Verify the build has no warnings**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep warning`
Expected: Check for any unused import warnings in `methodical.rs` (e.g., `Phase` is no longer imported/used). Fix any that appear.

The new `methodical.rs` imports should be exactly:
```rust
use rand::Rng;

use super::line_scoring::{score_all_lines, card_fits_line, best_placement, best_flip_target};
use super::{DrawSource, TurnAction, MethodicalState, should_play_smart};
use super::super::card::Card;
use super::super::config::PlayerConfig;
use super::super::grid::PlayerGrid;
```

Note: `Phase` and `LineStatus` should NOT be imported (no longer used). If `Phase` was in the old import, remove it.

- [ ] **Step 2: Add TODO comment on MethodicalState**

In `src-tauri/src/engine/strategy/mod.rs`, add a comment above the `MethodicalState` struct (line 37):

```rust
// TODO: MethodicalState is no longer used by the stateless Methodical strategy.
// Remove once we're confident in the new approach and can update the dispatch signatures.
#[derive(Clone, Debug)]
pub struct MethodicalState {
```

- [ ] **Step 3: Run full test suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: All tests pass, no warnings.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/engine/strategy/methodical.rs src-tauri/src/engine/strategy/mod.rs
git commit -m "chore: clean up unused imports and add TODO for MethodicalState removal"
```

---

### Task 5: Smoke test with the running app

**Files:** None (manual verification)

- [ ] **Step 1: Launch the app**

Run: `cargo tauri dev`

- [ ] **Step 2: Run a simulation**

In the app, set up a 4-player game with one Methodical player and run 1000+ simulations. Verify:
- Methodical no longer finishes last in nearly every game
- Methodical wins rate is reasonable for its skill level (doesn't need to be the best, just competitive)
- No panics or errors in the console

- [ ] **Step 3: Play an interactive game**

Switch to the Play tab, set an AI opponent to Methodical, and play a round. Verify:
- AI takes turns without errors
- AI behavior looks reasonable (takes good discard cards, places them sensibly, flips when it can't use a card)
