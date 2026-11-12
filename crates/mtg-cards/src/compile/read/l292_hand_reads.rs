//! Two shapes of a target player's hand: the card you choose from the revealed hand and exile ("Target opponent reveals
//! their hand. You choose a nonland card from it and exile that card"), and a card revealed at random whose mana value the
//! player loses as life ("Target player reveals a card at random from their hand, then loses life equal to that card's
//! mana value"). Each is read as its steps and written back the same way.

use super::selectors::{Form, phrase_in, phrase_text, spec_text};
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector, TargetSpec, ZoneKind};
use crate::value::{Amount, StatKind};

/// The name the chosen or revealed card is bound to.
const PICKED: &str = "picked";

/// The cards in the hand of `who` that also match `kind`.
fn hand_of(who: &Ref, kind: Vec<Pred>) -> Ref {
    let mut preds = kind;
    preds.push(Pred::In(ZoneKind::Hand));
    preds.push(Pred::OwnedBy(who.clone()));
    Ref::Each(Selector(preds))
}

/// The steps of the chosen exile.
fn chosen_exile(who: &Ref, kind: Vec<Pred>) -> Vec<Effect> {
    vec![
        Effect::Reveal {
            what: hand_of(who, Vec::new()),
        },
        Effect::Choose {
            who: Ref::You,
            from: hand_of(who, kind),
            count: crate::refs::Bounds::default(),
            bind: PICKED.to_owned(),
            rest_as: None,
        },
        Effect::Exile {
            what: Ref::Bound(PICKED.to_owned()),
            bind: None,
            duration: None,
        },
    ]
}

/// The steps of the card revealed at random and the life lost.
fn random_reveal(who: Ref) -> Vec<Effect> {
    vec![
        Effect::ChooseAtRandom {
            from: hand_of(&who, Vec::new()),
            count: Amount::one(),
            bind: PICKED.to_owned(),
        },
        Effect::Reveal {
            what: Ref::Bound(PICKED.to_owned()),
        },
        Effect::LoseLife {
            who,
            amount: Amount::Stat {
                of: Ref::Bound(PICKED.to_owned()),
                stat: StatKind::ManaValue,
            },
        },
    ]
}

/// Reads either shape; the last period is left to the caller.
pub(super) fn l292_hand_reads(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let who = super::effects::player(r)?;
    matches!(who, Ref::Target(_)).then_some(())?;
    if r.words("reveals a card at random from their hand") {
        r.punct(',');
        r.need("then loses life equal to that card's mana value")?;
        return Some(random_reveal(who));
    }
    r.need("reveals their hand")?;
    r.punct('.').then_some(())?;
    r.need("you choose")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let Selector(preds) = phrase_in(r, Form::Count, true)?;
    r.need("from it and exile that card")?;
    Some(chosen_exile(&who, preds))
}

/// The cards the source exiled ("the exiled cards"), in exile.
fn exiled_cards() -> Ref {
    Ref::Each(Selector(vec![
        Pred::In(ZoneKind::Exile),
        Pred::ExiledWith(Ref::This),
    ]))
}

/// "exile all cards from target opponent's hand" and "return the exiled cards to their owner's hand": a target player's
/// whole hand exiled, and what the source exiled returned.
fn whole_hand_text(effect: &Effect, targets: &[TargetSpec]) -> Option<String> {
    match effect {
        Effect::Exile {
            what: Ref::Each(Selector(preds)),
            bind: None,
            duration: None,
        } => {
            let [Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::Target(index))] = preds.as_slice()
            else {
                return None;
            };
            let player = spec_text(targets.get(usize::from(*index))?)?;
            Some(format!("exile all cards from {player}'s hand"))
        }
        Effect::Move {
            what,
            to: crate::effect::Destination::Hand,
            bind: None,
        } if *what == exiled_cards() => {
            Some("return the exiled cards to their owner's hand".to_owned())
        }
        _ => None,
    }
}

/// Reads "exile all cards from target opponent's hand" and "return the exiled cards to their owner's hand".
pub(super) fn l292_whole_hand(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    if r.words("return the exiled cards to their owner's hand") {
        return Some(vec![Effect::Move {
            what: exiled_cards(),
            to: crate::effect::Destination::Hand,
            bind: None,
        }]);
    }
    r.need("exile all cards from target")?;
    let what = if r.word("player's") {
        crate::refs::Targetable::Player
    } else {
        r.need("opponent's")?;
        crate::refs::Targetable::Opponent
    };
    r.need("hand")?;
    let who = Ref::Target(r.targets.len().try_into().ok()?);
    r.targets.push(TargetSpec {
        what,
        bounds: crate::refs::Bounds::default(),
        divide: None,
    });
    Some(vec![Effect::Exile {
        what: hand_of(&who, Vec::new()),
        bind: None,
        duration: None,
    }])
}

/// The sentence of either shape at the front of `effects`, and how many effects it takes.
pub(super) fn l292_hand_text(
    effects: &[Effect],
    targets: &[TargetSpec],
) -> Option<(String, usize)> {
    if let Some(text) = whole_hand_text(effects.first()?, targets) {
        return Some((text, 1));
    }
    let who = match effects.first()? {
        Effect::Reveal {
            what: Ref::Each(Selector(preds)),
        }
        | Effect::ChooseAtRandom {
            from: Ref::Each(Selector(preds)),
            ..
        } => match preds.as_slice() {
            [
                Pred::In(ZoneKind::Hand),
                Pred::OwnedBy(who @ Ref::Target(_)),
            ] => who.clone(),
            _ => return None,
        },
        _ => return None,
    };
    let Ref::Target(index) = &who else {
        return None;
    };
    let player = spec_text(targets.get(usize::from(*index))?)?;
    if effects.get(..3)? == random_reveal(who.clone()).as_slice() {
        return Some((
            format!(
                "{player} reveals a card at random from their hand, then loses life equal to that card's mana value"
            ),
            3,
        ));
    }
    let Effect::Choose {
        from: Ref::Each(Selector(chosen)),
        ..
    } = effects.get(1)?
    else {
        return None;
    };
    let kind = chosen.get(..chosen.len().checked_sub(2)?)?.to_vec();
    (effects.get(..3)? == chosen_exile(&who, kind.clone()).as_slice()).then_some(())?;
    // The cards of a hand are cards: "nonland card", not "nonland permanent".
    let noun = phrase_text(&Selector(kind), Form::Count)?;
    let noun = noun.strip_suffix(" permanent").unwrap_or(&noun);
    let noun = format!("{} card", noun.strip_suffix(" card").unwrap_or(noun));
    let article = super::triggers::article(&noun);
    Some((
        format!(
            "{player} reveals their hand. You choose {article} {noun} from it and exile that card"
        ),
        3,
    ))
}
