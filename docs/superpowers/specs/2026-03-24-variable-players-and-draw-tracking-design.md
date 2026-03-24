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

When player count changes, deck auto-populates with a preset. User can manually override after. Range is always -5 to 8 with the same bell-curve distribution shape, just scaled quantities.

| Players | Total Cards | Wild | Draw Pile Start | Buffer |
|---------|------------|------|----------------|--------|
| 2 | 90 | 6 | 57 | comfortable |
| 3 | 110 | 8 | 61 | comfortable |
| 4 | 130 | 10 | 65 | comfortable |
| 5 | 150 | 12 | 69 | comfortable |
| 6 | 170 | 14 | 73 | comfortable |

**Card quantities per preset:**

```
2 players (90 cards):
  -5:3  -4:4  -3:5  -2:6  -1:7  0:8  1:7  2:7  3:7  4:6  5:5  6:4  7:4  8:3  Wild:6

3 players (110 cards):
  -5:4  -4:5  -3:6  -2:7  -1:8  0:9  1:8  2:8  3:8  4:7  5:6  6:5  7:5  8:4  Wild:8

4 players (130 cards):
  -5:4  -4:6  -3:7  -2:9  -1:10  0:12  1:10  2:10  3:10  4:9  5:8  6:7  7:6  8:5  Wild:10

5 players (150 cards):
  -5:5  -4:7  -3:8  -2:10  -1:12  0:14  1:12  2:12  3:12  4:10  5:9  6:8  7:7  8:6  Wild:12

6 players (170 cards):
  -5:6  -4:8  -3:10  -2:12  -1:14  0:16  1:14  2:14  3:14  4:12  5:10  6:9  7:8  8:7  Wild:14
```

**Behavior:**
- Changing player count auto-applies the matching deck preset
- User can manually edit any quantity after
- Existing "Default" and "Original" deck preset buttons remain; the player-count-based preset is a third option applied automatically on count change
- `DeckConfig::validate` already checks `player_count * 16 + 20` — no change needed

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
Computed in the simulation runner from round results.

**Interactive PlayableGameState** — add field:
```rust
pub draw_pile_count: u32,  // current draw pile size, sent to frontend each turn
```
Already available from `self.draw_pile.len()` in state.rs.

### Play Panel: Per-AI Config

Replace the single "AI Difficulty" dropdown with individual configuration per AI opponent.

**Layout:**
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

- Player count selector: 2-6. Human is always Player 1.
- Each AI gets: archetype dropdown, skill slider (0-100%), flip strategy dropdown
- Quick-fill buttons: "All Beginner", "All Expert", "Mixed" (cycles through beginner→intermediate→advanced→expert)
- Changing player count adds/removes AI config rows
- Config is read when "Start Game" is clicked and sent to backend as `GameConfig.players`

### Config Panel (Simulation): Variable Player Count

- Player count dropdown already exists, just ensure it supports 2-6
- Changing count triggers: (1) auto-apply deck preset, (2) rebuild player config panels
- Player panels already dynamically build based on count via `buildPlayerPanels()`

### Frontend: Draw Pile Display

**Interactive play scoreboard:**
- Show "Draw pile: X cards" alongside existing round/turn info
- Updated after each turn from `playState.draw_pile_count`

**Simulation results:**
- Add "Avg Draw Pile Remaining" row to the summary stats table
- Displayed alongside existing metrics (avg turns/round, win rates, etc.)

## Files Changed

### Backend
- `src-tauri/src/engine/game.rs` — Set `draw_pile_remaining` in `RoundResult` at round end
- `src-tauri/src/engine/game.rs` — `RoundResult` struct: add `draw_pile_remaining: u32`
- `src-tauri/src/interactive/state.rs` — Add `draw_pile_count` to `PlayableGameState`, populate from `self.draw_pile.len()`
- `src-tauri/src/simulation/runner.rs` — Compute `avg_draw_pile_remaining` from round results
- `src-tauri/src/simulation/runner.rs` — `SimulationSummary` struct: add `avg_draw_pile_remaining: f64`

### Frontend
- `src/js/config-panel.js` — Add deck presets per player count, auto-apply on count change
- `src/js/play-panel.js` — Replace single AI preset with per-opponent config, player count selector, quick-fill buttons
- `src/js/app.js` — Display `avg_draw_pile_remaining` in simulation results, display `draw_pile_count` in play scoreboard

## Testing

- Unit test: each deck preset validates for its player count
- Unit test: `draw_pile_remaining` is populated correctly in RoundResult
- Integration test: 2-player and 6-player games complete without panics
- Integration test: draw pile never hits 0 (no reshuffling) with preset decks over 100 games
- Smoke test: simulation with 6 players completes, summary includes avg_draw_pile_remaining
