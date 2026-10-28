//! A target for each opponent: "For each opponent, destroy up to one target artifact or enchantment that player controls."
//! is one `Targetable::PerPlayer` specification over the opponents, at most one target of each, and the steps act on all
//! of them at once. The sentence is read as if it said "an opponent controls" (or "an opponent's graveyard"), and the
//! target it asks for becomes the specification for each opponent.

use super::conditional::body;
use super::tokens::{Reader, Token};
use crate::effect::Effect;
use crate::refs::{Bounds, Pred, Ref, Selector, Targetable};

/// "for each opponent, S" whose steps ask for up to one target "that player controls" or "in that player's graveyard"
/// and name that player nowhere else.
pub fn c10_dice_per_opponent(r: &mut Reader<'_>, over: &Ref) -> Option<Vec<Effect>> {
    (*over == Ref::Opponents).then_some(())?;
    let before = r.targets.len();
    r.attempt(|r| {
        let effects = r.c10_dice_rewritten(c10_dice_as_an_opponent, |inner| {
            inner.that_player = false;
            body(inner)
        })?;
        let [spec] = r.targets.get_mut(before..)? else {
            return None;
        };
        let one =
            spec.bounds == Bounds::up_to(Some(crate::value::Amount::one())) || spec.bounds.is_one();
        (one && spec.divide.is_none()).then_some(())?;
        let Targetable::Object(selector) = &spec.what else {
            return None;
        };
        spec.what = c10_dice_per_player(selector)?;
        Some(effects)
    })
}

/// The objects of the selector without the opponents it names, for each opponent.
fn c10_dice_per_player(selector: &Selector) -> Option<Targetable> {
    let opponents = |pred: &Pred| {
        matches!(
            pred,
            Pred::ControlledBy(Ref::Opponents) | Pred::OwnedBy(Ref::Opponents)
        )
    };
    let what = Selector(
        selector
            .0
            .iter()
            .filter(|pred| !opponents(pred))
            .cloned()
            .collect(),
    );
    (what.0.len() + 1 == selector.0.len()).then_some(())?;
    // The opponents must be named the way a per-player target names them back: who controls a permanent, whose a card is.
    let back = Targetable::c10_dice_per_player_objects(&Ref::Opponents, &what);
    back.0
        .iter()
        .all(|pred| selector.0.contains(pred))
        .then_some(())?;
    Some(Targetable::PerPlayer {
        players: Ref::Opponents,
        what,
    })
}

/// The tokens with each "that player controls" written "an opponent controls" and each "that player's" written "an
/// opponent's"; none when the sentence has no such phrase or names that player some other way.
fn c10_dice_as_an_opponent(tokens: &[Token]) -> Option<Vec<Token>> {
    let word = |at: usize, text: &str| matches!(tokens.get(at), Some(Token::Word(w)) if w == text);
    let mut edited = tokens.to_vec();
    let mut changed = false;
    for at in 0..tokens.len() {
        if !(word(at, "that") && word(at + 1, "player")) {
            continue;
        }
        let possessive =
            matches!(tokens.get(at + 2), Some(Token::Punct('\''))) && word(at + 3, "s");
        (word(at + 2, "controls") || possessive).then_some(())?;
        edited[at] = Token::Word("an".to_owned());
        edited[at + 1] = Token::Word("opponent".to_owned());
        changed = true;
    }
    changed.then_some(edited)
}
