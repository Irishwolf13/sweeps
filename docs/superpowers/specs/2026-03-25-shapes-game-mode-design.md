# Shape Sweep: Game Mode Integration Design

## Overview

Add a "Shapes" game mode to the existing Sweeps simulator alongside the current "Numbers" mode. A toggle in the UI switches between modes, swapping card semantics, elimination rules, deck presets, and end-of-round conditions. The grid, game loop, scoring (card count), AI strategies, simulation runner, and stats/charts all remain structurally unchanged.

**Tagline:** "No Numbers. Just Smart Matching."

## Game Mode Enum

```rust
enum GameMode { Numbers, Shapes }
```

Added to `GameConfig`. Gates all mode-specific behavior throughout the engine.

## Card Domain

### Expanded Card Enum

```rust
enum Shape { Circle, Square, Triangle, Rectangle }
enum Shade { Unshaded, Shaded }

enum Card {
    Number(i32),              // Numbers mode
    Shape(Shape, Shade),      // Shapes mode
    Wild,                     // Both modes — matches anything
    WildShaded,               // Shapes only — matches any shaded card
    WildUnshaded,             // Shapes only — matches any unshaded card
}
```

### Scoring

- Numbers mode: Basic (card count) or Expert (sum of abs values) — unchanged.
- Shapes mode: Always card count via `remaining_card_count()`. Every remaining card in the grid (including wilds) costs 1 point. The `score_value()` method is only used in Numbers Expert mode and is not called for Shapes scoring.

### Out of Scope

Switch and Swap card types do not exist in this game and will not be implemented. Any references to them in the original rules document should be ignored.

### Display

Shape cards render as text labels for interactive play:
- `Circle`, `Square`, `Triangle`, `Rectangle` (unshaded)
- `Shaded Circle`, `Shaded Square`, etc. (shaded)
- `Wild`, `Wild Shaded`, `Wild Unshaded`

## Tier System (Shapes Only)

Four tiers controlling which elimination rules are active and how wilds behave:

| Tier | Target Age | `shade_matters` | `allow_matching` | `allow_cancellation` | `allow_diagonal` | Wilds in deck |
|------|:----------:|:-:|:-:|:-:|:-:|:--|
| Beginner | Ages 3-5 | false | true | false | false | None (wild counts set to 0 in preset) |
| Intermediate | Ages 4-6 | true | true | false | false | All 30 |
| Advanced | Ages 5-8 | true | true | true | false | All 30 |
| Expert | Ages 7+ | true | true | true | true | All 30 |

The "Target Age" column is informational only (from the physical game design) and does not affect simulation behavior.

### New Config Fields

- `shade_matters: bool` — when false, shade is ignored for matching (Shaded Circle == Unshaded Circle). All wild types act as universal wild.
- `allow_cancellation: bool` — when true, shaded + unshaded of the same shape cancel each other (Shapes equivalent of sum-to-zero).

Existing fields reused: `allow_matching_elimination`, `allow_diagonal_elimination`.

## Elimination Logic

### All-Matching (All Tiers)

- **Beginner (`shade_matters: false`):** All cards in a line are the same shape, ignoring shade. Circle(Shaded) matches Circle(Unshaded). All wild types match anything.
- **Intermediate+ (`shade_matters: true`):** All cards are the exact same card (shape + shade). Circle(Shaded) only matches Circle(Shaded). Wild matches anything. WildShaded matches any shaded card. WildUnshaded matches any unshaded card.

### Cancellation (Advanced/Expert Only)

- Pairs of shaded + unshaded of the same shape cancel each other.
- A line cancels if every card is paired. E.g., [Circle(Shaded), Circle(Unshaded), Triangle(Shaded), Triangle(Unshaded)] — each shape has its shaded/unshaded pair.
- Line length must be even for full cancellation (no unpaired cards), unless wilds fill the gap.
- Wild counts as either side of any shape. WildShaded counts as any shaded card. WildUnshaded counts as any unshaded card.

### Cancellation Pairing Algorithm

For a line to cancel, every non-wild card must be paired: for each shape present, the count of shaded must equal the count of unshaded. Wilds fill gaps:

