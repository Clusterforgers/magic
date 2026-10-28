//! Sentences of moves, choices and players the renderers have no wording for, derived from the data of the effects:
//! "Put one, two, or three target creature cards from graveyards onto the battlefield under your control. Each of them
//! enters with an additional -1/-1 counter on it.", "that player chooses up to two creatures they control, then
//! sacrifices the rest", "defending player loses 1 life for each card in their graveyard", "target permanent phases
//! out", "return target spell to its owner's hand", "create a 0/0 green and blue Fractal creature token. Put X
//! +1/+1 counters on it" and a random discard before the shuffle of a search.

use super::super::super::amounts::count_word;
use super::super::super::selectors::{Form, phrase_text, spec_text};
use super::super::variants;
use super::w4e_counters::{lower_first, w4e_counter_phrase, w4e_one_to};
use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use crate::value::Amount;

/// The sentences of the steps, without their period, each opening in lower case.
pub(super) fn sentences(targets: &[TargetSpec], effects: &[Effect]) -> Vec<String> {
    let mut found = Vec::new();
    found.extend(returned_many(targets, effects));
    found.extend(rest_sacrificed(targets, effects));
    found.extend(per_card_lost(targets, effects));
    found.extend(phased(targets, effects));
    found.extend(token_with_counters(targets, effects));
    found.extend(spell_returned(targets, effects));
    found
}

/// "put one, two, or three target creature cards from graveyards onto the battlefield under your control. Each of them
/// enters with an additional -1/-1 counter on it".
fn returned_many(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    let [
        Effect::Move {
            what: Ref::Target(0),
            to:
                Destination::Battlefield {
                    tapped: false,
                    under: Some(Ref::You),
                    transformed: false,
                    counters,
                },
        },
    ] = effects
    else {
        return None;
    };
    let [spec] = targets else { return None };
    let Targetable::Object(selector) = &spec.what else {
        return None;
    };
    let many = w4e_one_to(&spec.bounds)?;
    let cards =
        phrase_text(selector, Form::Targets)?.replace("from a graveyard", "from graveyards");
    let placed = format!("put {many} target {cards} onto the battlefield under your control");
    if let [(counter, number)] = counters.as_slice() {
        let added = w4e_counter_phrase(&Amount::Fixed(i32::try_from(*number).ok()?), counter)?;
        let added = if let Some(rest) = added.strip_prefix("a ") {
            format!("an additional {rest}")
        } else {
            added.clone()
        };
        return Some(format!("{placed}. Each of them enters with {added} on it"));
    }
    counters.is_empty().then_some(placed)
}

/// "that player chooses up to two creatures they control, then sacrifices the rest".
fn rest_sacrificed(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    let [
        Effect::Choose {
            who: Ref::Triggering,
            from: Ref::Each(Selector(chosen)),
            count:
                Bounds {
                    min: Amount::Fixed(0),
                    max: Some(Amount::Fixed(most)),
                },
            rest_as: Some(rest),
            ..
        },
        Effect::Sacrifice {
            who: Ref::Triggering,
            filter: Selector(filter),
            count: Amount::Count(Selector(counted)),
        },
    ] = effects
    else {
        return None;
    };
    let the_rest = [Pred::Is(Ref::Bound(rest.clone()))];
    if !targets.is_empty() || filter.as_slice() != the_rest || counted.as_slice() != the_rest {
        return None;
    }
    let owned = Pred::ControlledBy(Ref::Triggering);
    if !chosen.contains(&owned) {
        return None;
    }
    let group: Vec<Pred> = chosen
        .iter()
        .filter(|pred| **pred != owned)
        .cloned()
        .collect();
    let group = phrase_text(&Selector(group), Form::All)?;
    let most = count_word(u32::try_from(*most).ok()?)?;
    Some(format!(
        "that player chooses up to {most} {group} they control, then sacrifices the rest"
    ))
}

