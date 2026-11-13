//! Suffixes of the shared noun-phrase reader and writer for a block in either direction this turn ("that blocked or was
//! blocked this turn", "that blocked or was blocked by a Zombie this turn"), and the trigger head "Whenever players finish
//! voting" and the free cast of the first spell of the game.

use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use crate::did::Action;
use crate::refs::{Pred, Ref, Selector};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Cmp, Condition, TurnTally};
use mtg_core::card::CardType;

/// A suffix of this module after the noun of a single object.
pub(super) fn r379_suffix(r: &mut Reader<'_>, form: Form) -> Option<Pred> {
    if !form.plural() && r.words("that entered since your last turn ended") {
        return Some(Pred::EnteredSinceLastTurn(Ref::You));
    }
    if form.plural() || !r.words("that blocked or was blocked") {
        return None;
    }
    if r.words("this turn") {
        return Some(Pred::BlockedThisTurn(Selector(Vec::new())));
    }
    if !(r.word("by") && (r.word("a") || r.word("an"))) {
        return None;
    }
    let with = phrase(r, Form::Subject)?;
    r.words("this turn").then_some(Pred::BlockedThisTurn(with))
}

/// The suffix of this module for a predicate.
pub(super) fn r379_suffix_text(pred: &Pred, form: Form) -> Option<String> {
    if *pred == Pred::EnteredSinceLastTurn(Ref::You) && !form.plural() {
        return Some("that entered since your last turn ended".to_owned());
    }
    let Pred::BlockedThisTurn(with) = pred else {
        return None;
    };
    if form.plural() {
        return None;
    }
    if with.is_empty() {
        return Some("that blocked or was blocked this turn".to_owned());
    }
    let words = phrase_text(with, Form::Subject)?;
    Some(format!(
        "that blocked or was blocked by {} {words} this turn",
        super::triggers::article(&words)
    ))
}

/// The event after "Whenever": "players finish voting".
pub(super) fn r379_voting(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    r.words("players finish voting")
        .then(|| TriggerPattern::Did {
            action: Action::FinishedVoting,
            who: Ref::EachPlayer,
            what: Selector(Vec::new()),
        })
}

/// "Whenever players finish voting" for the pattern that reads it.
pub(super) fn r379_voting_text(pattern: &TriggerPattern) -> Option<String> {
    matches!(
        pattern,
        TriggerPattern::Did {
            action: Action::FinishedVoting,
            who: Ref::EachPlayer,
            what,
        } if what.is_empty()
    )
    .then(|| "Whenever players finish voting".to_owned())
}

/// "if this spell is the first spell you've cast this game, you may cast it without paying its mana cost": the free cast
/// open while the caster has cast no spell this game.
pub(super) fn r379_first_spell_text(condition: &Condition) -> Option<String> {
    let first = Condition::Compare {
        left: Amount::Tallied {
            who: Ref::You,
            what: TurnTally::SpellsCastThisGame,
        },
        op: Cmp::Eq,
        right: Amount::Fixed(0),
    };
    (*condition == first).then(|| {
        "if this spell is the first spell you've cast this game, you may cast it without paying its mana cost"
            .to_owned()
    })
}

/// "the greatest mana value among instant and sorcery spells you've cast this turn": the greatest value among the spells
/// of the caster's own turn log.
pub(super) fn r379_greatest_cast_text(who: &Ref, of: &Selector, stat: &str) -> Option<String> {
    let spells = match of.0.as_slice() {
        [Pred::AnyOf(kinds)]
            if *kinds == [Pred::Type(CardType::Instant), Pred::Type(CardType::Sorcery)] =>
        {
            "instant and sorcery spells"
        }
        [] => "spells",
        _ => return None,
    };
    (*who == Ref::You).then(|| format!("the greatest {stat} among {spells} you've cast this turn"))
}
