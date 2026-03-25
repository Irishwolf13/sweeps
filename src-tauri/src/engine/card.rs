use serde::{Deserialize, Serialize};

use super::config::DeckConfig;

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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Card {
    Number(i32),
    Shape(Shape, Shade),
    Wild,
    WildShaded,
    WildUnshaded,
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
            Card::Shape(shape, Shade::Unshaded) => write!(f, "{:?}", shape),
            Card::Shape(shape, Shade::Shaded) => write!(f, "Shaded {:?}", shape),
            Card::Wild => write!(f, "Wild"),
            Card::WildShaded => write!(f, "Wild Shaded"),
            Card::WildUnshaded => write!(f, "Wild Unshaded"),
        }
    }
}

/// Build a full deck from the given configuration.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_default_deck_count() {
        let config = DeckConfig::default();
        let deck = build_deck(&config);
        // 120 number cards + 12 wild = 132 (4-player preset)
        assert_eq!(deck.len(), 132);
    }

    #[test]
    fn test_deck_contains_expected_cards() {
        let config = DeckConfig::default();
        let deck = build_deck(&config);

        let neg5_count = deck.iter().filter(|c| **c == Card::Number(-5)).count();
        assert_eq!(neg5_count, 5);

        let zero_count = deck.iter().filter(|c| **c == Card::Number(0)).count();
        assert_eq!(zero_count, 13);

        let wild_count = deck.iter().filter(|c| **c == Card::Wild).count();
        assert_eq!(wild_count, 12);

        let five_count = deck.iter().filter(|c| **c == Card::Number(5)).count();
        assert_eq!(five_count, 8);
    }

    #[test]
    fn test_score_values() {
        assert_eq!(Card::Number(-3).score_value(), 3);
        assert_eq!(Card::Number(5).score_value(), 5);
        assert_eq!(Card::Number(0).score_value(), 0);
        assert_eq!(Card::Wild.score_value(), 0);
    }

    #[test]
    fn test_build_shapes_deck_original() {
        let config = DeckConfig::shapes_original();
        let deck = build_deck(&config);
        assert_eq!(deck.len(), 230);
        assert_eq!(deck.iter().filter(|c| matches!(c, Card::Shape(_, _))).count(), 200);
        assert_eq!(deck.iter().filter(|c| matches!(c, Card::Wild)).count(), 10);
        assert_eq!(deck.iter().filter(|c| matches!(c, Card::WildShaded)).count(), 10);
        assert_eq!(deck.iter().filter(|c| matches!(c, Card::WildUnshaded)).count(), 10);
    }

    #[test]
    fn test_build_shapes_deck_scaled_4p() {
        let config = DeckConfig::shapes_scaled(4);
        let deck = build_deck(&config);
        assert_eq!(deck.len(), 130);
    }

    #[test]
    fn test_shapes_deck_validation() {
        let config = DeckConfig::shapes_scaled(4);
        assert!(config.validate(4).is_ok());
        // shapes_scaled(2)=76 cards can't handle 6 players (need 116)
        let config2 = DeckConfig::shapes_scaled(2);
        assert!(config2.validate(6).is_err());
    }
}
