//! r55-14: the one player a rule that lasts "until that player's next turn" (`Duration::UntilTheirNextTurn`) is about.

use crate::modify::GameRule;
use crate::refs::Ref;

/// The player reference of a rule that can last until that player's next turn: "they gain protection from everything",
/// "their life total can't change"; none for any other rule.
pub fn r5514_rule_player(rule: &GameRule) -> Option<&Ref> {
    match rule {
        GameRule::LifeTotalCantChange(who) | GameRule::PlayersHave { who, .. } => Some(who),
        _ => None,
    }
}
