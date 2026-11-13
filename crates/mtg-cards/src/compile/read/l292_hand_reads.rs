//! Shapes of a target player's hand and of counters: the card you choose from a revealed hand and exile ("Target
//! opponent reveals their hand. You choose a nonland card from it and exile that card"), a card revealed at random
//! whose mana value the player loses as life, each kind of counter on a target put again or removed, a +1/+1 counter
//! or one of each kind you have, and a counter for each coin flip won. Each is read as its steps and written back so.

use super::selectors::{Form, phrase_in, phrase_text, spec_text};
use super::tokens::Reader;
use crate::effect::{Effect, PerKind};
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

/// The permanents you control.
fn yours() -> Ref {
    Ref::Each(Selector(vec![Pred::ControlledBy(Ref::You)]))
}

/// One counter of each kind, chosen kind by kind.
fn per_kind(on: Ref, of: Ref, choice: PerKind) -> Vec<Effect> {
    vec![Effect::PutCountersOf {
        on,
        of,
        each: Some(Amount::Fixed(1)),
        one_kind: false,
        per_kind: Some(choice),
        put: false,
    }]
}

/// Reads "for each kind of counter on target permanent, put another counter of that kind on it or remove one from it"
/// and "for each kind of counter on permanents you control, you may put your choice of a +1/+1 counter or a counter of
/// that kind on ~".
pub(super) fn l292_add_or_remove(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("for each kind of counter on")?;
    if r.words("permanents you control") {
        r.punct(',').then_some(())?;
        r.need("you may put your choice of a")?;
        (super::counters::kind(r)? == crate::value::CounterKind::PlusOnePlusOne).then_some(())?;
        r.need("counter or a counter of that kind on")?;
        r.source().then_some(())?;
        return Some(per_kind(Ref::This, yours(), PerKind::MayPlusOneInstead));
    }
    let on = super::counters::place(r)?;
    matches!(on, Ref::Target(_)).then_some(())?;
    r.punct(',').then_some(())?;
    r.need("put another counter of that kind on it or remove one from it")?;
    Some(per_kind(on.clone(), on, PerKind::PutOrRemove))
}

/// The words of [`l292_add_or_remove`]; `target` is the name of the target the counters are on, if one is.
pub(super) fn l292_per_kind_text(
    on: &Ref,
    of: &Ref,
    choice: PerKind,
    target: Option<&str>,
) -> Option<String> {
    match choice {
        PerKind::PutOrRemove => {
            (on == of).then_some(())?;
            Some(format!(
                "for each kind of counter on {}, put another counter of that kind on it or remove one from it",
                target?
            ))
        }
        PerKind::MayPlusOneInstead => (*on == Ref::This && *of == yours()).then(|| {
            "for each kind of counter on permanents you control, you may put your choice of a +1/+1 counter or a counter \
             of that kind on ~"
                .to_owned()
        }),
    }
}

/// The name the flips won are kept under.
const WON: &str = "won";

/// Flips until a loss, then a counter of the kind on the card for each flip won.
fn flips_won(counter: crate::value::CounterKind) -> Vec<Effect> {
    vec![
        Effect::FlipCoin {
            who: Ref::You,
            win: Vec::new(),
            lose: Vec::new(),
            lose_first: false,
            until: true,
            won_as: Some(WON.to_owned()),
            heads: false,
        },
        Effect::PutCounters {
            on: Ref::This,
            counter,
            count: Amount::Bound(WON.to_owned()),
        },
    ]
}

/// Reads "flip a coin until you lose a flip. Put a +1/+1 counter on ~ for each flip you won".
pub(super) fn l292_flips_won(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("flip a coin until you lose a flip")?;
    r.punct('.').then_some(())?;
    r.need("put a")?;
    let counter = super::counters::kind(r)?;
    r.need("counter on")?;
    r.source().then_some(())?;
    r.need("for each flip you won")?;
    Some(flips_won(counter))
}

/// The words of [`l292_flips_won`] for the two steps at the front of `effects`.
fn flips_won_text(effects: &[Effect]) -> Option<String> {
    let [_, Effect::PutCounters { counter, .. }, ..] = effects else {
        return None;
    };
    (effects.get(..2)? == flips_won(counter.clone()).as_slice()).then_some(())?;
    Some(format!(
        "flip a coin until you lose a flip. Put a {} counter on ~ for each flip you won",
        super::counters::kind_text(counter)?
    ))
}

/// The sentence of either shape at the front of `effects`, and how many effects it takes.
pub(super) fn l292_hand_text(
    effects: &[Effect],
    targets: &[TargetSpec],
) -> Option<(String, usize)> {
    if let Some(text) = flips_won_text(effects) {
        return Some((text, 2));
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