1. Count each shape's shaded and unshaded occurrences among non-wild cards.
2. For each shape, compute `deficit = |shaded_count - unshaded_count|`. Sum all deficits.
3. Assign constrained wilds first: WildShaded can fill any shaded deficit, WildUnshaded can fill any unshaded deficit.
4. Assign universal Wilds to remaining deficits (they can fill either side of any shape).
5. After assignment, if total remaining deficit is 0, the line cancels. Any leftover wilds (beyond what's needed for pairing) are acceptable — they pair with each other.

This greedy approach works because wilds are fungible within their constraint class — there's no case where optimal assignment of a Wild to shape A vs shape B matters, since all that matters is the total deficit.

### Implementation

A new `check_shape_elimination` function sits alongside the existing numeric checks. `find_eliminations` dispatches to the right check based on `GameMode`. The `EliminationReason` enum gains a `Cancellation` variant for Shapes mode (rather than reusing `SumToZero`, which would be confusing in UI/logs).

## End-of-Round Condition

- **Numbers mode:** Round triggered when a player reaches ≤4 face-up cards or 0 cards (unchanged).
- **Shapes mode:** Round triggered only when a player reaches 0 cards (must fully clear the grid).

After trigger, all other players get one more turn. Then scores are tallied.

## Going-Out-First Bonus

- **Numbers mode:** -2 point bonus for the player who triggers the end of round (unchanged).
- **Shapes mode:** No bonus. No penalty. Just card count.

## AI Strategy Layer

### Replacing `neg_min`/`pos_max` Threading

Currently `neg_min` and `pos_max` are passed as bare `i32` parameters through the entire strategy layer (~15 function signatures). In Shapes mode these values are meaningless. Rather than adding a `GameMode` parameter alongside them everywhere, introduce a context struct:

```rust
struct EliminationContext {
    game_mode: GameMode,
    // Numbers mode fields
    neg_min: i32,
    pos_max: i32,
    // Shapes mode fields
    shade_matters: bool,
    allow_cancellation: bool,
}
```

All functions that currently take `neg_min, pos_max` take `&EliminationContext` instead. This is a single signature change per function and avoids adding more parameters as modes grow. `GameConfig` provides a method to build this context.

### LineStatus Adaptation

The current `LineStatus` has Numbers-specific fields (`current_sum`, `gap`, `gap_achievable`). For Shapes mode, add shape-specific fields:

```rust
struct LineStatus {
    // Common fields
    pub positions: Vec<(usize, usize)>,
    pub face_up_count: usize,
    pub face_down_count: usize,
    pub wild_count: usize,
    pub cards_needed: usize,

    // Numbers mode fields (zeroed in Shapes mode)
    pub current_sum: i32,
    pub gap: i32,
    pub gap_achievable: bool,
    pub matching_value: Option<i32>,
    pub matching_viable: bool,

    // Shapes mode fields (None/zeroed in Numbers mode)
    pub matching_shape: Option<(Shape, Option<Shade>)>,  // shape match target
    pub shade_deficit: i32,  // positive = need more shaded, negative = need more unshaded
    pub cancellation_viable: bool,
}
```

This keeps `LineStatus` as a single struct rather than an enum, since the archetypes only care about the score (0-100) returned by `score_line()`, not the raw fields. The fields exist for `analyze_line` and `score_line` to use internally.

### Line Scoring Adaptation

The archetypes (Opportunist, Methodical, Calculator) call into `line_scoring` functions: `card_fits_line`, `score_all_lines`, `best_placement`, `best_flip_target`.

`analyze_line` and `score_line` dispatch internally based on `EliminationContext.game_mode`:
- In Numbers mode, they evaluate numeric sums and value matching (unchanged).
- In Shapes mode, they evaluate shape matching and shaded/unshaded pairing.

`card_fits_line` changes signature from `card_fits_line(card_value: i32, ...)` to `card_fits_line(card: &Card, ...)` so it can inspect shape/shade. `best_placement` similarly takes `&Card` instead of extracting a numeric value.

The archetype code itself does not change. An Opportunist grabbing a card that scores 90 on a line doesn't care if that score comes from completing a number sum or a shape pair.

### Archetype Presets

Beginner/Intermediate/Advanced/Expert player presets (skill levels, archetypes) remain the same across both modes. Skill level and play style are independent of card domain.

## Deck Configuration

### DeckConfig Structure

The current `DeckConfig` is Numbers-specific (`neg_min`, `pos_max`, `card_quantities: Vec<(i32, u32)>`). Rather than making it generic, use an enum approach:

```rust
enum DeckConfig {
    Numbers {
        neg_min: i32,
        pos_max: i32,
        card_quantities: Vec<(i32, u32)>,  // (value, count)
        wild_count: u32,
    },
    Shapes {
        shape_quantities: Vec<(Shape, Shade, u32)>,  // (shape, shade, count)
        wild_count: u32,          // universal Wild
        wild_shaded_count: u32,   // WildShaded
        wild_unshaded_count: u32, // WildUnshaded
    },
}
```

`build_deck` matches on the enum variant and builds the appropriate card types. `total_cards()` and `validate()` work on either variant. The `neg_min`/`pos_max` values are only accessible from the `Numbers` variant, which is correct since they are meaningless in Shapes mode.

**Beginner wild removal:** The Beginner tier preset constructor simply sets all three wild counts to 0. No post-processing needed.

### Original 230-Card Deck

- 25 each x 8 types (Circle/Square/Triangle/Rectangle x Shaded/Unshaded) = 200
- 10 Wild + 10 WildShaded + 10 WildUnshaded = 30
- Total: 230
- Beginner tier: wilds removed automatically (200 cards)

### Scaled "Default" Deck (Per Player Count)

| Players | Shape cards (per type) | Wilds (W/WS/WU) | Total |
|:-------:|:---------------------:|:----------------:|:-----:|
| 2 | 8 each (64) | 4/4/4 (12) | 76 |
| 3 | 11 each (88) | 5/5/5 (15) | 103 |
| 4 | 14 each (112) | 6/6/6 (18) | 130 |
| 5 | 17 each (136) | 8/8/8 (24) | 160 |
| 6 | 20 each (160) | 9/9/9 (27) | 187 |

Targets ~30-40% draw pile after dealing grids, matching Numbers game ratios.

### Manual Overrides

After selecting a preset, users can manually adjust the quantity of each individual card type (all 8 shape types + 3 wild types), same as Numbers mode.

## UI Changes

### Config Panel

- **Game mode toggle** at the top: "Numbers" / "Shapes"
- Switching mode swaps:
  - Deck preset options (Numbers presets vs Shapes presets)
  - Card quantity editor (number values vs shape types)
  - Tier selector for Shapes (Beginner/Intermediate/Advanced/Expert) which auto-sets elimination rule checkboxes
  - Scoring mode dropdown hidden in Shapes (always card count)
  - Going-out-first bonus hidden in Shapes

### Simulation Results

No changes. Stats, charts, bell curves, score histograms all work on scores and card counts — structurally identical regardless of mode.

### Interactive Play Tab

- Shape cards displayed as text labels with visual distinction for shaded vs unshaded
- Grid layout, turn flow, draw/discard mechanics identical
- Mode-appropriate labels (shape names instead of numbers on cards)

## Files Affected

### Backend (Rust)

| File | Change |
|------|--------|
| `engine/card.rs` | Add `Shape`, `Shade` enums, `WildShaded`/`WildUnshaded` variants, update `build_deck`, `score_value`, `Display` |
| `engine/config.rs` | Add `GameMode`, `shade_matters`, `allow_cancellation`, shapes deck presets, tier constructors |
| `engine/grid.rs` | Update `find_eliminations` / `check_elimination` to dispatch by mode, add `check_shape_elimination` |
| `engine/strategy/line_scoring.rs` | Mode-aware `card_fits_line`, `score_all_lines`, `analyze_line` |
| `engine/strategy/mod.rs` | Pass game mode through to scoring functions |
| `engine/game.rs` | Mode-aware end-of-round condition, going-out bonus gating |
| `interactive/state.rs` | Pass `GameMode` / `EliminationContext` through interactive play state machine |
| `commands.rs` | Pass `GameMode` through IPC commands |

### Frontend (JS)

| File | Change |
|------|--------|
| `js/config-panel.js` | Game mode toggle, shapes tier selector, shapes deck presets, shape card quantity editor |
| `js/play-panel.js` | Shape card rendering in interactive play grid |
| `js/app.js` | Pass game mode in simulation/play config |

## Serialization

Adding new `Card` variants and restructuring `DeckConfig` as an enum is a breaking change for serde serialization. This affects IPC payloads between Rust and the frontend. Since there are no persisted save files or stable API contracts, this is acceptable. The frontend config-panel JS will be updated to send the new structure.

## What Does NOT Change

- Grid structure (4x4, cleanup, reshape)
- Game loop flow (draw, replace/flip, check eliminations, next player)
- Round/game structure (N rounds, cumulative scores, lowest wins)
- Simulation runner (parallel with rayon)
- Stats and charts
- Player count (2-6)
- AI archetype behavior (Opportunist/Methodical/Calculator)
- Flip strategies
- Starting order options
