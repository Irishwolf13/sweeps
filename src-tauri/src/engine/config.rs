use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeckConfig {
    pub neg_min: i32,
    pub pos_max: i32,
    pub card_quantities: Vec<(i32, u32)>,
    pub wild_count: u32,
}

impl DeckConfig {
    pub fn total_cards(&self) -> u32 {
        let number_cards: u32 = self.card_quantities.iter().map(|(_, count)| count).sum();
        number_cards + self.wild_count
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
        if self.neg_min > 0 {
            return Err("Negative range minimum must be <= 0".to_string());
        }
        if self.pos_max < 0 {
            return Err("Positive range maximum must be >= 0".to_string());
        }
        Ok(())
    }
}

impl Default for DeckConfig {
    fn default() -> Self {
        // Flattened curve: -5 to 8, gentle slope, negatives slightly rarer
        let card_quantities = vec![
            (-5, 4), (-4, 6), (-3, 7), (-2, 8), (-1, 9),
            (0, 10),
            (1, 9), (2, 9), (3, 9), (4, 8), (5, 7), (6, 6), (7, 5), (8, 4),
        ];
        DeckConfig {
            neg_min: -5,
            pos_max: 8,
            card_quantities,
            wild_count: 8,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ScoringMode {
    Basic,
    Expert,
}

impl Default for ScoringMode {
    fn default() -> Self {
        ScoringMode::Basic
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerConfig {
    /// Absolute value threshold for keeping a drawn card [0–10].
    /// If the drawn card's |value| <= this threshold, AI keeps it and replaces
    /// a face-down card. Above this, AI leans toward discarding & flipping.
    /// Example: 3 means keep anything |val| <= 3, discard 4+ unless it helps a line.
    pub keep_threshold: i32,

    /// How well the player spots and plays toward line completions [0.0–1.0].
    /// This is the PRIMARY skill knob.
    /// Low = ignores line potential, just uses keep_threshold;
    /// High = always checks if a card completes/nearly completes a line first,
    /// sees cascade potential, picks smart flip targets.
    pub line_awareness: f64,

    /// How much the player considers what the next player needs [0.0–1.0].
    /// Low = ignores opponents; High = avoids discarding cards that help
    /// the next player complete a line.
    pub opponent_awareness: f64,
}

impl Default for PlayerConfig {
    fn default() -> Self {
        // Advanced preset
        PlayerConfig {
            keep_threshold: 4,
            line_awareness: 0.7,
            opponent_awareness: 0.5,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameConfig {
    pub deck: DeckConfig,
    pub player_count: u8,
    pub allow_matching_elimination: bool,
    pub allow_diagonal_elimination: bool,
    pub scoring_mode: ScoringMode,
    pub players: Vec<PlayerConfig>,
    pub max_turns_per_round: u32,
}

impl Default for GameConfig {
    fn default() -> Self {
        let player_count = 4u8;
        let players = (0..player_count).map(|_| PlayerConfig::default()).collect();
        GameConfig {
            deck: DeckConfig::default(),
            player_count,
            allow_matching_elimination: true,
            allow_diagonal_elimination: true,
            scoring_mode: ScoringMode::Basic,
            players,
            max_turns_per_round: 500,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_deck_total() {
        let deck = DeckConfig::default();
        // 101 number cards + 8 wild = 109
        assert_eq!(deck.total_cards(), 109);
    }

    #[test]
    fn test_deck_validation_ok() {
        let deck = DeckConfig::default();
        assert!(deck.validate(4).is_ok());
    }

    #[test]
    fn test_deck_validation_too_few() {
        let mut deck = DeckConfig::default();
        deck.card_quantities = vec![(0, 1)];
        deck.wild_count = 0;
        assert!(deck.validate(4).is_err());
    }

    #[test]
    fn test_default_game_config() {
        let config = GameConfig::default();
        assert_eq!(config.player_count, 4);
        assert_eq!(config.players.len(), 4);
        assert!(config.allow_matching_elimination);
        assert!(config.allow_diagonal_elimination);
    }
}
