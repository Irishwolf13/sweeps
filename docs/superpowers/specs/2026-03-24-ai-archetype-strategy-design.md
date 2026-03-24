# AI Archetype Strategy Redesign

**Date:** 2026-03-24
**Status:** Draft

## Problem

With 4 players, rounds take ~60 turns (15 per player). The theoretical minimum is 16 turns. The root cause: the current AI strategy prioritizes individual card quality (`keep_threshold`) over line completion. Bots spend turns collecting "good" low-value cards instead of building toward eliminations.

## Goal

Replace the single parameterized strategy with three distinct AI archetypes, each with a fundamentally different approach to play. Target average turns per round: 28-40 (down from ~60). Each archetype should be selectable per player.

## Game Mechanics Summary

- 4x4 grid, 16 cards, most face-down at start (2 flipped via FlipStrategy)
- Each turn: draw from draw pile or discard pile, then either replace a card on your board (face-up) or discard and flip a face-down card
- **Elimination:** a row, column, or diagonal where all cards are face-up AND either sum to zero (primary) or all match (secondary). Wilds count as any value.
- After elimination, grid shrinks. Eliminations cascade.
- **Round ends** when a player has ≤4 cards remaining, all face-up (or 0 cards)
- Sum-to-zero is the primary elimination path. All-matching becomes viable at 2x2.

## Design

### Config Changes

**Current PlayerConfig (removed):**
```rust
pub struct PlayerConfig {
    pub keep_threshold: i32,      // REMOVED
    pub line_awareness: f64,      // REMOVED
    pub opponent_awareness: f64,  // REMOVED
    pub flip_strategy: FlipStrategy,
}
```

**New PlayerConfig:**
```rust
pub enum AiArchetype {
    Opportunist,  // Line-first reactive play
    Methodical,   // Scout → Build → Close phases
    Calculator,   // Score every move, pick optimal
}

pub struct PlayerConfig {
    pub archetype: AiArchetype,
    pub skill: f64,               // 0.0-1.0, execution quality
    pub flip_strategy: FlipStrategy,
}
```

**Preset constructors:**
```rust
impl PlayerConfig {
    pub fn beginner() -> Self { /* Opportunist, skill 0.3 */ }
    pub fn intermediate() -> Self { /* Methodical, skill 0.6 */ }
    pub fn advanced() -> Self { /* Opportunist, skill 0.85 */ }
    pub fn expert() -> Self { /* Calculator, skill 1.0 */ }
}
```

Default game config: 4 players using beginner, intermediate, advanced, expert presets.

### Shared Infrastructure: Line Scoring

All three archetypes share a line-scoring foundation that replaces the current `analyze_lines`/`value_helps_line` system.

**LineStatus** — computed per line:
- `positions: Vec<(usize, usize)>` — cells in this line
- `face_up_count: usize`
- `face_down_count: usize`
- `current_sum: i32` — sum of face-up number cards
- `wild_count: usize` — face-up wilds in the line
- `gap: i32` — what remaining cards need to sum to (`-current_sum`)
- `gap_achievable: bool` — can face-down + wilds theoretically fill the gap?
- `cards_needed: usize` — number of unresolved positions (`face_down_count`)
- `matching_value: Option<i32>` — if all face-up numbers match, what value
- `matching_viable: bool` — could this line still be all-matching?

**LineScore** — how close to elimination:
- **Completable** (100): 0 face-down, gap achievable with wilds alone
- **One away** (70-90): 1 face-down, gap is a single achievable value. Higher score if that value is common in the deck.
- **Two away** (30-60): 2 face-down, gap achievable. Higher score if needed value range is wide (easier to hit).
- **Hopeless** (0): Gap not achievable given remaining unknowns, or too many face-down

**Key functions:**
- `score_all_lines(grid) -> Vec<(LineStatus, f64)>` — score every line in the grid
- `card_fits_line(card, line) -> f64` — 0-100, how well a card helps a specific line. 100 = completes it.
- `best_placement(card, grid) -> (position, net_score)` — best position considering all lines, accounting for helping one line while hurting another
- `needed_cards(line) -> Vec<i32>` — what specific values would complete this line

### Archetype 1: Opportunist (Line-First Reactive)

No memory between turns. Every turn is a fresh evaluation.

**Decision flow:**
1. Score all lines, identify hottest (highest scoring) line
2. **Draw decision:**
   - Discard top completes any line? → Always take
   - Discard top fits hottest line (score ≥ 50)? → Take
   - Discard top is Wild or 0? → Take (universally useful for sum-to-zero)
   - Otherwise → Draw blind
3. **Place decision:**
   - Compute `best_placement(drawn_card, grid)`
   - If placement score ≥ 30 → place it there
   - Otherwise → discard + flip the best face-down card in the hottest line
4. **Endgame (2x2):** Factor matching viability into line scores

**Skill dial:** Each decision point gated by `rng.gen_bool(skill)`. Fail → random draw source, random placement/flip.

### Archetype 2: Methodical (Phase-Based)

Maintains state across turns within a round.

**State:**
```rust
struct MethodicalState {
    phase: Phase,          // Scout, Build, Close
    target_lines: Vec<usize>,  // indices of committed lines
    turns_in_phase: u32,
}
enum Phase { Scout, Build, Close }
```

