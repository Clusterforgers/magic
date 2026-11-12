//! Rules of loyalty activations: "twice each turn rather than only once" (Oath of Teferi), "any time you could cast an
//! instant" (Teferi's emblem), "cost an additional [+1]" (Carth the Lion) and The Chain Veil's one more activation for
//! each planeswalker; the Flagbearer's rule of targets; and Drain Life's gain of the damage dealt up to what each recipient had. Read from the wording and
//! written back the same way.

use super::tokens::{Reader, Token, tokenize};
use crate::effect::Effect;
use crate::modify::{Duration, GameRule};
use crate::refs::{Pred, Ref, Selector};
use mtg_core::card::Subtype;

/// Drain Life's gain: "life equal to" this.
pub(super) const UP_TO_HEALTH: &str = "the damage dealt, but not more life than the player's life total before the damage was dealt, the planeswalker's loyalty before the damage was dealt, or the creature's toughness";
const FLAG: &str = "While an opponent is choosing targets as part of casting a spell they control or activating an ability they control, that player must choose at least one Flagbearer on the battlefield if able";
const TWICE: &str =
    "You may activate the loyalty abilities of planeswalkers you control twice each turn rather than only once";
const AS_INSTANT: &str = "You may activate loyalty abilities of planeswalkers you control on any player's turn any time you could cast an instant";
const ONCE_MORE: &str = "for each planeswalker you control, you may activate one of its loyalty abilities once this turn as though none of its loyalty abilities have been activated this turn";

/// The wordings of a loyalty rule a permanent or an emblem has.
pub(super) fn r373_rule_texts(rule: &GameRule) -> Vec<String> {
    match rule {
        GameRule::LoyaltyTwice(Ref::You) => vec![TWICE.to_owned()],
        GameRule::LoyaltyAsInstant(Ref::You) => vec![AS_INSTANT.to_owned()],
        GameRule::LoyaltyCostMore {
            who: Ref::You,
            plus,
        } if *plus > 0 => vec![format!(
            "Planeswalkers' loyalty abilities you activate cost an additional [+{plus}] to activate"
        )],
        GameRule::MustTarget {
            who: Ref::Opponents,
            what,
        } if *what == flagbearer() => vec![FLAG.to_owned()],
        _ => Vec::new(),
    }
}

/// "a Flagbearer": a permanent of that creature type.
fn flagbearer() -> Selector {
    Selector(vec![Pred::Subtype(Subtype("Flagbearer".to_owned()))])
}

/// A static loyalty rule, read from its wording.
pub(super) fn r373_rule(r: &mut Reader<'_>) -> Option<GameRule> {
    if r.attempt(|r| text(r, TWICE)).is_some() {
        return Some(GameRule::LoyaltyTwice(Ref::You));
    }
    if r.attempt(|r| text(r, AS_INSTANT)).is_some() {
        return Some(GameRule::LoyaltyAsInstant(Ref::You));
    }
    if r.attempt(|r| text(r, FLAG)).is_some() {
        return Some(GameRule::MustTarget {
            who: Ref::Opponents,
            what: flagbearer(),
        });
    }
    text(r, "Planeswalkers' loyalty abilities you activate cost an additional [+")?;
    let plus = r.number().filter(|plus| *plus > 0)?;
    text(r, "] to activate")?;
    Some(GameRule::LoyaltyCostMore { who: Ref::You, plus })
}

/// The Chain Veil's effect, written.
pub(super) fn r373_once_more_text(effect: &Effect) -> Option<String> {
    matches!(
        effect,
        Effect::Rule {
            rule: GameRule::LoyaltyOnceMore(Ref::You),
            duration: Duration::UntilEndOfTurn,
        }
    )
    .then(|| ONCE_MORE.to_owned())
}

/// The Chain Veil's effect, read.
pub(super) fn r373_once_more(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    text(r, ONCE_MORE)?;
    Some(vec![Effect::Rule {
        rule: GameRule::LoyaltyOnceMore(Ref::You),
        duration: Duration::UntilEndOfTurn,
    }])
}

/// Reads the tokens of `wording`, punctuation included, all or none.
pub(super) fn text(r: &mut Reader<'_>, wording: &str) -> Option<()> {
    r.attempt(|r| {
        tokenize(wording).iter().all(|token| match token {
            Token::Word(word) => r.word(word),
            Token::Punct(mark) => r.punct(*mark),
            Token::Number(number) => r.number() == Some(*number),
            Token::Name(name) => r.name() == Some(name.as_str()),
            Token::Symbol(_) | Token::Source => false,
        })
        .then_some(())
    })
}