/// "defending player loses 1 life for each card in their graveyard".
fn per_card_lost(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    let [
        Effect::LoseLife {
            who,
            amount: Amount::Count(Selector(preds)),
        },
    ] = effects
    else {
        return None;
    };
    let player = match who {
        Ref::Defending => "defending player",
        Ref::Triggering => "that player",
        _ => return None,
    };
    let [
        Pred::In(zone @ (ZoneKind::Graveyard | ZoneKind::Hand)),
        Pred::OwnedBy(owner),
    ] = preds.as_slice()
    else {
        return None;
    };
    if owner != who || !targets.is_empty() {
        return None;
    }
    let zone = if *zone == ZoneKind::Hand {
        "hand"
    } else {
        "graveyard"
    };
    Some(format!(
        "{player} loses 1 life for each card in their {zone}"
    ))
}

/// "return target spell to its owner's hand".
fn spell_returned(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    let [
        Effect::Move {
            what: Ref::Target(0),
            to: Destination::Hand,
        },
    ] = effects
    else {
        return None;
    };
    let [spec] = targets else { return None };
    let Targetable::Object(Selector(preds)) = &spec.what else {
        return None;
    };
    (spec.bounds.is_one() && preds.contains(&Pred::Spell))
        .then(|| spec_text(spec))
        .flatten()
        .map(|target| format!("return {target} to its owner's hand"))
}

/// "target permanent phases out".
fn phased(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    let [
        Effect::PhaseOut {
            what: Ref::Target(0),
            duration: None,
            tapped: false,
        },
    ] = effects
    else {
        return None;
    };
    let [spec] = targets else { return None };
    spec.bounds
        .is_one()
        .then(|| spec_text(spec))
        .flatten()
        .map(|target| format!("{target} phases out"))
}

/// "create a 0/0 green and blue Fractal creature token. Put X +1/+1 counters on it", after "double the number of
/// growth counters on ~. Then " for one step before it.
fn token_with_counters(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    let (
        before,
        [
            create @ Effect::CreateToken {
                bind: Some(made), ..
            },
            Effect::PutCounters {
                on: Ref::Bound(on),
                counter,
                count,
            },
        ],
    ) = effects.split_at(effects.len().checked_sub(2)?)
    else {
        return None;
    };
    if on != made || !targets.is_empty() || before.len() > 1 {
        return None;
    }
    let mut plain = create.clone();
    if let Effect::CreateToken { bind, .. } = &mut plain {
        *bind = None;
    }
    let created = only_sentence(vec![plain])?;
    let put = w4e_counter_phrase(count, counter)?;
    let sentence = format!("{created}. Put {put} on it");
    if before.is_empty() {
        return Some(sentence);
    }
    let first = only_sentence(before.to_vec())?;
    Some(format!("{first}. Then {sentence}"))
}

/// The one-sentence wording of the steps as a spell, in lower case and without its period.
fn only_sentence(effects: Vec<Effect>) -> Option<String> {
    let spell = Ability::Spell {
        targets: Vec::new(),
        effects,
    };
    variants(&spell).into_iter().find_map(|text| {
        let sentence = text.strip_suffix('.')?;
        (!sentence.contains(". ")).then(|| lower_first(sentence))
    })
}

/// The wordings of a search whose card is discarded at random before the shuffle: "Search your library for a card,
/// put that card into your hand, discard a card at random, then shuffle."
pub(super) fn search_discard(ability: &Ability) -> Vec<String> {
    let mut plain = ability.clone();
    let (Ability::Spell { effects, .. }
    | Ability::Activated { effects, .. }
    | Ability::Triggered { effects, .. }) = &mut plain
    else {
        return Vec::new();
    };
    let [
        Effect::Search { .. },
        Effect::Move { .. },
        discard @ Effect::Discard { .. },
        Effect::Shuffle { .. },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    let Some(discarded) = only_sentence(vec![discard.clone()]) else {
        return Vec::new();
    };
    effects.remove(2);
    let mut found = Vec::new();
    for text in variants(&plain) {
        let Some((head, tail)) = text.split_once(", then shuffle") else {
            continue;
        };
        let said = format!("{head}, {discarded}, then shuffle{tail}");
        found.push(said.replace("put it into your hand", "put that card into your hand"));
        found.push(said);
    }
    found
}
