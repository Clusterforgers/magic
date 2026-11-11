//! Conditions on how many turns a player has taken (`Amount::TurnsTaken`): "it's your first, second, or third turn of
//! the game" is your turn while you have begun three turns or fewer.

use crate::refs::Ref;
use crate::value::{Amount, Cmp, Condition};

/// The ordinals of the first turns, as the card lists them.
const ORDINALS: [&str; 5] = ["first", "second", "third", "fourth", "fifth"];

/// "your first, second, or third turn" (`plural`: "turns") for the condition of your early turns; `None` for any other.
pub fn early_turns_text(condition: &Condition, plural: bool) -> Option<String> {
    let Condition::All(parts) = condition else {
        return None;
    };
    let [
        Condition::YourTurn,
        Condition::Compare {
            left: Amount::TurnsTaken(Ref::You),
            op: Cmp::Le,
            right: Amount::Fixed(most),
        },
    ] = parts.as_slice()
    else {
        return None;
    };
    let names = ORDINALS.get(..usize::try_from(*most).ok().filter(|n| *n >= 2)?)?;
    let (last, rest) = names.split_last()?;
    let joined = if rest.len() == 1 {
        format!("{} or {last}", rest.join(", "))
    } else {
        format!("{}, or {last}", rest.join(", "))
    };
    let noun = if plural { "turns" } else { "turn" };
    Some(format!("your {joined} {noun} of the game"))
}

/// The condition after "if" or "unless": "it's your first, second, or third turn of the game".
pub fn condition_text(condition: &Condition) -> Option<String> {
    early_turns_text(condition, false).map(|turns| format!("it's {turns}"))
}

/// Whether the reference names every player other than the one whose ability asks: "another player".
pub fn is_another_player(reference: &Ref) -> bool {
    let Ref::PlayersWhere { who, test } = reference else {
        return false;
    };
    **who == Ref::EachPlayer && **test == Condition::Not(Box::new(Condition::Among(Ref::Asker)))
}

/// "that player's life total as the turn began": the life total the player the trigger is about had then.
pub fn amount_text(amount: &Amount) -> Option<String> {
    match amount {
        Amount::LifeAsTurnBegan(Ref::TriggeringPlayer) => {
            Some("that player's life total as the turn began".to_owned())
        }
        Amount::LifeAsTurnBegan(Ref::You) => Some("your life total as the turn began".to_owned()),
        _ => None,
    }
}
