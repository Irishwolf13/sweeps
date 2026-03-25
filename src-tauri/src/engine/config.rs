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
        // 4-player preset: 132 cards (120 number + 12 wild)
        let card_quantities = vec![
            (-5, 5), (-4, 6), (-3, 8), (-2, 9), (-1, 11),
            (0, 13),
            (1, 11), (2, 11), (3, 10), (4, 9), (5, 8), (6, 7), (7, 6), (8, 6),
        ];
        DeckConfig {
            neg_min: -5,
            pos_max: 8,
            card_quantities,
            wild_count: 12,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum StartingOrder {
    RoundRobin,
    WorstScoreFirst,
}

impl Default for StartingOrder {
    fn default() -> Self {
        StartingOrder::RoundRobin
    }
}

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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum AiArchetype {
    Opportunist,
    Methodical,
    Calculator,
}

impl Default for AiArchetype {
    fn default() -> Self {
        AiArchetype::Opportunist
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerConfig {
    pub archetype: AiArchetype,
    /// 0.0 = random play, 1.0 = perfect execution of archetype strategy
    pub skill: f64,
    #[serde(default)]
    pub flip_strategy: FlipStrategy,
}

impl Default for PlayerConfig {
    fn default() -> Self {
        PlayerConfig {
            archetype: AiArchetype::Opportunist,
            skill: 0.85,
            flip_strategy: FlipStrategy::default(),
        }
    }
}

impl PlayerConfig {
    pub fn beginner() -> Self {
        PlayerConfig { archetype: AiArchetype::Methodical, skill: 0.6, flip_strategy: FlipStrategy::Random }
    }
    pub fn intermediate() -> Self {
        PlayerConfig { archetype: AiArchetype::Opportunist, skill: 0.7, flip_strategy: FlipStrategy::Random }
    }
    pub fn advanced() -> Self {
        PlayerConfig { archetype: AiArchetype::Opportunist, skill: 0.85, flip_strategy: FlipStrategy::Random }
    }
    pub fn expert() -> Self {
        PlayerConfig { archetype: AiArchetype::Calculator, skill: 1.0, flip_strategy: FlipStrategy::Random }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameConfig {
    pub deck: DeckConfig,
    pub player_count: u8,
    pub allow_matching_elimination: bool,
    pub allow_diagonal_elimination: bool,
    pub scoring_mode: ScoringMode,
    #[serde(default)]
    pub starting_order: StartingOrder,
    pub players: Vec<PlayerConfig>,
    pub max_turns_per_round: u32,
    /// Rounds per game = player_count * round_multiplier. Default 1.
    #[serde(default = "default_round_multiplier")]
    pub round_multiplier: u8,
}

fn default_round_multiplier() -> u8 { 1 }

impl Default for GameConfig {
    fn default() -> Self {
        let player_count = 4u8;
        let players = vec![
            PlayerConfig::beginner(),
            PlayerConfig::intermediate(),
            PlayerConfig::advanced(),
            PlayerConfig::expert(),
        ];
        GameConfig {
            deck: DeckConfig::default(),
            player_count,
            allow_matching_elimination: true,
            allow_diagonal_elimination: true,
            scoring_mode: ScoringMode::Basic,
            starting_order: StartingOrder::default(),
            players,
            max_turns_per_round: 500,
            round_multiplier: 1,
        }
    }
}

impl GameConfig {
    /// Total rounds in a game = player_count * round_multiplier
    pub fn total_rounds(&self) -> u8 {
        self.player_count.saturating_mul(self.round_multiplier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_deck_total() {
        let deck = DeckConfig::default();
        // 120 number cards + 12 wild = 132
        assert_eq!(deck.total_cards(), 132);
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
        assert_eq!(config.starting_order, StartingOrder::RoundRobin);
        assert_eq!(config.players[0].flip_strategy, FlipStrategy::Random);
    }

    #[test]
    fn test_ai_archetype_serialization() {
        let config = PlayerConfig {
            archetype: AiArchetype::Opportunist,
            skill: 0.7,
            flip_strategy: FlipStrategy::Random,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: PlayerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.archetype, AiArchetype::Opportunist);
        assert!((deserialized.skill - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_player_presets() {
        let b = PlayerConfig::beginner();
        assert_eq!(b.archetype, AiArchetype::Methodical);
        assert!((b.skill - 0.6).abs() < f64::EPSILON);

        let i = PlayerConfig::intermediate();
        assert_eq!(i.archetype, AiArchetype::Opportunist);
        assert!((i.skill - 0.7).abs() < f64::EPSILON);

        let a = PlayerConfig::advanced();
        assert_eq!(a.archetype, AiArchetype::Opportunist);
        assert!(a.skill > 0.8);

        let e = PlayerConfig::expert();
        assert_eq!(e.archetype, AiArchetype::Calculator);
        assert!((e.skill - 1.0).abs() < f64::EPSILON);
    }
}
