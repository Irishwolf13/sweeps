# Shapes Game Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Shapes game mode toggle to the existing Sweeps Numbers simulator, allowing simulation and interactive play of the Shape Sweep card game.

**Architecture:** Expand the Card enum with Shape/Shade variants + constrained wilds. Introduce GameMode enum and EliminationContext struct to replace bare neg_min/pos_max threading. DeckConfig becomes an enum with Numbers/Shapes variants. Elimination, scoring, line-scoring, and end-of-round logic dispatch by game mode.

**Tech Stack:** Rust (Tauri 2.x backend), HTML/JS/CSS frontend, serde for IPC serialization.

**Spec:** `docs/superpowers/specs/2026-03-25-shapes-game-mode-design.md`

---

## File Structure

### New files
- None — all changes modify existing files.

### Modified files (in implementation order)
1. `src-tauri/src/engine/card.rs` — Shape, Shade enums; new Card variants; build_deck for shapes
2. `src-tauri/src/engine/config.rs` — GameMode, EliminationContext, DeckConfig enum, shapes presets
3. `src-tauri/src/engine/grid.rs` — Shape elimination logic, mode-aware find_eliminations
4. `src-tauri/src/engine/strategy/line_scoring.rs` — Shapes-mode analyze_line, score_line, card_fits_line, best_placement
5. `src-tauri/src/engine/strategy/mod.rs` — EliminationContext threading, mode-aware discard selection
6. `src-tauri/src/engine/strategy/opportunist.rs` — EliminationContext signatures
7. `src-tauri/src/engine/strategy/methodical.rs` — EliminationContext signatures
8. `src-tauri/src/engine/strategy/calculator.rs` — EliminationContext signatures
9. `src-tauri/src/engine/game.rs` — Mode-aware round end, scoring, EliminationContext usage
10. `src-tauri/src/interactive/state.rs` — EliminationContext usage, shapes card display
11. `src-tauri/src/commands.rs` — Pass-through (no signature changes needed, GameConfig carries mode)
12. `src/js/config-panel.js` — Game mode toggle, shapes deck presets, tier selector, shape card quantities
13. `src/js/play-panel.js` — Shape card rendering
14. `src/js/app.js` — Build config with game_mode field

---

## Task 1: Card Domain — Shape, Shade enums and new Card variants

**Files:**
- Modify: `src-tauri/src/engine/card.rs`

- [ ] **Step 1: Add Shape and Shade enums**

