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

Compared to Opportunist: checks ALL lines at step 6 rather than just the "hottest" one.

### Action Priority (Place or Discard)

```
1. Skill check fails → Fall back to opportunist::fallback_action
2. Check ALL lines for completion (card_fits_line >= 100) → Place at the face-down slot completing that line
3. Run best_placement() for the best spot on the whole board
   - If score >= 20 → Place it there
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
- Still makes mistakes (skill ~0.45 means random play ~55% of the time)

### File Changes

**`src-tauri/src/engine/strategy/methodical.rs`** — Full rewrite:
- Replace `choose_draw_source`: flat priority list, no phase branching
- Replace `choose_action`: completion check → best_placement → discard & flip
- Delete: `update_phase`, `select_targets`, `targets_still_valid`, `best_scout_flip`, `best_target_flip`, `scout_threshold`
- Add: `best_flip` helper (flip face-down card in highest-scoring line)
- New tests covering priority list behavior

**No changes to:**
- `strategy/mod.rs` — Still dispatches to methodical functions, still passes `&mut MethodicalState` (ignored)
- `game.rs` — Still creates `MethodicalState` per player (harmless, unused)
- `config.rs` — No preset tier changes yet; tune after simulation results
- `line_scoring.rs` — Reuse existing `score_all_lines`, `card_fits_line`, `best_placement`
- `opportunist.rs`, `calculator.rs` — Untouched

### Tests

Replace phase-based tests with:
1. Always takes Wild from discard
2. Takes completing card from discard
3. Places completing card at the correct position
4. Places helpful card (score >= 20) rather than discarding
5. Discards unhelpful card and flips in best line
6. Falls back to random on skill check failure

### Future Work

- Tune preset tiers after running simulations to see where Methodical lands competitively
- Clean up unused `MethodicalState`/`Phase` types once we're confident in the new approach
- Consider whether `MethodicalState` parameter can be removed from the function signatures
