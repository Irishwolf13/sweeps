# Initial Flip Strategy — AI Config + Human Manual Selection

## Problem
Currently, the 2 initial face-up cards are always chosen randomly for all players. The user wants AI players to have configurable flip strategies, and the human player to manually choose their 2 starting cards in interactive mode.

## Design

### Feature 1: AI Initial Flip Strategy

**New enum `FlipStrategy` in `config.rs`:**
```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum FlipStrategy {
    Random,
    SameColumn,
    SameRow,
    Corners,
    Diagonal,
}

impl Default for FlipStrategy {
    fn default() -> Self {
        FlipStrategy::Random
    }
}
```
- `Random` — pick any 2 random positions (current behavior, default)
- `SameColumn` — pick a random column, flip 2 random cells in it
- `SameRow` — pick a random row, flip 2 random cells in it
- `Corners` — pick 2 random corners from `(0,0)`, `(0,3)`, `(3,0)`, `(3,3)`
- `Diagonal` — pick a random diagonal (main or anti), then pick 2 random cells from it

**New field on `PlayerConfig`:**
```rust
#[serde(default)]
pub flip_strategy: FlipStrategy,
```
Default: `Random`. `#[serde(default)]` for backward compat with saved runs.

**Update `PlayerConfig::default()` and presets** — all presets use `Random`.

**Grid changes (`grid.rs`):**
- `PlayerGrid::new()` signature changes from `(cards, initial_face_up: usize, rng)` to `(cards, strategy: &FlipStrategy, rng)`.
- New function `flip_positions(strategy: &FlipStrategy, rng: &mut impl Rng) -> Vec<(usize, usize)>` that returns the 2 positions to flip based on the strategy. Called inside `new()`.
- For human manual selection, use `FlipStrategy::Random` when creating the grid (the 2 auto-flipped cards will be overridden — see Feature 2 below). Actually, simpler: add a `PlayerGrid::new_no_flips(cards)` constructor that creates a grid with all cards face-down. Used only for the human player in interactive mode.
- All callers updated:
  - `game.rs` `play_round()`: change loop from `for _ in 0..player_count` to `for i in 0..player_count` and pass `&config.players[i].flip_strategy`.
  - `interactive/state.rs` `start_round()`: AI players use their `FlipStrategy`, human player uses `new_no_flips()`.

**Frontend (`config-panel.js`):**
- New dropdown per player in the player strategy panels: "Initial Flip" with options matching enum variant names exactly for serde: `Random`, `SameColumn`, `SameRow`, `Corners`, `Diagonal`. Display labels: "Random", "Same Column", "Same Row", "Corners", "Diagonal".
- `buildConfigFromUI()` reads the dropdown value and includes `flip_strategy` in each player's config object.
- `PLAYER_PRESETS` updated to include `flipStrategy: 'Random'`.

### Feature 2: Human Manual Flip in Interactive Play

**New pending state in `interactive/state.rs`:**
- Add `ChooseInitialFlips { remaining: u8 }` to the `InternalPending` enum.
- At round start in `start_round()`: create the human player's grid with `PlayerGrid::new_no_flips(hand)` (all 16 cards face-down). Set pending to `ChooseInitialFlips { remaining: 2 }`.
- AI players' grids are initialized with their configured `FlipStrategy` as normal.

**New method on `InteractiveGame`: `human_flip_initial(row: usize, col: usize)`**
- Only valid during `ChooseInitialFlips` phase. Return error otherwise.
- Validate: cell must exist and be face-down. If already face-up, return error "Card is already face-up" without decrementing remaining.
- Validate: row/col in bounds (0-3). Return error if out of bounds.
- Flip the card face-up, decrement `remaining`.
- When `remaining` reaches 0, transition to normal play state (`ChooseDrawSource` if it's human's turn, `NotYourTurn` otherwise).

**New Tauri command: `play_flip_initial(row: usize, col: usize)`**
- Calls `game.human_flip_initial(row, col)`.
- Returns updated `PlayableGameState`.
- Registered in `main.rs` `generate_handler!`.

**PlayableGameState changes:**
- Add `"choose_initial_flips"` as a new `action_type` value in `PendingAction`.
- Add `flips_remaining: Option<u8>` field to `PendingAction` (only populated during this phase, `None` otherwise).

**Frontend play panel (`play-panel.js`):**
- During `choose_initial_flips` phase: show prompt "Click 2 cards to flip face-up" in the action log area.
- Update `canClickCell()` to handle this phase: `if (actionType === 'choose_initial_flips') return cell.state === 'face_down';`
- Clicking a valid face-down cell calls `tauriPlayFlipInitial(row, col)`.
- After both flips, state updates automatically trigger normal play UI.

**Bridge (`tauri-bridge.js`):**
- New function `tauriPlayFlipInitial(row, col)`.
