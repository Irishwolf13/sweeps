# Methodical AI Rework: Stateless Beginner Strategy

**Date:** 2026-03-25
**Status:** Design

## Problem

The current Methodical AI uses a Scout/Build/Close phase system that causes it to lose almost every game. The core issues:

1. **Scout phase wastes turns** — Only keeps Wilds and 0s for ~5 turns while other players build lines
2. **Build phase is too narrow** — Only places cards serving 1-2 target lines, discards everything else
3. **Close phase is too restrictive** — Only places line-completing cards (fit == 100), wastes good cards
4. **Empty targets** — `select_targets` can produce no targets, leaving Build/Close aimless
5. **Skill 0.6 compounds waste** — 40% random fallback on top of already-wasted smart turns

## Design

Replace the phase-based state machine with a stateless priority list. The new Methodical plays like a beginner with common sense: if you can complete a line, do it; otherwise look around the board for a good placement; otherwise flip something to learn more.

### Draw Source Priority

```
1. No discard available → Draw pile
2. Skill check fails → Random choice (coin flip)
3. Discard is Wild → Take it
4. Discard completes ANY line (card_fits_line >= 100) → Take it
5. Discard is a 0 → Take it (universally useful for sum-to-zero)
6. Discard meaningfully helps any line (card_fits_line >= 40) → Take it
7. Otherwise → Draw pile
```

Compared to Opportunist: checks ALL lines at step 6 (threshold 40) rather than just the "hottest" line (threshold 50). The lower threshold is intentional — the Methodical beginner is more willing to grab a card that looks useful.

### Action Priority (Place or Discard)

```
1. Skill check fails → Fall back to opportunist::fallback_action
2. Check ALL lines for completion (card_fits_line >= 100) → Place at the face-down slot completing that line
   - If multiple lines completable, pick the one with the highest line score
   - Note: card_fits_line returns 100 only when remaining_unknowns == 0 (exactly 1 face-down slot),
     so the target position is always unambiguous within a line
3. Run best_placement() for the best spot on the whole board (face-down OR face-up positions)
   - If score >= 20 → Place it there
   - This may replace a face-up card if that's the best move (e.g., swapping a high card for a low one)
4. Face-down cards remain → Discard and flip the best one (highest-scoring line)
5. All face-up → Place at best_placement position regardless of score
```

Key differences from Opportunist:
- **Explicit completion check first** (step 2) before general placement
- **Lower placement threshold** (20 vs Opportunist's 30) — more willing to place useful cards
- Same flip logic when discarding

### Personality

The Methodical beginner is someone who:
- Recognizes when a line is one card away and takes the opportunity
- Looks around the whole board before deciding, not just the best line
- Is more willing to place a card that "looks useful" than to hold out for perfect
- Still makes mistakes at lower skill levels (current preset is skill 0.6; may be tuned down to ~0.45 after simulation)

### File Changes

**`src-tauri/src/engine/strategy/methodical.rs`** — Full rewrite:
- Replace `choose_draw_source`: flat priority list, no phase branching
- Replace `choose_action`: completion check → best_placement → discard & flip
- Delete: `update_phase`, `select_targets`, `targets_still_valid`, `best_scout_flip`, `best_target_flip`, `scout_threshold`
- Promote `best_flip_target` from `opportunist.rs` to a shared `pub(super)` function in `mod.rs` (or `line_scoring.rs`) and reuse from both archetypes, avoiding duplication
- New tests covering priority list behavior

**No changes to:**
- `strategy/mod.rs` — Still dispatches to methodical functions, still passes `&mut MethodicalState` (ignored with `let _ = state;`)
- `game.rs` — Still creates `MethodicalState` per player (harmless, unused)
- `config.rs` — No preset tier changes yet; tune after simulation results
- `line_scoring.rs` — Reuse existing `score_all_lines`, `card_fits_line`, `best_placement`
- `opportunist.rs`, `calculator.rs` — Untouched

### Tests

Replace phase-based tests with:

**Draw source tests:**
1. Always takes Wild from discard
2. Takes completing card from discard
3. Takes 0 from discard
4. Takes helpful card (fit >= 40) from discard
5. Draws from pile when discard is unhelpful
6. Random choice on skill check failure

**Action tests:**
7. Places completing card at the correct position
8. When multiple lines completable, picks highest-scoring line
9. Places helpful card (score >= 20) rather than discarding
10. Discards unhelpful card and flips in best line
11. All face-up fallback: places at best_placement regardless of score
12. Falls back to opportunist::fallback_action on skill check failure

### Future Work

- Tune preset tiers after running simulations to see where Methodical lands competitively
- Clean up unused `MethodicalState`/`Phase` types once we're confident in the new approach
- Consider whether `MethodicalState` parameter can be removed from the function signatures
