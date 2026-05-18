//! Hero draft logic — determines when drafts occur and what choices are offered.

use serde::{Deserialize, Serialize};

/// Rounds on which a hero draft occurs.
const DRAFT_ROUNDS: &[u32] = &[1, 3, 6, 9, 12];

/// Returns true if the given round is a hero draft round.
pub fn is_draft_round(round: u32) -> bool {
    DRAFT_ROUNDS.contains(&round)
}

/// The three hero choices offered during a draft (one per attribute).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftChoices {
    /// Strength hero option.
    pub strength: String,
    /// Agility hero option.
    pub agility: String,
    /// Intelligence hero option.
    pub intelligence: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_draft_round() {
        let expected: Vec<bool> = (1..=15)
            .map(|r| [1, 3, 6, 9, 12].contains(&r))
            .collect();
        let actual: Vec<bool> = (1..=15).map(is_draft_round).collect();
        assert_eq!(actual, expected);
    }
}
