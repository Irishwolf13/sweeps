use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

use crate::engine::card::{build_deck, Card};
use crate::engine::config::{GameConfig, ScoringMode};
use crate::engine::grid::{EliminationType, PlayerGrid, SlideDirection};
use crate::engine::strategy::{self, DrawSource, TurnAction};

// ── Serializable view types (sent to frontend) ──────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CellView {
    pub state: String,             // "empty", "face_down", "face_up"
    pub card: Option<String>,      // display string: "5", "-3", "Wild"
    pub value: Option<i32>,        // numeric value for Number cards, null for Wild
    pub card_type: Option<String>, // "number", "wild"
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GridView {
    pub cells: Vec<Vec<CellView>>,
    pub remaining: usize,
    pub eliminations: u32,
    pub all_face_up: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CardView {
    pub display: String,
    pub value: Option<i32>,
    pub card_type: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PendingAction {
    pub action_type: String, // "choose_draw_source", "handle_normal_card",
                              // "choose_slide_direction", "not_your_turn",
                              // "game_over", "round_over"
    pub drawn_card: Option<CardView>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayableGameState {
    pub round: u8,
    pub turn: u32,
    pub current_player: usize,
    pub player_names: Vec<String>,
    pub grids: Vec<GridView>,
    pub draw_pile_count: usize,
    pub discard_top: Option<CardView>,
    pub cumulative_scores: Vec<i32>,
    pub round_scores: Vec<Vec<i32>>,
    pub pending: PendingAction,
    pub action_log: Vec<String>,
    pub round_ended: bool,
    pub game_over: bool,
    pub winner: Option<usize>,
    pub round_trigger_player: Option<usize>,
}

// ── Internal state ──────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct PlayerState {
    grid: PlayerGrid,
    went_out_first: bool,
    cleared_all: bool,
    eliminations: u32,
}

#[derive(Clone, Debug)]
enum InternalPending {
    ChooseDrawSource,
    HandleNormalCard(Card),
    ChooseSlideDirection(EliminationType),
    NotYourTurn,
    RoundOver,
    GameOver,
}

#[derive(Clone, Debug)]
pub struct InteractiveGame {
    config: GameConfig,
    players: Vec<PlayerState>,
    draw_pile: Vec<Card>,
    discard_pile: Vec<Card>,
    current_player: usize,
    turn: u32,
    round: u8,
    round_ended: bool,
    trigger_player: Option<usize>,
    final_turns_given: usize,
    cumulative_scores: Vec<i32>,
    round_scores: Vec<Vec<i32>>,
    pending: InternalPending,
    action_log: Vec<String>,
    rng: StdRng,
    human_player: usize,
}

impl InteractiveGame {
    pub fn new(config: GameConfig) -> Self {
        let player_count = config.player_count as usize;
        let rng = StdRng::from_entropy();

        let mut game = InteractiveGame {
            config,
            players: Vec::new(),
            draw_pile: Vec::new(),
            discard_pile: Vec::new(),
            current_player: 0,
            turn: 0,
            round: 0,
            round_ended: false,
            trigger_player: None,
            final_turns_given: 0,
            cumulative_scores: vec![0; player_count],
            round_scores: Vec::new(),
            pending: InternalPending::ChooseDrawSource,
            action_log: vec!["Game started! Round 1 begins.".to_string()],
            rng,
            human_player: 0,
        };

        game.start_round();
        game
    }

    fn start_round(&mut self) {
        let player_count = self.config.player_count as usize;

        let mut deck = build_deck(&self.config.deck);
        deck.shuffle(&mut self.rng);

        let mut players = Vec::with_capacity(player_count);
        for _ in 0..player_count {
            let hand: Vec<Card> = deck.drain(..16).collect();
            let grid = PlayerGrid::new(hand, 2, &mut self.rng);
            players.push(PlayerState {
                grid,
                went_out_first: false,
                cleared_all: false,
                eliminations: 0,
            });
        }

        let mut discard_pile = Vec::new();
        if let Some(top) = deck.pop() {
            discard_pile.push(top);
        }

        self.players = players;
        self.draw_pile = deck;
        self.discard_pile = discard_pile;
        self.current_player = (self.round as usize) % self.config.player_count as usize;
        self.turn = 0;
        self.round_ended = false;
        self.trigger_player = None;
        self.final_turns_given = 0;

        if self.current_player == self.human_player {
            self.pending = InternalPending::ChooseDrawSource;
        } else {
            self.pending = InternalPending::NotYourTurn;
        }
    }

    // ── Human turn actions ──────────────────────────────────────────────

    pub fn human_draw(&mut self, source: &str) -> Result<(), String> {
        if !matches!(self.pending, InternalPending::ChooseDrawSource) {
            return Err("Not time to choose draw source".to_string());
        }
        if self.current_player != self.human_player {
            return Err("Not your turn".to_string());
        }

        let draw_source = match source {
            "draw" => DrawSource::DrawPile,
            "discard" => DrawSource::DiscardPile,
            _ => return Err("Invalid source, use 'draw' or 'discard'".to_string()),
        };

        let drawn = match self.draw_card(draw_source) {
            Some(card) => card,
            None => return Err("No cards available to draw".to_string()),
        };

        let source_name = if source == "draw" { "draw pile" } else { "discard pile" };
        self.action_log.push(format!("You drew {} from the {}.", drawn, source_name));
        self.pending = InternalPending::HandleNormalCard(drawn);

        Ok(())
    }

    pub fn human_action(&mut self, action_type: &str, params: &ActionParams) -> Result<(), String> {
        match &self.pending.clone() {
            InternalPending::HandleNormalCard(card) => {
                let card = card.clone();
                match action_type {
                    "replace" => {
                        let row = params.row.ok_or("Missing row")?;
                        let col = params.col.ok_or("Missing col")?;
                        let pos_str = format!("({},{})", row, col);
                        if let Some(old) = self.players[self.human_player].grid.replace_card(row, col, card.clone()) {
                            self.action_log.push(format!("You placed {} at {}, discarding {}.", card, pos_str, old));
                            self.discard_pile.push(old);
                        }
                    }
                    "flip" => {
                        let row = params.row.ok_or("Missing row")?;
                        let col = params.col.ok_or("Missing col")?;
                        self.discard_pile.push(card.clone());
                        self.players[self.human_player].grid.flip_card(row, col);

                        let flipped_display = self.players[self.human_player].grid
                            .get(row, col)
                            .map(|gc| format!("{}", gc.card))
                            .unwrap_or("?".to_string());

                        self.action_log.push(format!("You discarded {} and flipped ({},{}) revealing {}.", card, row, col, flipped_display));
                    }
                    _ => return Err("Invalid action type. Use 'replace' or 'flip'.".to_string()),
                }
                self.after_human_action();
            }
            InternalPending::ChooseSlideDirection(_) => {
                return Err("Use human_slide for slide direction choice.".to_string());
            }
            _ => return Err("No pending action for human.".to_string()),
        }
        Ok(())
    }

    pub fn human_slide(&mut self, direction: &str) -> Result<(), String> {
        let elim_kind = match &self.pending {
            InternalPending::ChooseSlideDirection(k) => k.clone(),
            _ => return Err("Not waiting for slide direction".to_string()),
        };

        let dir = match direction {
            "horizontal" => SlideDirection::Horizontal,
            "vertical" => SlideDirection::Vertical,
            _ => return Err("Invalid direction, use 'horizontal' or 'vertical'".to_string()),
        };

        self.players[self.human_player].grid.reshape_after_diagonal(&elim_kind, dir);
        self.players[self.human_player].grid.cleanup();
        self.action_log.push(format!("You chose {} slide after diagonal elimination.", direction));

        self.check_eliminations_human();
        Ok(())
    }

    // ── AI turn ─────────────────────────────────────────────────────────

    pub fn advance_ai(&mut self) -> Result<(), String> {
        if self.current_player == self.human_player {
            return Err("It's your turn, not AI's".to_string());
        }
        if matches!(self.pending, InternalPending::GameOver) {
            return Err("Game is over".to_string());
        }
        if matches!(self.pending, InternalPending::RoundOver) {
            return Err("Round is over, advance to next round".to_string());
        }

        let player_idx = self.current_player;
        let player_name = self.player_name(player_idx);

        if self.players[player_idx].grid.remaining_card_count() == 0 {
            self.action_log.push(format!("{} has no cards left, skip.", player_name));
            if self.round_ended {
                self.final_turns_given += 1;
            }
            self.advance_to_next_player();
            return Ok(());
        }

        if self.round_ended {
            self.final_turns_given += 1;
        }

        self.turn += 1;
        let player_config = &self.config.players[player_idx].clone();

        // 1. Choose draw source
        let discard_top = self.discard_pile.last().cloned();
        let source = strategy::choose_draw_source(
            player_config,
            discard_top.as_ref(),
            &self.players[player_idx].grid,
            self.config.deck.neg_min,
            self.config.deck.pos_max,
            &mut self.rng,
        );

        let source_name = match &source {
            DrawSource::DrawPile => "draw pile",
            DrawSource::DiscardPile => "discard pile",
        };

        // 2. Draw
        let drawn = match self.draw_card(source) {
            Some(card) => card,
            None => {
                self.action_log.push(format!("{} couldn't draw - no cards left.", player_name));
                self.advance_to_next_player();
                return Ok(());
            }
        };

        self.action_log.push(format!("{} drew {} from the {}.", player_name, drawn, source_name));

        // 3. Handle card (always normal — no special cards in game)
        let action = strategy::choose_action(
            player_config,
            &drawn,
            &self.players[player_idx].grid,
            self.config.deck.neg_min,
            self.config.deck.pos_max,
            &mut self.rng,
        );
        match action {
            TurnAction::ReplaceCard { row, col } => {
                if let Some(old) = self.players[player_idx].grid.replace_card(row, col, drawn.clone()) {
                    self.action_log.push(format!(
                        "{} placed {} at ({},{}), discarding {}.",
                        player_name, drawn, row, col, old
                    ));
                    self.discard_pile.push(old);
                }
            }
            TurnAction::DiscardAndFlip { row, col } => {
                self.discard_pile.push(drawn.clone());
                self.players[player_idx].grid.flip_card(row, col);

                let flipped_display = self.players[player_idx].grid
                    .get(row, col)
                    .map(|gc| format!("{}", gc.card))
                    .unwrap_or("?".to_string());

                self.action_log.push(format!(
                    "{} discarded {} and flipped ({},{}) revealing {}.",
                    player_name, drawn, row, col, flipped_display
                ));
            }
        }

        // 4. Check eliminations
        self.check_eliminations_ai(player_idx);

        // 5. Check round end
        self.check_round_end_trigger(player_idx);

        self.advance_to_next_player();
        Ok(())
    }

    pub fn advance_round(&mut self) -> Result<(), String> {
        if !matches!(self.pending, InternalPending::RoundOver) {
            return Err("Round is not over".to_string());
        }

        let scores = self.score_round();
        for (i, &s) in scores.iter().enumerate() {
            self.cumulative_scores[i] += s;
        }
        self.round_scores.push(scores.clone());

        let score_summary: Vec<String> = scores.iter().enumerate()
            .map(|(i, s)| format!("{}: {}", self.player_name(i), s))
            .collect();
        self.action_log.push(format!("Round {} scores: {}", self.round + 1, score_summary.join(", ")));

        self.round += 1;

        if self.round >= 4 {
            let winner = self.cumulative_scores
                .iter()
                .enumerate()
                .min_by_key(|(_, &s)| s)
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.action_log.push(format!(
                "Game Over! {} wins with {} points!",
                self.player_name(winner),
                self.cumulative_scores[winner]
            ));
            self.pending = InternalPending::GameOver;
        } else {
            self.action_log.push(format!("Round {} begins!", self.round + 1));
            self.start_round();
        }

        Ok(())
    }

    // ── Internal helpers ────────────────────────────────────────────────

    fn after_human_action(&mut self) {
        self.check_eliminations_human();
    }

    fn check_eliminations_human(&mut self) {
        loop {
            let eliminations = self.players[self.human_player].grid.find_eliminations(
                self.config.allow_matching_elimination,
                self.config.allow_diagonal_elimination,
                self.config.deck.neg_min,
                self.config.deck.pos_max,
            );

            if eliminations.is_empty() {
                break;
            }

            let elim = &eliminations[0];
            let is_diagonal = matches!(
                elim.kind,
                EliminationType::MainDiagonal | EliminationType::AntiDiagonal
            );

            let removed = self.players[self.human_player].grid.eliminate(&elim.positions);
            self.players[self.human_player].eliminations += 1;

            let reason = match &elim.reason {
                crate::engine::grid::EliminationReason::SumToZero => "sum-to-zero",
                crate::engine::grid::EliminationReason::AllMatching => "all-matching",
            };
            let kind_str = match &elim.kind {
                EliminationType::Row(r) => format!("Row {}", r),
                EliminationType::Column(c) => format!("Column {}", c),
                EliminationType::MainDiagonal => "Main diagonal".to_string(),
                EliminationType::AntiDiagonal => "Anti-diagonal".to_string(),
            };
            self.action_log.push(format!("Elimination! {} ({}).", kind_str, reason));

            if !removed.is_empty() {
                let discard_idx = self.best_discard_idx(&removed);
                self.discard_pile.push(removed[discard_idx].clone());
            }

            if is_diagonal {
                self.pending = InternalPending::ChooseSlideDirection(elim.kind.clone());
                return;
            }

            self.players[self.human_player].grid.cleanup();

            if self.players[self.human_player].grid.remaining_card_count() == 0 {
                self.players[self.human_player].cleared_all = true;
                self.action_log.push("You cleared all your cards!".to_string());
                break;
            }
        }

        self.check_round_end_trigger(self.human_player);
        self.turn += 1;
        self.advance_to_next_player();
    }

    fn check_eliminations_ai(&mut self, player_idx: usize) {
        let player_name = self.player_name(player_idx);
        loop {
            let eliminations = self.players[player_idx].grid.find_eliminations(
                self.config.allow_matching_elimination,
                self.config.allow_diagonal_elimination,
                self.config.deck.neg_min,
                self.config.deck.pos_max,
            );

            if eliminations.is_empty() {
                break;
            }

            let elim = &eliminations[0];
            let is_diagonal = matches!(
                elim.kind,
                EliminationType::MainDiagonal | EliminationType::AntiDiagonal
            );

            let removed = self.players[player_idx].grid.eliminate(&elim.positions);
            self.players[player_idx].eliminations += 1;

            let reason = match &elim.reason {
                crate::engine::grid::EliminationReason::SumToZero => "sum-to-zero",
                crate::engine::grid::EliminationReason::AllMatching => "all-matching",
            };
            let kind_str = match &elim.kind {
                EliminationType::Row(r) => format!("Row {}", r),
                EliminationType::Column(c) => format!("Column {}", c),
                EliminationType::MainDiagonal => "Main diagonal".to_string(),
                EliminationType::AntiDiagonal => "Anti-diagonal".to_string(),
            };
            self.action_log.push(format!("{} got an elimination! {} ({}).", player_name, kind_str, reason));

            if !removed.is_empty() {
                let player_config = self.config.players[player_idx].clone();
                let next_player = (player_idx + 1) % self.config.player_count as usize;
                let next_grid = Some(&self.players[next_player].grid);
                let discard_idx = strategy::choose_discard_with_opponent(
                    &player_config, &removed, next_grid,
                    self.config.deck.neg_min, self.config.deck.pos_max, &mut self.rng,
                );
                self.discard_pile.push(removed[discard_idx].clone());
            }

            if is_diagonal {
                let player_config = self.config.players[player_idx].clone();
                let direction = strategy::choose_slide_direction(
                    &player_config,
                    &self.players[player_idx].grid,
                    &elim.kind,
                    &mut self.rng,
                );
                let dir_name = match &direction {
                    SlideDirection::Horizontal => "horizontal",
                    SlideDirection::Vertical => "vertical",
                };
                self.action_log.push(format!("{} chose {} slide.", player_name, dir_name));
                self.players[player_idx].grid.reshape_after_diagonal(&elim.kind, direction);
            }

            self.players[player_idx].grid.cleanup();

            if self.players[player_idx].grid.remaining_card_count() == 0 {
                self.players[player_idx].cleared_all = true;
                self.action_log.push(format!("{} cleared all their cards!", player_name));
                break;
            }
        }
    }

    fn check_round_end_trigger(&mut self, player_idx: usize) {
        if self.round_ended {
            return;
        }

        let grid = &self.players[player_idx].grid;
        let remaining = grid.remaining_card_count();

        if (remaining <= 4 && grid.all_face_up()) || remaining == 0 {
            self.round_ended = true;
            self.trigger_player = Some(player_idx);
            self.players[player_idx].went_out_first = true;
            let name = self.player_name(player_idx);
            self.action_log.push(format!("{} triggered round end! ({} cards left). Each other player gets one more turn.", name, remaining));
        }
    }

    fn advance_to_next_player(&mut self) {
        let player_count = self.config.player_count as usize;

        if self.round_ended && self.final_turns_given >= player_count - 1 {
            self.pending = InternalPending::RoundOver;
            self.action_log.push("Round is over! Click 'Next Round' to continue.".to_string());
            return;
        }

        if self.turn > self.config.max_turns_per_round {
            self.pending = InternalPending::RoundOver;
            self.action_log.push("Max turns reached. Round over.".to_string());
            return;
        }

        self.current_player = (self.current_player + 1) % player_count;

        if self.current_player == self.human_player {
            if self.players[self.human_player].grid.remaining_card_count() == 0 {
                if self.round_ended {
                    self.final_turns_given += 1;
                }
                self.advance_to_next_player();
            } else {
                if self.round_ended {
                    self.final_turns_given += 1;
                }
                self.pending = InternalPending::ChooseDrawSource;
            }
        } else {
            self.pending = InternalPending::NotYourTurn;
        }
    }

    fn draw_card(&mut self, source: DrawSource) -> Option<Card> {
        match source {
            DrawSource::DiscardPile => {
                if let Some(card) = self.discard_pile.pop() {
                    Some(card)
                } else {
                    self.draw_from_pile()
                }
            }
            DrawSource::DrawPile => self.draw_from_pile(),
        }
    }

    fn draw_from_pile(&mut self) -> Option<Card> {
        if let Some(card) = self.draw_pile.pop() {
            return Some(card);
        }

        if self.discard_pile.len() <= 1 {
            return None;
        }
        let top = self.discard_pile.pop();
        self.draw_pile.extend(self.discard_pile.drain(..));
        self.draw_pile.shuffle(&mut self.rng);
        if let Some(t) = top {
            self.discard_pile.push(t);
        }
        self.draw_pile.pop()
    }

    fn best_discard_idx(&self, cards: &[Card]) -> usize {
        if cards.len() <= 1 {
            return 0;
        }
        let mut best_idx = 0;
        let mut best_score = i32::MIN;
        for (i, card) in cards.iter().enumerate() {
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

    fn score_round(&self) -> Vec<i32> {
        self.players
            .iter()
            .map(|p| {
                let mut score = match self.config.scoring_mode {
                    ScoringMode::Basic => p.grid.remaining_card_count() as i32,
                    ScoringMode::Expert => {
                        p.grid
                            .occupied_positions()
                            .iter()
                            .map(|&(r, c)| {
                                p.grid.get(r, c).map(|gc| gc.card.score_value()).unwrap_or(0)
                            })
                            .sum::<i32>()
                    }
                };
                if p.went_out_first { score -= 5; }
                score
            })
            .collect()
    }

    fn player_name(&self, idx: usize) -> String {
        if idx == self.human_player {
            "You".to_string()
        } else {
            match idx {
                1 => "West (AI)".to_string(),
                2 => "North (AI)".to_string(),
                3 => "East (AI)".to_string(),
                _ => format!("Player {}", idx + 1),
            }
        }
    }

    // ── Build view state ────────────────────────────────────────────────

    pub fn get_state(&self) -> PlayableGameState {
        let player_count = self.config.player_count as usize;

        let grids: Vec<GridView> = (0..player_count)
            .map(|i| self.build_grid_view(i))
            .collect();

        let discard_top = self.discard_pile.last().map(|c| card_to_view(c));

        let pending = match &self.pending {
            InternalPending::ChooseDrawSource => PendingAction {
                action_type: "choose_draw_source".to_string(),
                drawn_card: None,
            },
            InternalPending::HandleNormalCard(card) => PendingAction {
                action_type: "handle_normal_card".to_string(),
                drawn_card: Some(card_to_view(card)),
            },
            InternalPending::ChooseSlideDirection(_) => PendingAction {
                action_type: "choose_slide_direction".to_string(),
                drawn_card: None,
            },
            InternalPending::NotYourTurn => PendingAction {
                action_type: "not_your_turn".to_string(),
                drawn_card: None,
            },
            InternalPending::RoundOver => PendingAction {
                action_type: "round_over".to_string(),
                drawn_card: None,
            },
            InternalPending::GameOver => PendingAction {
                action_type: "game_over".to_string(),
                drawn_card: None,
            },
        };

        let names = (0..player_count)
            .map(|i| self.player_name(i))
            .collect();

        PlayableGameState {
            round: self.round,
            turn: self.turn,
            current_player: self.current_player,
            player_names: names,
            grids,
            draw_pile_count: self.draw_pile.len(),
            discard_top,
            cumulative_scores: self.cumulative_scores.clone(),
            round_scores: self.round_scores.clone(),
            pending,
            action_log: self.action_log.clone(),
            round_ended: self.round_ended,
            game_over: matches!(self.pending, InternalPending::GameOver),
            winner: if matches!(self.pending, InternalPending::GameOver) {
                Some(
                    self.cumulative_scores
                        .iter()
                        .enumerate()
                        .min_by_key(|(_, &s)| s)
                        .map(|(i, _)| i)
                        .unwrap_or(0),
                )
            } else {
                None
            },
            round_trigger_player: self.trigger_player,
        }
    }

    fn build_grid_view(&self, player_idx: usize) -> GridView {
        let grid = &self.players[player_idx].grid;

        let mut cells = Vec::new();
        for r in 0..grid.row_count() {
            let mut row = Vec::new();
            for c in 0..grid.col_count(r) {
                match grid.get(r, c) {
                    Some(gc) => {
                        if gc.face_up {
                            row.push(CellView {
                                state: "face_up".to_string(),
                                card: Some(format!("{}", gc.card)),
                                value: match &gc.card {
                                    Card::Number(v) => Some(*v),
                                    _ => None,
                                },
                                card_type: Some(match &gc.card {
                                    Card::Number(_) => "number".to_string(),
                                    Card::Wild => "wild".to_string(),
                                }),
                            });
                        } else {
                            row.push(CellView {
                                state: "face_down".to_string(),
                                card: None,
                                value: None,
                                card_type: None,
                            });
                        }
                    }
                    None => {
                        row.push(CellView {
                            state: "empty".to_string(),
                            card: None,
                            value: None,
                            card_type: None,
                        });
                    }
                }
            }
            cells.push(row);
        }

        GridView {
            cells,
            remaining: grid.remaining_card_count(),
            eliminations: self.players[player_idx].eliminations,
            all_face_up: grid.all_face_up(),
        }
    }
}

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
        },
    }
}

// ── Action params from frontend ─────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct ActionParams {
    pub row: Option<usize>,
    pub col: Option<usize>,
}
