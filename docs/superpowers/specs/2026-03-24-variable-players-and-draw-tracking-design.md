# Variable Player Count, Per-AI Config, and Draw Pile Tracking

**Date:** 2026-03-24
**Status:** Draft

## Problem

The game is hardcoded to 4 players in practice. The play panel uses a single global AI preset instead of per-opponent configuration. There's no visibility into draw pile health during or after games.

## Goal

1. Support 2-6 players in both simulation and interactive play modes
2. Auto-scale deck presets per player count (no reshuffling needed)
3. Per-AI opponent configuration in play mode (archetype, skill, flip strategy)
4. Draw pile remaining count tracked per round and in aggregated stats

## Design

### Deck Presets Per Player Count

When player count changes, deck auto-populates with a preset. User can manually override after. Range is always -5 to 8 with the same bell-curve distribution shape, scaled quantities.

**Card quantities per preset (verified sums):**

```
2 players (90 total = 82 number + 8 wild):
  -5:3  -4:4  -3:5  -2:6  -1:8  0:10  1:8  2:8  3:7  4:6  5:5  6:4  7:4  8:4  Wild:8
  Deal: 32, Draw pile start: 57

3 players (112 total = 102 number + 10 wild):
  -5:4  -4:5  -3:6  -2:8  -1:9  0:11  1:9  2:9  3:9  4:8  5:7  6:6  7:5  8:6  Wild:10
  Deal: 48, Draw pile start: 63

4 players (132 total = 120 number + 12 wild):
  -5:5  -4:6  -3:8  -2:9  -1:11  0:13  1:11  2:11  3:10  4:9  5:8  6:7  7:6  8:6  Wild:12
  Deal: 64, Draw pile start: 67

5 players (154 total = 140 number + 14 wild):
  -5:6  -4:7  -3:9  -2:11  -1:13  0:15  1:13  2:13  3:12  4:11  5:9  6:8  7:7  8:6  Wild:14
  Deal: 80, Draw pile start: 73

6 players (176 total = 160 number + 16 wild):
  -5:7  -4:9  -3:11  -2:13  -1:15  0:17  1:15  2:15  3:14  4:12  5:11  6:9  7:8  8:8  Wild:16
  Deal: 96, Draw pile start: 79
```

| Players | Total | Dealt | Draw Pile Start | Min Required |
|---------|-------|-------|----------------|-------------|
| 2 | 90 | 32 | 57 | 52 |
| 3 | 112 | 48 | 63 | 68 |
| 4 | 132 | 64 | 67 | 84 |
| 5 | 154 | 80 | 73 | 100 |
| 6 | 176 | 96 | 79 | 116 |

**Behavior:**
- Changing player count auto-applies the matching deck preset
- User can manually edit any quantity after
- Existing "Default" and "Original" deck preset buttons remain
- `DeckConfig::validate` already checks `player_count * 16 + 20` — no change needed
- `DeckConfig::default()` in Rust updated to match the 4-player preset (132 cards)

### Backend: Draw Pile Tracking

**RoundResult** — add field:
```rust
pub draw_pile_remaining: u32,  // cards left in draw pile at round end
```
Set from `state.draw_pile.len()` when scoring the round in `game.rs`.

**SimulationSummary** — add field:
```rust
pub avg_draw_pile_remaining: f64,  // averaged across all rounds of all games
```
Computed in the simulation runner. Note: `effective_deck_usage` and `draw_pile_exhaustion_rate` already exist as percentage metrics — `avg_draw_pile_remaining` provides the concrete card count which is more intuitive.

**PlayableGameState** — `draw_pile_count: usize` already exists and is populated from `self.draw_pile.len()`. No backend change needed for interactive draw pile display.

### Play Panel: Per-AI Config and Variable Player Count

Replace the single "AI Difficulty" dropdown with individual configuration per AI opponent.

**Setup layout:**
```
Play a Game
──────────────
Players: [3 ▼]

You (Player 1) — Human

Player 2:  [Opportunist ▼]  Skill: [====85%====]  Flip: [Random ▼]
Player 3:  [Calculator  ▼]  Skill: [===100%====]  Flip: [Random ▼]

Quick fill: [All Beginner] [All Expert] [Mixed]

[Start Game]
```

- Player count selector: 2-6, `id="play-player-count"`. Human is always Player 1.
- Each AI gets: archetype dropdown, skill slider (0-100%), flip strategy dropdown
- Quick-fill buttons: "All Beginner", "All Expert", "Mixed" (cycles through presets)
- Changing player count adds/removes AI config rows
- `startPlayGame()` reads per-AI configs and sets `config.player_count` from the play panel selector (currently hardcoded to 4)

**Game board layout for variable player counts:**

The current play board uses a fixed compass layout (south/west/north/east) for exactly 4 players. This must be restructured for 2-6 players.

New approach: **linear layout with player grids in a scrollable row/wrap.** Human's grid is always prominent at bottom. AI grids displayed above in a flex-wrap container. This avoids the compass naming issue and scales naturally.

Player naming changes from compass directions to numbered:
- Player 1: "You" (human)
- Players 2-6: "Player 2 (AI)" through "Player 6 (AI)"

`state.rs::player_name()` updated to use numbered names instead of compass directions.

HTML changes in `src/index.html`:
- Play board section: replace fixed compass grid divs with a dynamic container
- `play-panel.js::renderPlayState()` dynamically creates grid containers based on player count
- Config panel: add options 5 and 6 to player-count dropdown
- Play setup: add player-count selector and per-AI config section

### Config Panel (Simulation): Variable Player Count

- Player count dropdown in `index.html`: add options for 5 and 6
- Changing count triggers: (1) auto-apply deck preset, (2) rebuild player config panels
- Player panels already dynamically build based on count via `buildPlayerPanels()`

### Frontend: Draw Pile Display

**Interactive play scoreboard:**
- Show "Draw pile: X cards" alongside existing round/turn info
- Read from existing `playState.draw_pile_count` field (already sent by backend)

**Simulation results:**
- Add "Avg Draw Pile Remaining" to summary stats in `app.js`
- Displayed alongside existing metrics (avg turns/round, win rates, etc.)

## Files Changed

### Backend
- `src-tauri/src/engine/config.rs` — Update `DeckConfig::default()` to match 4-player preset (132 cards)
- `src-tauri/src/engine/game.rs` — `RoundResult`: add `draw_pile_remaining: u32`, set at round end
- `src-tauri/src/simulation/runner.rs` — `SimulationSummary`: add `avg_draw_pile_remaining: f64`, compute from round results
- `src-tauri/src/interactive/state.rs` — Update `player_name()` to use numbered names instead of compass directions

### Frontend
- `src/index.html` — Add 5/6 to config player-count dropdown; add play-panel player count selector and per-AI config section; restructure play board from fixed compass to dynamic layout
- `src/js/config-panel.js` — Add deck presets per player count, auto-apply on count change
- `src/js/play-panel.js` — Per-opponent config UI, player count selector, quick-fill buttons, dynamic grid rendering for variable player counts
- `src/js/app.js` — Display `avg_draw_pile_remaining` in simulation results, display draw pile count in play scoreboard

## Testing

- Unit test: each deck preset validates for its player count (`DeckConfig::validate`)
- Unit test: `draw_pile_remaining` is populated correctly in RoundResult
- Integration test: 2-player and 6-player games complete without panics
- Integration test: draw pile never hits 0 (no reshuffling) with preset decks over 100 games per player count
- Smoke test: simulation with 6 players completes, summary includes `avg_draw_pile_remaining`
- Guard in `handleAllAiTurns()`: add turn counter limit to prevent infinite loop if state gets stuck