Add above the existing `Card` enum:

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Shape {
    Circle,
    Square,
    Triangle,
    Rectangle,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Shade {
    Unshaded,
    Shaded,
}
```

- [ ] **Step 2: Expand Card enum with new variants**

Add three new variants to `Card`:

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Card {
    Number(i32),
    Shape(Shape, Shade),
    Wild,
    WildShaded,
    WildUnshaded,
}
```

- [ ] **Step 3: Update score_value() for new variants**

```rust
pub fn score_value(&self) -> i32 {
    match self {
        Card::Number(v) => v.abs(),
        _ => 0,
    }
}
```

The existing `_ => 0` wildcard already handles new variants. Verify it compiles.

- [ ] **Step 4: Update Display impl for new variants**

```rust
impl std::fmt::Display for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Card::Number(v) => write!(f, "{}", v),
            Card::Shape(shape, Shade::Unshaded) => write!(f, "{:?}", shape),
            Card::Shape(shape, Shade::Shaded) => write!(f, "Shaded {:?}", shape),
            Card::Wild => write!(f, "Wild"),
            Card::WildShaded => write!(f, "Wild Shaded"),
            Card::WildUnshaded => write!(f, "Wild Unshaded"),
        }
    }
}
```

- [ ] **Step 5: Run `cargo check` in src-tauri — fix any non-exhaustive match errors**

The new Card variants will cause compile errors in files that match on Card (grid.rs, strategy files, interactive/state.rs). That's expected — we'll fix those in later tasks. For now, confirm card.rs itself compiles by checking only the module:

Run: `cd src-tauri && cargo check 2>&1 | head -30`
Expected: Errors in OTHER files (grid.rs, strategy, state.rs) — not in card.rs itself.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/engine/card.rs
git commit -m "feat: add Shape, Shade enums and new Card variants for Shapes game mode"
```

---

## Task 2: DeckConfig enum and GameMode

**Files:**
- Modify: `src-tauri/src/engine/config.rs`

- [ ] **Step 1: Add GameMode enum**

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum GameMode {
    Numbers,
    Shapes,
}

impl Default for GameMode {
    fn default() -> Self {
        GameMode::Numbers
    }
}
```

- [ ] **Step 2: Add EliminationContext struct**

```rust
#[derive(Clone, Debug)]
pub struct EliminationContext {
    pub game_mode: GameMode,
    pub neg_min: i32,
    pub pos_max: i32,
    pub shade_matters: bool,
    pub allow_cancellation: bool,
}
```

- [ ] **Step 3: Convert DeckConfig from struct to enum**

Replace the existing `DeckConfig` struct with:

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum DeckConfig {
    Numbers {
        neg_min: i32,
        pos_max: i32,
        card_quantities: Vec<(i32, u32)>,
        wild_count: u32,
    },
    Shapes {
        shape_quantities: Vec<(Shape, Shade, u32)>,
        wild_count: u32,
        wild_shaded_count: u32,
        wild_unshaded_count: u32,
    },
}
```

Note: uses `#[serde(tag = "type")]` for clean JSON discriminator.

- [ ] **Step 4: Implement total_cards() and validate() on DeckConfig**

```rust
impl DeckConfig {
    pub fn total_cards(&self) -> u32 {
        match self {
            DeckConfig::Numbers { card_quantities, wild_count, .. } => {
                let number_cards: u32 = card_quantities.iter().map(|(_, count)| count).sum();
                number_cards + wild_count
            }
            DeckConfig::Shapes { shape_quantities, wild_count, wild_shaded_count, wild_unshaded_count, .. } => {
                let shape_cards: u32 = shape_quantities.iter().map(|(_, _, count)| count).sum();
                shape_cards + wild_count + wild_shaded_count + wild_unshaded_count
            }
        }
    }

    pub fn validate(&self, player_count: u8) -> Result<(), String> {
        let needed = (player_count as u32) * 16 + 20;
        let total = self.total_cards();
        if total < needed {
            return Err(format!(
                "Deck has {} cards but {} players need at least {} ({}×16 + 20 for draw pile)",
                total, player_count, needed, player_count
            ));
        }
        match self {
            DeckConfig::Numbers { neg_min, pos_max, .. } => {
                if *neg_min > 0 {
                    return Err("Negative range minimum must be <= 0".to_string());
                }
                if *pos_max < 0 {
                    return Err("Positive range maximum must be >= 0".to_string());
                }
            }
            DeckConfig::Shapes { shape_quantities, .. } => {
                if shape_quantities.is_empty() {
                    return Err("Shapes deck must have at least one shape type".to_string());
                }
            }
        }
        Ok(())
    }
}
```

- [ ] **Step 5: Update Default for DeckConfig**

```rust
impl Default for DeckConfig {
    fn default() -> Self {
        let card_quantities = vec![
            (-5, 5), (-4, 6), (-3, 8), (-2, 9), (-1, 11),
            (0, 13),
            (1, 11), (2, 11), (3, 10), (4, 9), (5, 8), (6, 7), (7, 6), (8, 6),
        ];
        DeckConfig::Numbers {
            neg_min: -5,
            pos_max: 8,
            card_quantities,
            wild_count: 12,
        }
    }
}
```

- [ ] **Step 6: Add new config fields to GameConfig**

Add `game_mode`, `shade_matters`, `allow_cancellation` fields:

```rust
pub struct GameConfig {
    pub game_mode: GameMode,
    pub deck: DeckConfig,
    pub player_count: u8,
    pub allow_matching_elimination: bool,
    pub allow_diagonal_elimination: bool,
    pub allow_cancellation: bool,    // NEW: shapes shaded/unshaded pairing
    pub shade_matters: bool,         // NEW: false = ignore shade for matching
    pub scoring_mode: ScoringMode,
    pub starting_order: StartingOrder,
    pub players: Vec<PlayerConfig>,
    pub max_turns_per_round: u32,
    pub round_multiplier: u8,
}
```

Update `Default` for `GameConfig` to set `game_mode: GameMode::Numbers`, `shade_matters: false`, `allow_cancellation: false`.

- [ ] **Step 7: Add elimination_context() method to GameConfig**

```rust
impl GameConfig {
    pub fn elimination_context(&self) -> EliminationContext {
        match &self.deck {
            DeckConfig::Numbers { neg_min, pos_max, .. } => EliminationContext {
                game_mode: self.game_mode.clone(),
                neg_min: *neg_min,
                pos_max: *pos_max,
                shade_matters: self.shade_matters,
                allow_cancellation: self.allow_cancellation,
            },
            DeckConfig::Shapes { .. } => EliminationContext {
                game_mode: self.game_mode.clone(),
                neg_min: 0,
                pos_max: 0,
                shade_matters: self.shade_matters,
                allow_cancellation: self.allow_cancellation,
            },
        }
    }
}
```

- [ ] **Step 8: Add shapes deck preset constructors**

```rust
impl DeckConfig {
    /// Full 230-card shapes deck
    pub fn shapes_original() -> Self {
        let mut shape_quantities = Vec::new();
        for shape in &[Shape::Circle, Shape::Square, Shape::Triangle, Shape::Rectangle] {
            for shade in &[Shade::Unshaded, Shade::Shaded] {
                shape_quantities.push((shape.clone(), shade.clone(), 25u32));
            }
        }
        DeckConfig::Shapes {
            shape_quantities,
            wild_count: 10,
            wild_shaded_count: 10,
            wild_unshaded_count: 10,
        }
    }

    /// Scaled shapes deck for given player count
    pub fn shapes_scaled(player_count: u8) -> Self {
        let (per_type, w, ws, wu) = match player_count {
            2 => (8, 4, 4, 4),
            3 => (11, 5, 5, 5),
            4 => (14, 6, 6, 6),
            5 => (17, 8, 8, 8),
            6 => (20, 9, 9, 9),
            _ => (14, 6, 6, 6),
        };
        let mut shape_quantities = Vec::new();
        for shape in &[Shape::Circle, Shape::Square, Shape::Triangle, Shape::Rectangle] {
            for shade in &[Shade::Unshaded, Shade::Shaded] {
                shape_quantities.push((shape.clone(), shade.clone(), per_type));
            }
        }
        DeckConfig::Shapes {
            shape_quantities,
            wild_count: w,
            wild_shaded_count: ws,
            wild_unshaded_count: wu,
        }
    }
}
```

- [ ] **Step 9: Fix all compile errors from DeckConfig becoming an enum**

Every place that accesses `config.deck.neg_min` or `config.deck.pos_max` directly will fail. These are in:
- `game.rs` (~10 occurrences) — will be replaced with `config.elimination_context()` in Task 6
- `interactive/state.rs` (~8 occurrences) — will be replaced in Task 7
- `commands.rs` — just calls `validate()` which already works

For now, add temporary helper methods to unblock compilation:

```rust
impl DeckConfig {
    /// Temporary: extract neg_min for Numbers mode, 0 for Shapes
    pub fn neg_min(&self) -> i32 {
        match self { DeckConfig::Numbers { neg_min, .. } => *neg_min, _ => 0 }
    }
    /// Temporary: extract pos_max for Numbers mode, 0 for Shapes
    pub fn pos_max(&self) -> i32 {
        match self { DeckConfig::Numbers { pos_max, .. } => *pos_max, _ => 0 }
    }
}
```

- [ ] **Step 10: Update build_deck in card.rs for the new DeckConfig enum**

```rust
pub fn build_deck(config: &DeckConfig) -> Vec<Card> {
    match config {
        DeckConfig::Numbers { card_quantities, wild_count, .. } => {
            let mut deck = Vec::new();
            for &(value, count) in card_quantities {
                for _ in 0..count {
                    deck.push(Card::Number(value));
                }
            }
            for _ in 0..*wild_count {
                deck.push(Card::Wild);
            }
            deck
        }
        DeckConfig::Shapes { shape_quantities, wild_count, wild_shaded_count, wild_unshaded_count } => {
            let mut deck = Vec::new();
            for (shape, shade, count) in shape_quantities {
                for _ in 0..*count {
                    deck.push(Card::Shape(shape.clone(), shade.clone()));
                }
            }
            for _ in 0..*wild_count {
                deck.push(Card::Wild);
            }
            for _ in 0..*wild_shaded_count {
                deck.push(Card::WildShaded);
            }
            for _ in 0..*wild_unshaded_count {
                deck.push(Card::WildUnshaded);
            }
            deck
        }
    }
}
```

- [ ] **Step 11: Fix existing config tests**

The tests reference `DeckConfig::default()` which now returns `DeckConfig::Numbers { .. }`. Update tests that access `.neg_min` directly to use the helper method or pattern matching.

- [ ] **Step 12: Run `cargo check` — all existing tests should still compile**

Run: `cd src-tauri && cargo check`
Expected: Clean compile (possibly with warnings about unused fields).

- [ ] **Step 13: Run `cargo test` — existing tests pass**

Run: `cd src-tauri && cargo test`
Expected: All existing tests pass.

- [ ] **Step 14: Write tests for shapes deck building**

Add tests in `card.rs`:

```rust
#[test]
fn test_build_shapes_deck_original() {
    let config = DeckConfig::shapes_original();
    let deck = build_deck(&config);
    assert_eq!(deck.len(), 230); // 200 shape + 30 wild
    let shape_count = deck.iter().filter(|c| matches!(c, Card::Shape(_, _))).count();
    assert_eq!(shape_count, 200);
    let wild_count = deck.iter().filter(|c| matches!(c, Card::Wild)).count();
    assert_eq!(wild_count, 10);
    let ws_count = deck.iter().filter(|c| matches!(c, Card::WildShaded)).count();
    assert_eq!(ws_count, 10);
    let wu_count = deck.iter().filter(|c| matches!(c, Card::WildUnshaded)).count();
    assert_eq!(wu_count, 10);
}

#[test]
fn test_build_shapes_deck_scaled_4p() {
    let config = DeckConfig::shapes_scaled(4);
    let deck = build_deck(&config);
    assert_eq!(deck.len(), 130); // 112 shape + 18 wild
}

#[test]
fn test_shapes_deck_validation() {
    let config = DeckConfig::shapes_scaled(4);
    assert!(config.validate(4).is_ok());
    assert!(config.validate(6).is_err()); // not enough cards for 6 players
}
```

- [ ] **Step 15: Run tests, verify new tests pass**

Run: `cd src-tauri && cargo test`
Expected: All pass.

- [ ] **Step 16: Commit**

```bash
git add src-tauri/src/engine/config.rs src-tauri/src/engine/card.rs
git commit -m "feat: add GameMode, DeckConfig enum, EliminationContext, shapes deck presets"
```

---

## Task 3: Shape Elimination Logic

**Files:**
- Modify: `src-tauri/src/engine/grid.rs`

- [ ] **Step 1: Add Cancellation variant to EliminationReason**

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum EliminationReason {
    SumToZero,
    AllMatching,
    Cancellation,  // NEW: shapes shaded/unshaded pairing
}
```

- [ ] **Step 2: Update find_eliminations signature to take EliminationContext**

Change from:
```rust
pub fn find_eliminations(&self, allow_matching: bool, allow_diagonal: bool, neg_min: i32, pos_max: i32) -> Vec<Elimination>
```
To:
```rust
pub fn find_eliminations(&self, allow_matching: bool, allow_diagonal: bool, ctx: &EliminationContext) -> Vec<Elimination>
```

Note: `allow_cancellation` is NOT added as a separate parameter — it's already available as `ctx.allow_cancellation`. Similarly, `neg_min`/`pos_max` are in `ctx`. The `allow_matching` and `allow_diagonal` booleans remain separate parameters because they come from `GameConfig` fields that are independent of the elimination context.

Update all internal calls to `check_elimination` to pass `ctx`.

- [ ] **Step 3: Update check_elimination to dispatch by game mode**

```rust
fn check_elimination(
    cards: &[&Card],
    allow_matching: bool,
    ctx: &EliminationContext,
) -> Option<EliminationReason> {
    if cards.is_empty() { return None; }

    match ctx.game_mode {
        GameMode::Numbers => {
            if check_sum_to_zero(cards, ctx.neg_min, ctx.pos_max) {
                return Some(EliminationReason::SumToZero);
            }
            if allow_matching && check_all_matching(cards) {
                return Some(EliminationReason::AllMatching);
            }
        }
        GameMode::Shapes => {
            if allow_matching && check_shape_matching(cards, ctx.shade_matters) {
                return Some(EliminationReason::AllMatching);
            }
            if ctx.allow_cancellation && check_shape_cancellation(cards, ctx.shade_matters) {
                return Some(EliminationReason::Cancellation);
            }
        }
    }
    None
}
```

- [ ] **Step 4: Implement check_shape_matching**

```rust
/// Check if all cards match the same shape (and shade, if shade_matters).
fn check_shape_matching(cards: &[&Card], shade_matters: bool) -> bool {
    let mut target_shape: Option<&Shape> = None;
    let mut target_shade: Option<&Shade> = None;

    for &card in cards {
        match card {
            Card::Wild => continue, // matches anything
            Card::WildShaded => {
                if shade_matters {
                    // Must match Shaded
                    match target_shade {
                        Some(Shade::Unshaded) => return false,
                        None => target_shade = Some(&Shade::Shaded),
                        _ => {}
                    }
                }
                // Shape unconstrained
                continue;
            }
            Card::WildUnshaded => {
                if shade_matters {
                    match target_shade {
                        Some(Shade::Shaded) => return false,
                        None => target_shade = Some(&Shade::Unshaded),
                        _ => {}
                    }
                }
                continue;
            }
            Card::Shape(shape, shade) => {
                // Check shape
                match target_shape {
                    Some(existing) if existing != shape => return false,
                    None => target_shape = Some(shape),
                    _ => {}
                }
                // Check shade (only if shade_matters)
                if shade_matters {
                    match target_shade {
                        Some(existing) if existing != shade => return false,
                        None => target_shade = Some(shade),
                        _ => {}
                    }
                }
            }
            Card::Number(_) => return false, // Numbers can't match shapes
        }
    }
    true
}
```

- [ ] **Step 5: Implement check_shape_cancellation**

```rust
/// Check if shaded/unshaded pairs of same shape cancel the line.
/// Requires even card count and all deficits resolved.
fn check_shape_cancellation(cards: &[&Card], _shade_matters: bool) -> bool {
    if cards.len() % 2 != 0 { return false; } // odd lines can't fully pair

    use std::collections::HashMap;
    let mut shaded_counts: HashMap<&Shape, i32> = HashMap::new();
    let mut unshaded_counts: HashMap<&Shape, i32> = HashMap::new();
    let mut wild_count = 0u32;
    let mut wild_shaded_count = 0u32;
    let mut wild_unshaded_count = 0u32;

    for &card in cards {
        match card {
            Card::Shape(shape, Shade::Shaded) => *shaded_counts.entry(shape).or_insert(0) += 1,
            Card::Shape(shape, Shade::Unshaded) => *unshaded_counts.entry(shape).or_insert(0) += 1,
            Card::Wild => wild_count += 1,
            Card::WildShaded => wild_shaded_count += 1,
            Card::WildUnshaded => wild_unshaded_count += 1,
            Card::Number(_) => return false, // Numbers can't cancel shapes
        }
    }

    // Compute deficit per shape
    let all_shapes: std::collections::HashSet<&&Shape> =
        shaded_counts.keys().chain(unshaded_counts.keys()).collect();

    let mut total_shaded_deficit = 0i32; // need more shaded cards
    let mut total_unshaded_deficit = 0i32; // need more unshaded cards

    for shape in all_shapes {
        let shaded = shaded_counts.get(*shape).copied().unwrap_or(0);
        let unshaded = unshaded_counts.get(*shape).copied().unwrap_or(0);
        let diff = shaded - unshaded;
        if diff > 0 {
            total_unshaded_deficit += diff; // need more unshaded
        } else if diff < 0 {
            total_shaded_deficit += -diff; // need more shaded
        }
    }

    // Assign constrained wilds first
    let shaded_after = (total_shaded_deficit - wild_shaded_count as i32).max(0);
    let unshaded_after = (total_unshaded_deficit - wild_unshaded_count as i32).max(0);

    // Assign universal wilds to remaining deficit
    let remaining_deficit = shaded_after + unshaded_after;
    remaining_deficit <= wild_count as i32
}
```

- [ ] **Step 6: Write tests for shape matching**

```rust
#[test]
fn test_shape_matching_same_shade() {
    let cards: Vec<Card> = vec![
        Card::Shape(Shape::Circle, Shade::Shaded),
        Card::Shape(Shape::Circle, Shade::Shaded),
        Card::Shape(Shape::Circle, Shade::Shaded),
        Card::Shape(Shape::Circle, Shade::Shaded),
    ];
    let refs: Vec<&Card> = cards.iter().collect();
    assert!(check_shape_matching(&refs, true));
    assert!(check_shape_matching(&refs, false));
}

#[test]
fn test_shape_matching_shade_ignored() {
    let cards: Vec<Card> = vec![
        Card::Shape(Shape::Circle, Shade::Shaded),
        Card::Shape(Shape::Circle, Shade::Unshaded),
        Card::Shape(Shape::Circle, Shade::Shaded),
        Card::Shape(Shape::Circle, Shade::Unshaded),
    ];
    let refs: Vec<&Card> = cards.iter().collect();
    assert!(!check_shape_matching(&refs, true));  // shade matters: not all same
    assert!(check_shape_matching(&refs, false));   // shade ignored: all circles
}

#[test]
fn test_shape_matching_with_wild() {
    let cards: Vec<Card> = vec![
        Card::Shape(Shape::Triangle, Shade::Shaded),
        Card::Wild,
        Card::Shape(Shape::Triangle, Shade::Shaded),
        Card::WildShaded,
    ];
    let refs: Vec<&Card> = cards.iter().collect();
    assert!(check_shape_matching(&refs, true));
}

#[test]
fn test_shape_matching_wild_shaded_conflicts() {
    // WildShaded conflicts with Unshaded target when shade_matters
    let cards: Vec<Card> = vec![
        Card::Shape(Shape::Circle, Shade::Unshaded),
        Card::WildShaded,
        Card::Shape(Shape::Circle, Shade::Unshaded),
        Card::Shape(Shape::Circle, Shade::Unshaded),
    ];
    let refs: Vec<&Card> = cards.iter().collect();
    assert!(!check_shape_matching(&refs, true));  // WildShaded can't match Unshaded
    assert!(check_shape_matching(&refs, false));   // shade ignored
}
```

- [ ] **Step 7: Write tests for shape cancellation**

```rust
#[test]
fn test_shape_cancellation_basic() {
    let cards: Vec<Card> = vec![
        Card::Shape(Shape::Circle, Shade::Shaded),
        Card::Shape(Shape::Circle, Shade::Unshaded),
        Card::Shape(Shape::Triangle, Shade::Shaded),
        Card::Shape(Shape::Triangle, Shade::Unshaded),
    ];
    let refs: Vec<&Card> = cards.iter().collect();
    assert!(check_shape_cancellation(&refs, true));
}

#[test]
fn test_shape_cancellation_unbalanced() {
    let cards: Vec<Card> = vec![
        Card::Shape(Shape::Circle, Shade::Shaded),
        Card::Shape(Shape::Circle, Shade::Shaded),
        Card::Shape(Shape::Triangle, Shade::Shaded),
        Card::Shape(Shape::Triangle, Shade::Unshaded),
    ];
    let refs: Vec<&Card> = cards.iter().collect();
    assert!(!check_shape_cancellation(&refs, true)); // Circle has 2 shaded, 0 unshaded
}

#[test]
fn test_shape_cancellation_with_wild() {
    let cards: Vec<Card> = vec![
        Card::Shape(Shape::Circle, Shade::Shaded),
        Card::Wild, // fills Circle Unshaded
        Card::Shape(Shape::Square, Shade::Shaded),
        Card::Shape(Shape::Square, Shade::Unshaded),
    ];
    let refs: Vec<&Card> = cards.iter().collect();
    assert!(check_shape_cancellation(&refs, true));
}

#[test]
fn test_shape_cancellation_odd_length_fails() {
    let cards: Vec<Card> = vec![
        Card::Shape(Shape::Circle, Shade::Shaded),
        Card::Shape(Shape::Circle, Shade::Unshaded),
        Card::Wild,
    ];
    let refs: Vec<&Card> = cards.iter().collect();
    assert!(!check_shape_cancellation(&refs, true)); // odd length
}

#[test]
fn test_shape_cancellation_with_constrained_wilds() {
    let cards: Vec<Card> = vec![
        Card::Shape(Shape::Circle, Shade::Shaded),
        Card::WildUnshaded, // fills Circle Unshaded
        Card::Shape(Shape::Square, Shade::Unshaded),
        Card::WildShaded,   // fills Square Shaded
    ];
    let refs: Vec<&Card> = cards.iter().collect();
    assert!(check_shape_cancellation(&refs, true));
}
```

- [ ] **Step 8: Fix callers of find_eliminations**

All callers need the new signature. Temporarily use `config.elimination_context()` where possible, or the helper methods. Key callers:
- `game.rs: check_and_apply_eliminations` (line ~346)
- `interactive/state.rs: check_eliminations_human` (line ~457)
- `interactive/state.rs: check_eliminations_ai` (line ~516)

Update all to pass `&config.elimination_context()`.

- [ ] **Step 9: Run `cargo test` — all tests pass including new shape tests**

Run: `cd src-tauri && cargo test`
Expected: All pass.

- [ ] **Step 10: Commit**

```bash
git add src-tauri/src/engine/grid.rs src-tauri/src/engine/game.rs src-tauri/src/interactive/state.rs
git commit -m "feat: add shape matching and cancellation elimination logic"
```

---

## Task 4: EliminationContext Threading Through Strategy Layer

**Files:**
- Modify: `src-tauri/src/engine/strategy/line_scoring.rs`
- Modify: `src-tauri/src/engine/strategy/mod.rs`
- Modify: `src-tauri/src/engine/strategy/opportunist.rs`
- Modify: `src-tauri/src/engine/strategy/methodical.rs`
- Modify: `src-tauri/src/engine/strategy/calculator.rs`

This is a mechanical signature change: replace all `neg_min: i32, pos_max: i32` pairs with `ctx: &EliminationContext` throughout the strategy layer. The internal logic remains Numbers-mode for now (Shapes-mode scoring comes in Task 5).

- [ ] **Step 1: Update line_scoring.rs signatures**

Change all functions:
- `score_all_lines(grid, neg_min, pos_max)` → `score_all_lines(grid, ctx)`
- `card_fits_line(card_value, line, neg_min, pos_max)` → `card_fits_line(card: &Card, line, ctx)`
- `best_placement(card, grid, neg_min, pos_max)` → `best_placement(card, grid, ctx)`
- `needed_cards(line, neg_min, pos_max)` → `needed_cards(line, ctx)`
- `analyze_line(grid, positions, neg_min, pos_max)` → `analyze_line(grid, positions, ctx)`

Inside each function, use `ctx.neg_min` and `ctx.pos_max` where the raw values were used.

In `card_fits_line`, change from `card_value: i32` to `card: &Card` and extract the value inside:
```rust
let card_value = match card {
    Card::Number(v) => *v,
    _ => 0,
};
```

In `best_placement`, remove the internal card_value extraction (lines 140-143) since the `card` parameter is already `&Card`.

- [ ] **Step 2: Update strategy/mod.rs signatures**

Change all public functions to use `ctx: &EliminationContext` instead of `neg_min: i32, pos_max: i32`:
- `choose_draw_source`
- `choose_action`
- `choose_discard_with_opponent`
- `choose_slide_direction`

Update internal calls to line_scoring functions accordingly.

- [ ] **Step 3: Update opportunist.rs signatures**

Replace `neg_min: i32, pos_max: i32` with `ctx: &EliminationContext` in:
- `choose_draw_source`
- `choose_action`

Update all internal calls to `score_all_lines`, `card_fits_line`, `best_placement`, `best_flip_target`.

In `choose_draw_source`, update the card_value extraction and `card_fits_line` calls to pass `&Card` instead of `i32`:
```rust
// Before: card_fits_line(card_value, line, neg_min, pos_max)
// After:  card_fits_line(card, line, ctx)
```

- [ ] **Step 4: Update methodical.rs signatures**

Same pattern as opportunist.rs. Replace `neg_min, pos_max` with `ctx: &EliminationContext`.

- [ ] **Step 5: Update calculator.rs signatures**

Same pattern. Also update `blind_draw_expected_score` and `cascade_score` to take `ctx: &EliminationContext`.

**Important: calculator.rs has TWO calls to `find_eliminations`** (in `cascade_score`, lines 71 and 86) that need the new signature (`allow_cancellation, ctx`). Also update `cascade_score` to accept `allow_cancellation: bool` and pass it through.

The calculator's `blind_draw_expected_score` evaluates all possible card values in `neg_min..=pos_max` — for now this still uses `ctx.neg_min` and `ctx.pos_max`. The full Shapes-mode calculator adaptation comes in Task 5.

- [ ] **Step 6: Update game.rs to use elimination_context()**

Replace all `config.deck.neg_min, config.deck.pos_max` with `&config.elimination_context()` (or compute it once and reuse). Key locations:
- `play_turn` (line ~254, ~322)
- `check_and_apply_eliminations` (line ~346-380)

- [ ] **Step 7: Update interactive/state.rs to use elimination_context()**

Same as game.rs. Replace all `self.config.deck.neg_min, self.config.deck.pos_max` with `&self.config.elimination_context()`.

- [ ] **Step 8: Remove temporary neg_min()/pos_max() helper methods from DeckConfig**

These were added in Task 2 Step 9. Now that all callers use EliminationContext, remove them.

- [ ] **Step 9: Update line_scoring tests**

Tests that construct `LineStatus` directly and call `card_fits_line(value, ...)` need updating to pass `&Card::Number(value)` and a dummy `EliminationContext`.

Add test helper:
```rust
fn numbers_ctx() -> EliminationContext {
    EliminationContext {
        game_mode: GameMode::Numbers,
        neg_min: -5,
        pos_max: 8,
        shade_matters: false,
        allow_cancellation: false,
    }
}
```

- [ ] **Step 10: Update opportunist tests**

Same signature changes in test calls.

- [ ] **Step 11: Run `cargo test` — all tests pass**

Run: `cd src-tauri && cargo test`
Expected: All pass.

- [ ] **Step 12: Commit**

```bash
git add src-tauri/src/engine/strategy/ src-tauri/src/engine/game.rs src-tauri/src/interactive/state.rs src-tauri/src/engine/config.rs
git commit -m "refactor: thread EliminationContext through strategy layer replacing bare neg_min/pos_max"
```

---

## Task 5: Shapes-Mode Line Scoring and Strategy Logic

**Files:**
- Modify: `src-tauri/src/engine/strategy/line_scoring.rs`
- Modify: `src-tauri/src/engine/strategy/mod.rs`
- Modify: `src-tauri/src/engine/strategy/opportunist.rs`

- [ ] **Step 1: Add Shapes fields to LineStatus**

```rust
pub struct LineStatus {
    // Existing fields...
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
    // New Shapes fields
    pub matching_shape: Option<Shape>,
    pub matching_shade: Option<Shade>,
    pub shade_deficit: i32,
    pub cancellation_viable: bool,
}
```

Update all LineStatus construction sites to initialize the new fields to defaults (None/0/false).

- [ ] **Step 2: Add analyze_line_shapes**

```rust
fn analyze_line_shapes(grid: &PlayerGrid, positions: &[(usize, usize)], ctx: &EliminationContext) -> LineStatus {
    let mut face_up_count = 0usize;
    let mut face_down_count = 0usize;
    let mut wild_count = 0usize;
    let mut shape_target: Option<Shape> = None;
    let mut shade_target: Option<Shade> = None;
    let mut matching_viable = true;
    let mut shaded_count = std::collections::HashMap::<Shape, i32>::new();
    let mut unshaded_count = std::collections::HashMap::<Shape, i32>::new();

    for &(r, c) in positions {
        match grid.get(r, c) {
            Some(gc) if gc.face_up => {
                face_up_count += 1;
                match &gc.card {
                    Card::Shape(shape, shade) => {
                        // Track matching viability
                        if matching_viable {
                            match &shape_target {
                                Some(existing) if existing != shape => matching_viable = false,
                                None => shape_target = Some(shape.clone()),
                                _ => {}
                            }
                            if ctx.shade_matters && matching_viable {
                                match &shade_target {
                                    Some(existing) if existing != shade => matching_viable = false,
                                    None => shade_target = Some(shade.clone()),
                                    _ => {}
                                }
                            }
                        }
                        // Track cancellation counts
                        match shade {
                            Shade::Shaded => *shaded_count.entry(shape.clone()).or_insert(0) += 1,
                            Shade::Unshaded => *unshaded_count.entry(shape.clone()).or_insert(0) += 1,
                        }
                    }
                    Card::Wild => wild_count += 1,
                    Card::WildShaded => wild_count += 1,
                    Card::WildUnshaded => wild_count += 1,
                    Card::Number(_) => matching_viable = false,
                }
            }
            Some(_) => face_down_count += 1,
            None => {}
        }
    }

    // Compute shade deficit for cancellation
    let mut total_shaded_deficit = 0i32;
    let mut total_unshaded_deficit = 0i32;
    let all_shapes: std::collections::HashSet<&Shape> =
        shaded_count.keys().chain(unshaded_count.keys()).collect();
    for shape in &all_shapes {
        let s = shaded_count.get(*shape).copied().unwrap_or(0);
        let u = unshaded_count.get(*shape).copied().unwrap_or(0);
        if s > u { total_unshaded_deficit += s - u; }
        else { total_shaded_deficit += u - s; }
    }
    let shade_deficit = total_shaded_deficit - total_unshaded_deficit;
    let total_deficit = total_shaded_deficit + total_unshaded_deficit;

    let cancellation_viable = if !ctx.allow_cancellation {
        false
    } else if positions.len() % 2 != 0 && face_down_count == 0 {
        false // odd completed line can't cancel
    } else {
        // With face-down cards remaining, cancellation could still work
        total_deficit <= (wild_count + face_down_count) as i32
    };

    LineStatus {
        positions: positions.to_vec(),
        face_up_count,
        face_down_count,
        current_sum: 0,
        wild_count,
        gap: 0,
        gap_achievable: false,
        cards_needed: face_down_count,
        matching_value: None,
        matching_viable,
        matching_shape: shape_target,
        matching_shade: shade_target,
        shade_deficit,
        cancellation_viable,
    }
}
```

- [ ] **Step 3: Update analyze_line to dispatch by mode**

```rust
fn analyze_line(grid: &PlayerGrid, positions: &[(usize, usize)], ctx: &EliminationContext) -> LineStatus {
    match ctx.game_mode {
        GameMode::Numbers => analyze_line_numbers(grid, positions, ctx),
        GameMode::Shapes => analyze_line_shapes(grid, positions, ctx),
    }
}
```

Rename the existing `analyze_line` body to `analyze_line_numbers`.

- [ ] **Step 4: Update score_line for Shapes mode**

`score_line` already works on LineStatus fields generically — `face_down_count`, `matching_viable`, `gap_achievable`. For Shapes mode, `gap_achievable` is always false (no sum-to-zero), but `cancellation_viable` fills a similar role. Update:

```rust
fn score_line(status: &LineStatus) -> f64 {
    let total = status.positions.len();
    if total == 0 { return 0.0; }

    // Use cancellation_viable as an alternative "completable" path
    let sum_path = status.gap_achievable;
    let cancel_path = status.cancellation_viable;
    let match_path = status.matching_viable;

    match status.face_down_count {
        0 => {
            if sum_path || cancel_path { return 100.0; }
            if match_path && (status.matching_value.is_some() || status.matching_shape.is_some()) {
                return 100.0;
            }
            0.0
        }
        1 => {
            let base = 70.0;
            let length_bonus = if total <= 2 { 20.0 } else if total <= 3 { 15.0 } else { 10.0 };
            let matching_bonus = if match_path { 10.0 } else { 0.0 };
            if !sum_path && !cancel_path {
                if match_path { return base + length_bonus + matching_bonus; }
                return 0.0;
            }
            base + length_bonus + matching_bonus
        }
        2 => {
            if !sum_path && !cancel_path && !match_path { return 0.0; }
            let base = 30.0;
            let progress = (total - 2) as f64 / total as f64;
            let matching_bonus = if match_path && total <= 3 { 10.0 } else { 0.0 };
            base + progress * 30.0 + matching_bonus
        }
        _ => {
            if !sum_path && !cancel_path && !match_path { return 0.0; }
            let progress = (total - status.face_down_count) as f64 / total as f64;
            5.0 + progress * 15.0
        }
    }
}
```

- [ ] **Step 5: Add card_fits_line_shapes**

```rust
fn card_fits_line_shapes(card: &Card, line: &LineStatus, ctx: &EliminationContext) -> f64 {
    if line.face_down_count == 0 { return 0.0; }

    let total_slots = line.positions.len();
    let known_after = total_slots - (line.face_down_count - 1);
    let progress = known_after as f64 / total_slots as f64;

    // Check matching path
    if line.matching_viable {
        let matches = match card {
            Card::Shape(shape, shade) => {
                let shape_ok = line.matching_shape.as_ref().map_or(true, |s| s == shape);
                let shade_ok = if ctx.shade_matters {
                    line.matching_shade.as_ref().map_or(true, |s| s == shade)
                } else {
                    true
                };
                shape_ok && shade_ok
            }
            Card::Wild => true,
            Card::WildShaded => !ctx.shade_matters || line.matching_shade.as_ref().map_or(true, |s| *s == Shade::Shaded),
            Card::WildUnshaded => !ctx.shade_matters || line.matching_shade.as_ref().map_or(true, |s| *s == Shade::Unshaded),
            Card::Number(_) => false,
        };
        if matches {
            if line.face_down_count == 1 { return 100.0; }
            return 40.0 + progress * 40.0;
        }
    }

    // Check cancellation path
    if line.cancellation_viable {
        let helps_cancel = match card {
            Card::Shape(_, shade) => {
                // Does this shade help reduce the deficit?
                (line.shade_deficit > 0 && *shade == Shade::Shaded) ||
                (line.shade_deficit < 0 && *shade == Shade::Unshaded) ||
                line.shade_deficit == 0
            }
            Card::Wild | Card::WildShaded | Card::WildUnshaded => true,
            Card::Number(_) => false,
        };
        if helps_cancel {
            return 10.0 + progress * 30.0;
        }
    }

    0.0
}
```

Update `card_fits_line` to dispatch:
```rust
pub fn card_fits_line(card: &Card, line: &LineStatus, ctx: &EliminationContext) -> f64 {
    match ctx.game_mode {
        GameMode::Numbers => card_fits_line_numbers(card, line, ctx),
        GameMode::Shapes => card_fits_line_shapes(card, line, ctx),
    }
}
```

Rename existing body to `card_fits_line_numbers`.

- [ ] **Step 6: Update best_placement for Shapes mode**

The existing `best_placement` logic references `card_value`, `gap`, `gap_improvement`. For Shapes mode, add a parallel path inside `best_placement` (or dispatch internally). The key change: in Shapes mode, skip the gap-based scoring for face-up replacement and use shape-matching heuristics instead:

```rust
// In the face-up replacement branch, add mode check:
if ctx.game_mode == GameMode::Shapes {
    // Shapes: replacing a face-up card is useful if the new card
    // contributes to a matching or cancellation line
    let new_fit = card_fits_line(card, line, ctx);
    if new_fit > 0.0 {
        score += new_fit * 0.5; // Lower weight than face-down placement
    }
} else {
    // Existing Numbers gap-improvement logic...
}
```

Also update the "Bonus: replacing a high-value card" section to skip in Shapes mode (all cards have equal value).

- [ ] **Step 7: Update choose_discard_from_eliminated in strategy/mod.rs**

```rust
pub fn choose_discard_from_eliminated(
    config: &PlayerConfig,
    eliminated_cards: &[Card],
    ctx: &EliminationContext,
    rng: &mut impl Rng,
) -> usize {
    if eliminated_cards.len() <= 1 { return 0; }
    if !should_play_smart(config.skill, rng) {
        return rng.gen_range(0..eliminated_cards.len());
    }
    match ctx.game_mode {
        GameMode::Numbers => {
            // Discard highest absolute value, never Wild
            let mut best_idx = 0;
            let mut best_score = i32::MIN;
            for (i, card) in eliminated_cards.iter().enumerate() {
                let score = match card {
                    Card::Number(v) => v.abs(),
                    _ => -100,
                };
                if score > best_score { best_score = score; best_idx = i; }
            }
            best_idx
        }
        GameMode::Shapes => {
            // Discard any non-wild card (all shapes equal value). Prefer keeping wilds.
            for (i, card) in eliminated_cards.iter().enumerate() {
                if matches!(card, Card::Shape(_, _)) { return i; }
            }
            0 // all wilds, just discard first
        }
    }
}
```

Update `choose_discard_with_opponent` similarly to pass `ctx` through.

- [ ] **Step 8: Update opportunist.rs for Shapes wild handling**

In `choose_draw_source`, the "Always take a Wild" check should also take WildShaded/WildUnshaded:

```rust
if matches!(card, Card::Wild | Card::WildShaded | Card::WildUnshaded) {
    return DrawSource::DiscardPile;
}
```

Also remove the "Always take a 0" line (only meaningful for Numbers). Gate it:
```rust
if ctx.game_mode == GameMode::Numbers {
    if card_value == 0 { return DrawSource::DiscardPile; }
}
```

- [ ] **Step 9: Add Shapes-mode branch to blind_draw_expected_score in calculator.rs**

The calculator's `blind_draw_expected_score` iterates `neg_min..=pos_max` which produces `0..=0` in Shapes mode (broken). Add a Shapes-mode branch that samples representative shape cards:

```rust
fn blind_draw_expected_score(grid: &PlayerGrid, ctx: &EliminationContext, skill: f64, _rng: &mut impl Rng) -> f64 {
    match ctx.game_mode {
        GameMode::Numbers => {
            // Existing logic using ctx.neg_min..=ctx.pos_max...
        }
        GameMode::Shapes => {
            let mut total_score = 0.0f64;
            let mut count = 0.0f64;
            // Sample each shape type
            for shape in &[Shape::Circle, Shape::Square, Shape::Triangle, Shape::Rectangle] {
                for shade in &[Shade::Unshaded, Shade::Shaded] {
                    let card = Card::Shape(shape.clone(), shade.clone());
                    let (_, score) = best_placement(&card, grid, ctx);
                    total_score += score;
                    count += 1.0;
                }
            }
            // Sample wilds
            for wild in &[Card::Wild, Card::WildShaded, Card::WildUnshaded] {
                let (_, score) = best_placement(wild, grid, ctx);
                total_score += score * 0.3; // wilds are rarer
                count += 0.3;
            }
            if count > 0.0 { total_score / count } else { 0.0 }
        }
    }
}
```

Also update `cascade_score` to pass `ctx` to `find_eliminations`:

```rust
let eliminations = sim_grid.find_eliminations(allow_matching, allow_diagonal, ctx);
```

- [ ] **Step 10: Update fallback_action for Shapes mode**

In `opportunist.rs: fallback_action`, the `card_abs <= 3` heuristic is Numbers-specific. For Shapes, all cards are equal value. Update:

```rust
pub(super) fn fallback_action(drawn_card: &Card, grid: &PlayerGrid, ctx: &EliminationContext, rng: &mut impl Rng) -> TurnAction {
    let face_down = grid.face_down_positions();

    if ctx.game_mode == GameMode::Shapes {
        // Shapes: always place in face-down slot if available
        if !face_down.is_empty() {
            let idx = rng.gen_range(0..face_down.len());
            return TurnAction::ReplaceCard { row: face_down[idx].0, col: face_down[idx].1 };
        }
        let occupied = grid.occupied_positions();
        let idx = rng.gen_range(0..occupied.len());
        return TurnAction::ReplaceCard { row: occupied[idx].0, col: occupied[idx].1 };
    }

    // Existing Numbers fallback logic...
```

- [ ] **Step 10: Write tests for Shapes-mode line scoring**

```rust
#[test]
fn test_shapes_matching_line_scores_high() {
    // All circles face-up → score 100
    let cards: Vec<Card> = vec![
        Card::Shape(Shape::Circle, Shade::Shaded), Card::Shape(Shape::Circle, Shade::Shaded),
        Card::Shape(Shape::Circle, Shade::Shaded), Card::Shape(Shape::Circle, Shade::Shaded),
        // rest don't matter
        Card::Number(1), Card::Number(1), Card::Number(1), Card::Number(1),
        Card::Number(1), Card::Number(1), Card::Number(1), Card::Number(1),
        Card::Number(1), Card::Number(1), Card::Number(1), Card::Number(1),
    ];
    let mut grid = PlayerGrid::new_no_flips(cards);
    for r in 0..4 { for c in 0..4 { grid.flip_card(r, c); } }

    let ctx = EliminationContext {
        game_mode: GameMode::Shapes,
        neg_min: 0, pos_max: 0,
        shade_matters: true, allow_cancellation: false,
    };
    let lines = score_all_lines(&grid, &ctx);
    assert!(lines[0].1 >= 99.0, "All-matching shapes row should score ~100");
}
```

- [ ] **Step 11: Run `cargo test` — all tests pass**

Run: `cd src-tauri && cargo test`
Expected: All pass.

- [ ] **Step 12: Commit**

```bash
git add src-tauri/src/engine/strategy/
git commit -m "feat: add Shapes-mode line scoring, card fitting, and discard heuristics"
```

---

## Task 6: Game Loop — Mode-Aware Round End and Scoring

**Files:**
- Modify: `src-tauri/src/engine/game.rs`

- [ ] **Step 1: Update check_round_end_trigger for Shapes mode**

```rust
fn check_round_end_trigger(state: &mut RoundState, player_idx: usize, game_mode: &GameMode) {
    if state.round_ended { return; }

    let grid = &state.players[player_idx].grid;
    let remaining = grid.remaining_card_count();

    let triggered = match game_mode {
        GameMode::Numbers => (remaining <= 4 && grid.all_face_up()) || remaining == 0,
        GameMode::Shapes => remaining == 0,
    };

    if triggered {
        state.round_ended = true;
        state.trigger_player = Some(player_idx);
        state.players[player_idx].went_out_first = true;
    }
}
```

Update call site in `play_turn` to pass `&config.game_mode`.

- [ ] **Step 2: Update score_round for Shapes mode**

```rust
fn score_round(config: &GameConfig, state: &RoundState) -> Vec<i32> {
    state.players.iter().map(|p| {
        let mut score = match config.scoring_mode {
            ScoringMode::Basic => p.grid.remaining_card_count() as i32,
            ScoringMode::Expert => {
                p.grid.occupied_positions().iter()
                    .map(|&(r, c)| p.grid.get(r, c).map(|gc| gc.card.score_value()).unwrap_or(0))
                    .sum::<i32>()
            }
        };

        // Going-out bonus: Numbers mode only
        if config.game_mode == GameMode::Numbers && p.went_out_first {
            score -= 2;
        }

        score
    }).collect()
}
```

- [ ] **Step 3: Run `cargo test` — all game tests pass**

Run: `cd src-tauri && cargo test`
Expected: All pass. Numbers behavior unchanged.

- [ ] **Step 4: Write Shapes-mode game smoke test**

```rust
#[test]
fn test_shapes_game_runs_to_completion() {
    let mut config = GameConfig::default();
    config.game_mode = GameMode::Shapes;
    config.deck = DeckConfig::shapes_scaled(4);
    config.allow_matching_elimination = true;
    config.allow_cancellation = true;
    config.shade_matters = true;
    config.allow_diagonal_elimination = false;
    let mut rng = rand::thread_rng();
    let result = play_game(&config, &mut rng);

    assert_eq!(result.player_scores.len(), 4);
    assert!(result.total_turns > 0);
    // No going-out bonus in Shapes mode
    for round in &result.round_results {
        for &score in &round.player_round_scores {
            assert!(score >= 0, "Shapes scores should never be negative (no bonus)");
        }
    }
}

#[test]
fn test_shapes_beginner_no_wilds() {
    let mut config = GameConfig::default();
    config.game_mode = GameMode::Shapes;
    config.deck = DeckConfig::shapes_scaled(4);
    // Beginner: shade_matters=false, no cancellation, no diagonal
    config.shade_matters = false;
    config.allow_cancellation = false;
    config.allow_diagonal_elimination = false;
    // Zero out wilds for beginner
    if let DeckConfig::Shapes { ref mut wild_count, ref mut wild_shaded_count, ref mut wild_unshaded_count, .. } = config.deck {
        *wild_count = 0;
        *wild_shaded_count = 0;
        *wild_unshaded_count = 0;
    }
    let mut rng = rand::thread_rng();
    let result = play_game(&config, &mut rng);
    assert!(result.total_turns > 0);
}
```

- [ ] **Step 5: Run tests, verify new tests pass**

Run: `cd src-tauri && cargo test`
Expected: All pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/engine/game.rs
git commit -m "feat: mode-aware round end trigger and scoring for Shapes game mode"
```

---

## Task 7: Interactive Play State — Shapes Support

**Files:**
- Modify: `src-tauri/src/interactive/state.rs`

- [ ] **Step 1: Update card_to_view for new Card variants**

```rust
fn card_to_view(card: &Card) -> CardView {
    CardView {
        display: format!("{}", card),
        value: match card {
            Card::Number(v) => Some(*v),
            _ => None,
        },
        card_type: match card {
            Card::Number(_) => "number".to_string(),
            Card::Wild => "wild".to_string(),
            Card::WildShaded => "wild_shaded".to_string(),
            Card::WildUnshaded => "wild_unshaded".to_string(),
            Card::Shape(_, _) => "shape".to_string(),
        },
    }
}
```

- [ ] **Step 2: Update build_grid_view for new Card variants**

In `build_grid_view`, the `card_type` match needs the new variants:

```rust
card_type: Some(match &gc.card {
    Card::Number(_) => "number".to_string(),
    Card::Wild => "wild".to_string(),
    Card::WildShaded => "wild_shaded".to_string(),
    Card::WildUnshaded => "wild_unshaded".to_string(),
    Card::Shape(_, _) => "shape".to_string(),
}),
```

- [ ] **Step 3: Update check_round_end_trigger in interactive state**

```rust
fn check_round_end_trigger(&mut self, player_idx: usize) {
    if self.round_ended { return; }

    let grid = &self.players[player_idx].grid;
    let remaining = grid.remaining_card_count();

    let triggered = match self.config.game_mode {
        GameMode::Numbers => (remaining <= 4 && grid.all_face_up()) || remaining == 0,
        GameMode::Shapes => remaining == 0,
    };

    if triggered {
        self.round_ended = true;
        self.trigger_player = Some(player_idx);
        self.players[player_idx].went_out_first = true;
        let name = self.player_name(player_idx);
        self.action_log.push(format!("{} triggered round end! ({} cards left). Each other player gets one more turn.", name, remaining));
    }
}
```

- [ ] **Step 4: Update score_round in interactive state**

Add game_mode gating on the going-out bonus:

```rust
if self.config.game_mode == GameMode::Numbers && p.went_out_first {
    score -= 2;
}
```

- [ ] **Step 5: Update elimination reason display**

In `check_eliminations_human` and `check_eliminations_ai`, add the new variant:

```rust
let reason = match &elim.reason {
    crate::engine::grid::EliminationReason::SumToZero => "sum-to-zero",
    crate::engine::grid::EliminationReason::AllMatching => "all-matching",
    crate::engine::grid::EliminationReason::Cancellation => "cancellation",
};
```

- [ ] **Step 6: Update best_discard_idx for Shapes mode**

```rust
fn best_discard_idx(&self, cards: &[Card]) -> usize {
    if cards.len() <= 1 { return 0; }
    match self.config.game_mode {
        GameMode::Numbers => {
            // Existing: discard highest abs, never Wild
            let mut best_idx = 0;
            let mut best_score = i32::MIN;
            for (i, card) in cards.iter().enumerate() {
                let score = match card {
                    Card::Number(v) => v.abs(),
                    _ => -100,
                };
                if score > best_score { best_score = score; best_idx = i; }
            }
            best_idx
        }
        GameMode::Shapes => {
            // Prefer discarding non-wild cards
            for (i, card) in cards.iter().enumerate() {
                if matches!(card, Card::Shape(_, _)) { return i; }
            }
            0
        }
    }
}
```

- [ ] **Step 7: Run `cargo test` — all tests pass**

Run: `cd src-tauri && cargo test`
Expected: All pass.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/interactive/state.rs
git commit -m "feat: interactive play mode supports Shapes card display and mode-aware logic"
```

---

## Task 8: Frontend — Game Mode Toggle and Config Panel

**Files:**
- Modify: `src/js/config-panel.js`
- Modify: `src/js/app.js`
- Modify: `src/index.html` (for new HTML elements)

- [ ] **Step 1: Add shapes deck presets to config-panel.js**

Add at the top of the file:

```javascript
const SHAPES = ['Circle', 'Square', 'Triangle', 'Rectangle'];
const SHADES = ['Unshaded', 'Shaded'];

const SHAPES_DECK_PRESETS_BY_PLAYERS = {
  2: { perType: 8, wild: 4, wildShaded: 4, wildUnshaded: 4 },
  3: { perType: 11, wild: 5, wildShaded: 5, wildUnshaded: 5 },
  4: { perType: 14, wild: 6, wildShaded: 6, wildUnshaded: 6 },
  5: { perType: 17, wild: 8, wildShaded: 8, wildUnshaded: 8 },
  6: { perType: 20, wild: 9, wildShaded: 9, wildUnshaded: 9 },
};

const SHAPES_TIER_PRESETS = {
  Beginner:     { shadematters: false, matching: true, cancellation: false, diagonal: false, wilds: false },
  Intermediate: { shadematters: true,  matching: true, cancellation: false, diagonal: false, wilds: true },
  Advanced:     { shadematters: true,  matching: true, cancellation: true,  diagonal: false, wilds: true },
  Expert:       { shadematters: true,  matching: true, cancellation: true,  diagonal: true,  wilds: true },
};
```

- [ ] **Step 2: Add game mode toggle HTML**

Add to `index.html` config section (before deck config):

```html
<div class="config-group">
  <label>Game Mode</label>
  <select id="game-mode" onchange="onGameModeChange()">
    <option value="Numbers">Numbers</option>
    <option value="Shapes">Shapes</option>
  </select>
</div>
```

Add Shapes-specific controls (initially hidden):

```html
<div id="shapes-config" style="display:none">
  <div class="config-group">
    <label>Tier</label>
    <select id="shapes-tier" onchange="applyShapesTier()">
      <option value="Beginner">Beginner (Ages 3-5)</option>
      <option value="Intermediate">Intermediate (Ages 4-6)</option>
      <option value="Advanced" selected>Advanced (Ages 5-8)</option>
      <option value="Expert">Expert (Ages 7+)</option>
    </select>
  </div>
  <div class="config-group">
    <label>Shade Matters</label>
    <input type="checkbox" id="shade-matters" checked />
  </div>
  <div class="config-group">
    <label>Allow Cancellation</label>
    <input type="checkbox" id="allow-cancellation" checked />
  </div>
</div>
```

- [ ] **Step 3: Add buildShapeQuantityTable function**

```javascript
function buildShapeQuantityTable() {
  const container = document.getElementById('card-quantity-table');
  const gameMode = document.getElementById('game-mode').value;
  if (gameMode !== 'Shapes') return;

  let html = '<div class="quantity-table">';
  for (const shape of SHAPES) {
    for (const shade of SHADES) {
      const id = `shape-qty-${shape}-${shade}`;
      const label = shade === 'Shaded' ? `■ ${shape}` : shape;
      html += `
        <div class="quantity-cell">
          <span class="card-value ${shade.toLowerCase()}">${label}</span>
          <input type="number" class="shape-qty" data-shape="${shape}" data-shade="${shade}"
                 id="${id}" value="14" min="0" max="50" oninput="updateTotalCards()" />
        </div>`;
    }
  }
  html += '</div>';
  container.innerHTML = html;
  updateTotalCards();
}
```

- [ ] **Step 4: Add onGameModeChange function**

```javascript
function onGameModeChange() {
  const mode = document.getElementById('game-mode').value;
  const numbersConfig = document.getElementById('numbers-config');
  const shapesConfig = document.getElementById('shapes-config');
  const scoringMode = document.getElementById('scoring-mode');

  if (mode === 'Shapes') {
    numbersConfig.style.display = 'none';
    shapesConfig.style.display = '';
    scoringMode.closest('.config-group').style.display = 'none';
    applyShapesTier();
    buildShapeQuantityTable();
  } else {
    numbersConfig.style.display = '';
    shapesConfig.style.display = 'none';
    scoringMode.closest('.config-group').style.display = '';
    buildCardQuantityTable();
  }
}
```

Note: wrap existing Numbers-specific controls (neg-min, pos-max, scoring-mode) in a `<div id="numbers-config">` wrapper in index.html.

- [ ] **Step 5: Add applyShapesTier function**

```javascript
function applyShapesTier() {
  const tier = document.getElementById('shapes-tier').value;
  const preset = SHAPES_TIER_PRESETS[tier];
  if (!preset) return;

  document.getElementById('shade-matters').checked = preset.shadematters;
  document.getElementById('allow-matching').checked = preset.matching;
  document.getElementById('allow-cancellation').checked = preset.cancellation;
  document.getElementById('allow-diagonal').checked = preset.diagonal;

  // Apply deck preset for current player count
  const count = parseInt(document.getElementById('player-count').value);
  applyShapesDeckPreset(count, preset.wilds);
}
```

- [ ] **Step 6: Add applyShapesDeckPreset function**

```javascript
function applyShapesDeckPreset(playerCount, includeWilds) {
  const preset = SHAPES_DECK_PRESETS_BY_PLAYERS[playerCount];
  if (!preset) return;

  document.querySelectorAll('.shape-qty').forEach(input => {
    input.value = preset.perType;
  });

  document.getElementById('shapes-wild-count').value = includeWilds ? preset.wild : 0;
  const wsEl = document.getElementById('shapes-wild-shaded-count');
  const wuEl = document.getElementById('shapes-wild-unshaded-count');
  if (wsEl) wsEl.value = includeWilds ? preset.wildShaded : 0;
  if (wuEl) wuEl.value = includeWilds ? preset.wildUnshaded : 0;

  updateTotalCards();
}
```

- [ ] **Step 7: Update updateTotalCards for Shapes mode**

```javascript
function updateTotalCards() {
  let total = 0;
  const mode = document.getElementById('game-mode').value;

  if (mode === 'Shapes') {
    document.querySelectorAll('.shape-qty').forEach(input => {
      total += parseInt(input.value) || 0;
    });
    total += parseInt(document.getElementById('shapes-wild-count')?.value) || 0;
    total += parseInt(document.getElementById('shapes-wild-shaded-count')?.value) || 0;
    total += parseInt(document.getElementById('shapes-wild-unshaded-count')?.value) || 0;
  } else {
    document.querySelectorAll('.card-qty').forEach(input => {
      total += parseInt(input.value) || 0;
    });
    total += parseInt(document.getElementById('wild-count').value) || 0;
  }

  document.getElementById('total-cards').textContent = total.toLocaleString();
}
```

- [ ] **Step 8: Update buildConfigFromUI for Shapes mode**

```javascript
function buildConfigFromUI() {
  const playerCount = parseInt(document.getElementById('player-count').value);
  const gameMode = document.getElementById('game-mode').value;

  let deck;
  if (gameMode === 'Shapes') {
    const shapeQuantities = [];
    document.querySelectorAll('.shape-qty').forEach(input => {
      const count = parseInt(input.value) || 0;
      if (count > 0) {
        shapeQuantities.push([input.dataset.shape, input.dataset.shade, count]);
      }
    });
    deck = {
      type: 'Shapes',
      shape_quantities: shapeQuantities,
      wild_count: parseInt(document.getElementById('shapes-wild-count')?.value) || 0,
      wild_shaded_count: parseInt(document.getElementById('shapes-wild-shaded-count')?.value) || 0,
      wild_unshaded_count: parseInt(document.getElementById('shapes-wild-unshaded-count')?.value) || 0,
    };
  } else {
    const cardQuantities = [];
    document.querySelectorAll('.card-qty').forEach(input => {
      const value = parseInt(input.dataset.value);
      const count = parseInt(input.value) || 0;
      if (count > 0) { cardQuantities.push([value, count]); }
    });
    deck = {
      type: 'Numbers',
      neg_min: parseInt(document.getElementById('neg-min').value),
      pos_max: parseInt(document.getElementById('pos-max').value),
      card_quantities: cardQuantities,
      wild_count: parseInt(document.getElementById('wild-count').value) || 0,
    };
  }

  const players = [];
  for (let i = 0; i < playerCount; i++) {
    players.push({
      archetype: document.getElementById(`archetype-${i}`).value,
      skill: parseInt(document.getElementById(`skill-${i}`).value) / 100,
      flip_strategy: document.getElementById(`flip-strategy-${i}`).value,
    });
  }

  return {
    game_mode: gameMode,
    deck: deck,
    player_count: playerCount,
    allow_matching_elimination: document.getElementById('allow-matching').checked,
    allow_diagonal_elimination: document.getElementById('allow-diagonal').checked,
    allow_cancellation: document.getElementById('allow-cancellation')?.checked || false,
    shade_matters: document.getElementById('shade-matters')?.checked || false,
    scoring_mode: gameMode === 'Shapes' ? 'Basic' : document.getElementById('scoring-mode').value,
    starting_order: document.getElementById('starting-order').value,
    players: players,
    max_turns_per_round: 500,
    round_multiplier: parseInt(document.getElementById('round-multiplier')?.value) || 1,
  };
}
```

- [ ] **Step 9: Update player-count change handler**

When player count changes in Shapes mode, apply shapes deck preset:

```javascript
document.getElementById('player-count').addEventListener('change', () => {
  const count = parseInt(document.getElementById('player-count').value);
  const mode = document.getElementById('game-mode').value;
  if (mode === 'Shapes') {
    const tier = document.getElementById('shapes-tier').value;
    const preset = SHAPES_TIER_PRESETS[tier];
    applyShapesDeckPreset(count, preset.wilds);
  } else {
    document.getElementById('deck-preset').value = 'auto';
    applyDeckPresetForPlayers(count);
  }
  buildPlayerPanels();
});
```

- [ ] **Step 10: Add wild-shaded-count and wild-unshaded-count inputs to HTML**

Add to the shapes config section:

```html
<div class="config-group wild-counts" id="shapes-wild-counts">
  <label>Wild</label>
  <input type="number" id="shapes-wild-count" value="6" min="0" max="50" oninput="updateTotalCards()" />
  <label>Wild Shaded</label>
  <input type="number" id="shapes-wild-shaded-count" value="6" min="0" max="50" oninput="updateTotalCards()" />
  <label>Wild Unshaded</label>
  <input type="number" id="shapes-wild-unshaded-count" value="6" min="0" max="50" oninput="updateTotalCards()" />
</div>
```

**Important:** Use `shapes-wild-count`, `shapes-wild-shaded-count`, `shapes-wild-unshaded-count` to avoid collisions with the existing Numbers `wild-count` element. Update all JS references accordingly (in `updateTotalCards`, `buildConfigFromUI`, `applyShapesDeckPreset`).

- [ ] **Step 11: Test manually — launch `cargo tauri dev`, switch mode toggle**

Run: `cargo tauri dev`
- Verify Numbers mode works as before
- Switch to Shapes mode: config panel shows tier selector, shape quantities
- Run a Shapes simulation: no crashes, stats display correctly
- Switch back to Numbers: everything restored

- [ ] **Step 12: Commit**

```bash
git add src/js/config-panel.js src/js/app.js src/index.html
git commit -m "feat: frontend game mode toggle with Shapes tier selector and deck config"
```

---

## Task 9: Frontend — Interactive Play Shape Card Rendering

**Files:**
- Modify: `src/js/play-panel.js`

- [ ] **Step 1: Update card rendering for shape types**

Find the card display logic in play-panel.js and add shape card rendering. Cards come through as `card_type: "shape" | "wild_shaded" | "wild_unshaded"` and `card: "Circle" | "Shaded Circle" | etc.`

Update the cell rendering to show shape names with appropriate styling:

```javascript
// In the cell rendering function, update the card display logic:
function cardDisplayClass(cardType) {
  switch (cardType) {
    case 'number': return 'card-number';
    case 'wild': return 'card-wild';
    case 'wild_shaded': return 'card-wild-shaded';
    case 'wild_unshaded': return 'card-wild-unshaded';
    case 'shape': return 'card-shape';
    default: return '';
  }
}
```

- [ ] **Step 2: Add CSS for shape card styling**

Add shape-specific styles — shaded shapes get a filled background, unshaded get an outline style.

- [ ] **Step 3: Test manually — play a Shapes game in interactive mode**

Run: `cargo tauri dev`
- Start a Shapes game in Play tab
- Verify shape cards display correctly (name visible, shaded/unshaded distinguishable)
- Verify eliminations fire and display correctly
- Verify round end works (only at 0 cards)

- [ ] **Step 4: Commit**

```bash
git add src/js/play-panel.js src/css/
git commit -m "feat: shape card rendering in interactive play mode"
```

---

## Task 10: Full Integration Test and Cleanup

**Files:**
- Modify: `src-tauri/src/engine/game.rs` (tests only)

- [ ] **Step 1: Run full test suite**

Run: `cd src-tauri && cargo test`
Expected: All tests pass.

- [ ] **Step 2: Run 100-game Shapes simulation smoke test**

```rust
#[test]
fn test_shapes_100_games_all_tiers() {
    let mut rng = rand::thread_rng();
    for tier in &["beginner", "intermediate", "advanced", "expert"] {
        let mut config = GameConfig::default();
        config.game_mode = GameMode::Shapes;
        config.deck = DeckConfig::shapes_scaled(4);
        match *tier {
            "beginner" => {
                config.shade_matters = false;
                config.allow_cancellation = false;
                config.allow_diagonal_elimination = false;
                config.allow_matching_elimination = true;
                if let DeckConfig::Shapes { ref mut wild_count, ref mut wild_shaded_count, ref mut wild_unshaded_count, .. } = config.deck {
                    *wild_count = 0; *wild_shaded_count = 0; *wild_unshaded_count = 0;
                }
            }
            "intermediate" => {
                config.shade_matters = true;
                config.allow_cancellation = false;
                config.allow_diagonal_elimination = false;
            }
            "advanced" => {
                config.shade_matters = true;
                config.allow_cancellation = true;
                config.allow_diagonal_elimination = false;
            }
            "expert" => {
                config.shade_matters = true;
                config.allow_cancellation = true;
                config.allow_diagonal_elimination = true;
            }
            _ => {}
        }

        for _ in 0..100 {
            let result = play_game(&config, &mut rng);
            assert_eq!(result.player_scores.len(), 4);
            assert!(result.total_turns > 0);
            for &score in &result.player_scores {
                assert!(score >= 0, "Shapes scores should never be negative");
            }
        }
    }
}
```

- [ ] **Step 3: Run `cargo test` — verify smoke test passes**

Run: `cd src-tauri && cargo test test_shapes_100_games_all_tiers -- --nocapture`
Expected: 400 games across 4 tiers, all pass.

- [ ] **Step 4: Fix any compiler warnings**

Run: `cd src-tauri && cargo check 2>&1 | grep warning`
Fix unused imports, dead code, etc.

- [ ] **Step 5: Manual end-to-end test**

Run: `cargo tauri dev`
- Run Numbers simulation → verify unchanged behavior
- Switch to Shapes → run simulation at each tier → verify stats look reasonable
- Play interactive Shapes game → verify cards, eliminations, round end, scoring
- Switch between modes repeatedly → verify no state bleed

- [ ] **Step 6: Final commit**

```bash
git add -A
git commit -m "feat: complete Shapes game mode integration — all tiers, simulation, and interactive play"
```

---

## Summary

| Task | Description | Key files |
|------|-------------|-----------|
| 1 | Card domain (Shape, Shade, new variants) | card.rs |
| 2 | DeckConfig enum, GameMode, EliminationContext | config.rs, card.rs |
| 3 | Shape elimination logic (matching + cancellation) | grid.rs |
| 4 | EliminationContext threading (mechanical refactor) | strategy/*, game.rs, state.rs |
| 5 | Shapes-mode line scoring and strategy heuristics | strategy/line_scoring.rs, strategy/mod.rs |
| 6 | Game loop: mode-aware round end and scoring | game.rs |
| 7 | Interactive play: Shapes card display and logic | interactive/state.rs |
| 8 | Frontend: game mode toggle and config panel | config-panel.js, app.js, index.html |
| 9 | Frontend: shape card rendering in play mode | play-panel.js |
| 10 | Integration test and cleanup | game.rs (tests) |

Tasks 1-3 build the Shapes foundation. Task 4 is a mechanical refactor. Task 5 adds the brain. Tasks 6-7 integrate with game flow. Tasks 8-9 wire up the frontend. Task 10 validates everything.
