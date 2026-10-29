//! Rules about targets, losing, casting, combat damage and counters: "Cards in graveyards can't be the targets of
//! spells or abilities.", "You can't lose the game and your opponents can't win the game.", "Your opponents can't cast
//! spells during your turn.", "Each creature you control assigns combat damage equal to its toughness rather than its
//! power." and "Counters remain on ~ as it moves to any zone other than a player's hand or library."

use super::tokens::Reader;
use crate::modify::GameRule;
use crate::refs::Ref;

/// The sentence of `GameRule::GraveyardCardsUntargetable`.
const W17B_GRAVEYARDS: &str = "Cards in graveyards can't be the targets of spells or abilities";
/// The sentence of `GameRule::CantLoseGame` with its opponents.
const W17B_CANT_LOSE: &str = "You can't lose the game and your opponents can't win the game";
/// The sentence of `GameRule::CantLoseGame` alone.
const W17B_CANT_LOSE_ALONE: &str = "You can't lose the game";
/// The sentence of `GameRule::CantCastSpells` during your turn.
const W17B_CANT_CAST: &str = "Your opponents can't cast spells during your turn";
/// The end of the sentence of `GameRule::ToughnessAssignsDamage`.
const W17B_TOUGHNESS: &str = "assigns combat damage equal to its toughness rather than its power";

/// One of the rules this module reads, the whole sentence but its period.
pub(super) fn w17b_read(r: &mut Reader<'_>) -> Option<GameRule> {
    if r.words("cards in graveyards can't be the targets of spells or abilities") {
        return Some(GameRule::GraveyardCardsUntargetable);
    }
    if r.words("you can't lose the game") {
        let opponents_cant_win = r.words("and your opponents can't win the game");
        return Some(GameRule::CantLoseGame {
            who: Ref::You,
            opponents_cant_win,
        });
    }
    if r.words("your opponents can't cast spells during your turn") {
        return Some(GameRule::CantCastSpells {
            who: Ref::Opponents,
            your_turn: true,
        });
    }
    if r.words("counters remain on") {
        r.source().then_some(())?;
        r.need("as it moves to any zone other than a player's hand or library")?;
        return Some(GameRule::CountersRemain);
    }
    r.need("each creature")?;
    let who = if r.words("you control") {
        Ref::You
    } else {
        Ref::EachPlayer
    };
    r.need(&W17B_TOUGHNESS.to_lowercase())?;
    Some(GameRule::ToughnessAssignsDamage(who))
}

/// What `w17b_read` reads, written back.
pub(super) fn w17b_texts(rule: &GameRule) -> Vec<String> {
    let text = match rule {
        GameRule::GraveyardCardsUntargetable => W17B_GRAVEYARDS.to_owned(),
        GameRule::CantLoseGame {
            who: Ref::You,
            opponents_cant_win,
        } => if *opponents_cant_win {
            W17B_CANT_LOSE
        } else {
            W17B_CANT_LOSE_ALONE
        }
        .to_owned(),
        GameRule::CantCastSpells {
            who: Ref::Opponents,
            your_turn: true,
        } => W17B_CANT_CAST.to_owned(),
        GameRule::CountersRemain => {
            "Counters remain on ~ as it moves to any zone other than a player's hand or library".to_owned()
        }
        GameRule::ToughnessAssignsDamage(Ref::You) => {
            format!("Each creature you control {W17B_TOUGHNESS}")
        }
        GameRule::ToughnessAssignsDamage(Ref::EachPlayer) => format!("Each creature {W17B_TOUGHNESS}"),
        _ => return Vec::new(),
    };
    vec![text]
}
