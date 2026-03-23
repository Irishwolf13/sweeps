use serde::{Deserialize, Serialize};

use super::config::DeckConfig;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Card {
    Number(i32),
    Wild,
}

impl Card {
    /// Absolute value for Expert scoring. Wild scores as 0.
    pub fn score_value(&self) -> i32 {
        match self {
            Card::Number(v) => v.abs(),
            _ => 0,
        }
    }
}

impl std::fmt::Display for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Card::Number(v) => write!(f, "{}", v),
            Card::Wild => write!(f, "Wild"),
        }
    }
}

/// Build a full deck from the given configuration.
pub fn build_deck(config: &DeckConfig) -> Vec<Card> {
    let mut deck = Vec::with_capacity(config.total_cards() as usize);

    for &(value, count) in &config.card_quantities {
        for _ in 0..count {
            deck.push(Card::Number(value));
        }
    }
    for _ in 0..config.wild_count {
        deck.push(Card::Wild);
    }

    deck
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_default_deck_count() {
        let config = DeckConfig::default();
        let deck = build_deck(&config);
        // 101 number cards + 8 wild = 109
        assert_eq!(deck.len(), 109);
    }

    #[test]
    fn test_deck_contains_expected_cards() {
        let config = DeckConfig::default();
        let deck = build_deck(&config);

        let neg5_count = deck.iter().filter(|c| **c == Card::Number(-5)).count();
        assert_eq!(neg5_count, 4);

        let zero_count = deck.iter().filter(|c| **c == Card::Number(0)).count();
        assert_eq!(zero_count, 10);

        let wild_count = deck.iter().filter(|c| **c == Card::Wild).count();
        assert_eq!(wild_count, 8);

        let five_count = deck.iter().filter(|c| **c == Card::Number(5)).count();
        assert_eq!(five_count, 7);
    }

    #[test]
    fn test_score_values() {
        assert_eq!(Card::Number(-3).score_value(), 3);
        assert_eq!(Card::Number(5).score_value(), 5);
        assert_eq!(Card::Number(0).score_value(), 0);
        assert_eq!(Card::Wild.score_value(), 0);
    }
}