**Phase 1 — Scout:**
- Active while face-down ratio exceeds a threshold (skill-dependent: high skill scouts less, ~3-4 turns; low skill scouts longer, ~6-8 turns)
- Only keeps Wilds and 0s from draws
- Everything else: discard and flip
- Smart flip targeting: prioritize face-down cards that share lines with already-revealed cards
- Transition to Build when threshold met

**Phase 2 — Build:**
- Pick 1-2 target lines with best LineScore
- All draw/place decisions serve target lines only
- Re-evaluate targets if a reveal breaks a target line (face-down flipped to a value that makes gap unachievable)
- Transition to Close when any target is 1 card away from completion

**Phase 3 — Close:**
- Compute the exact value needed for the target line
- Discard top is the needed value? → Take and complete
- Drawn card is the needed value? → Place and complete
- Otherwise → discard + flip (don't waste placement on a non-completing card)
- After elimination → grid shrinks, re-enter Build for the new grid

**Skill dial:** Same `rng.gen_bool(skill)` gating. Fail → random action within current phase context (e.g., during Build, random placement instead of target-line placement, not fully random).

### Archetype 3: Calculator (Score Every Move)

No memory needed. Recomputes everything fresh each turn.

**Decision flow:**
1. **Score taking discard:**
   - `discard_score = best_placement(discard_top, grid).net_score`
2. **Score drawing blind (expected value):**
   - Weighted average over remaining deck distribution
   - `blind_score = Σ (card_probability × best_placement(card, grid).net_score)`
   - Skill < 0.8: sample 10 representative cards instead of full distribution
   - Skill ≥ 0.8: full distribution
3. **Draw from** `max(discard_score, blind_score)`
4. **Place:** Use the best placement already computed
   - If best placement score < discard-and-flip score → discard + flip position that maximizes expected information gain
5. **Cascade bonus (skill ≥ 0.6):**
   - For each placement option, simulate the elimination, score the resulting smaller grid
   - Add cascade potential to placement score

**Lookahead (interactive mode only, skill ≥ 0.8):**
- For each placement option, simulate 1 turn ahead
- Sample possible next-draw cards, evaluate best response
- Adds ~50ms per AI turn — invisible in interactive play
- **Disabled in bulk simulation** for performance (50k games stays under 10 minutes)

**Skill dial:**
- 1.0: full distribution scoring, cascade awareness, lookahead (interactive)
- 0.5-0.8: full option evaluation, no lookahead, no cascade
- < 0.5: random subset of options evaluated

### Skill Dial Mechanics

Universal across all archetypes:
```rust
fn should_play_smart(skill: f64, rng: &mut impl Rng) -> bool {
    rng.gen_bool(skill.clamp(0.0, 1.0))
}
```

Each decision point (draw choice, place choice, flip target) independently rolls against skill. At skill 1.0, always executes archetype logic. At 0.0, always random. At 0.5, each decision is a coin flip.

Fallback behavior when skill check fails:
- Draw: random source (50/50)
- Place: if card abs value ≤ 3, replace a random face-down; otherwise discard + flip random face-down
- This ensures even "failed" decisions aren't catastrophically bad

## Files Changed

### Backend (src-tauri/src/engine/)
- **config.rs**: Replace `PlayerConfig` fields. Add `AiArchetype` enum. Add preset constructors.
- **strategy.rs**: Full rewrite. New module structure:
  - `strategy/line_scoring.rs` — shared LineStatus, LineScore, card_fits_line, best_placement
  - `strategy/opportunist.rs` — Opportunist decision logic
  - `strategy/methodical.rs` — Methodical phase state machine and decisions
  - `strategy/calculator.rs` — Calculator scoring and optional lookahead
  - `strategy/mod.rs` — public API: `choose_draw_source`, `choose_action` dispatch to archetype
- **game.rs**: Minimal changes. `play_turn` calls same strategy API, which now dispatches internally.

### Backend (src-tauri/src/interactive/)
- **state.rs**: Update to use new PlayerConfig. Methodical needs per-player state stored between turns. Calculator lookahead enabled for interactive mode.

### Frontend (src/js/)
- **Player config panels**: Replace keep_threshold/line_awareness/opponent_awareness sliders with archetype dropdown + skill slider.
- **Simulation config**: Same panel updates.

## Testing Strategy

- Unit tests for `line_scoring.rs`: verify LineStatus computation, card_fits_line scoring, best_placement correctness
- Unit tests per archetype: verify decision logic with known grid states
- Integration test: run 100 games, assert average turns/round < 45 (significant improvement over current ~60)
- Regression: existing game flow tests still pass (round completion, scoring, elimination cascades)
- Performance: 50,000 games with mixed archetypes completes under 10 minutes

## Performance Budget

- Opportunist: ~0.5ms per game (fastest, no state, simple evaluation)
- Methodical: ~1ms per game (light state management)
- Calculator (no lookahead): ~10ms per game (evaluates all options per turn)
- Calculator (with lookahead, interactive only): ~50ms per AI turn
- 50,000 mixed games target: under 10 minutes
