mod calculator;
mod line_scoring;
mod methodical;
mod opportunist;

use rand::Rng;

use super::card::Card;
use super::config::{AiArchetype, PlayerConfig};
use super::grid::{EliminationType, PlayerGrid, SlideDirection};

pub use line_scoring::{LineStatus, score_all_lines, card_fits_line, best_placement, best_flip_target, needed_cards};

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
    methodical_state: &mut Option<MethodicalState>,
    rng: &mut impl Rng,
) -> DrawSource {
    match config.archetype {
        AiArchetype::Opportunist => opportunist::choose_draw_source(config, discard_top, grid, neg_min, pos_max, rng),
        AiArchetype::Methodical => {
            let state = methodical_state.get_or_insert_with(MethodicalState::new);
            methodical::choose_draw_source(config, discard_top, grid, neg_min, pos_max, state, rng)
        }
        AiArchetype::Calculator => calculator::choose_draw_source(config, discard_top, grid, neg_min, pos_max, rng),
    }
}

pub fn choose_action(
    config: &PlayerConfig,
    drawn_card: &Card,
    grid: &PlayerGrid,
    neg_min: i32,
    pos_max: i32,
    methodical_state: &mut Option<MethodicalState>,
    rng: &mut impl Rng,
) -> TurnAction {
    match config.archetype {
        AiArchetype::Opportunist => opportunist::choose_action(config, drawn_card, grid, neg_min, pos_max, rng),
        AiArchetype::Methodical => {
            let state = methodical_state.get_or_insert_with(MethodicalState::new);
            methodical::choose_action(config, drawn_card, grid, neg_min, pos_max, state, rng)
        }
        AiArchetype::Calculator => calculator::choose_action(config, drawn_card, grid, neg_min, pos_max, rng),
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
