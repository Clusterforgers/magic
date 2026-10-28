//! Choices and payments around steps the writers word apart: "you may S", "you may pay {C}. If you do, S", the reflexive
//! "you may pay {C}. When you do, S" whose steps choose targets, "S unless its controller pays {C}" and "Then that player
//! S unless they discard a land card". The steps S are the wordings of the steps as a spell of their own, or of the
//! pieces in `w4c_bodies`, after the head of the ability.

use super::super::super::payments::{may_pay_text, unless_text};
use super::super::super::triggers::article;
use super::super::variants;
use super::bare::{lowered, with_effects};
use super::standin;
use super::w4c_bodies;
use crate::ability::Ability;
use crate::cost::{Cost, CostItem};
use crate::effect::Effect;
use crate::refs::{Ref, Selector};

/// Adds the wordings of an ability whose one effect is a choice or a payment around steps; a trigger on an attack of
/// "one of your opponents" is also printed "an opponent".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let start = texts.len();
    match ability.effects() {
        [
            Effect::May {
                who: Ref::You,
                effects,
            },
        ] => {
            for body in w4c_bodies::bodies(ability, effects) {
                texts.extend(standin::with_effects(ability, &format!("you may {body}")));
            }
        }
        [
            before @ ..,
            Effect::May {
                who: Ref::You,
                effects,
            },
        ] if !before.is_empty() => {
            for first in w4c_bodies::bodies(ability, before) {
                for body in w4c_bodies::bodies(ability, effects) {
                    let sentence = format!("{first}, then you may {body}");
                    texts.extend(standin::with_effects(ability, &sentence));
                }
            }
        }
        [
            Effect::Pay {
                who: Ref::You,
                cost,
                times,
                paid,
                unpaid,
            },
        ] if times.is_one() && unpaid.is_empty() && !paid.is_empty() => {
            may_pay(ability, cost, paid, texts);
        }
        [
            Effect::Pay {
                who,
                cost,
                times,
                paid,
                unpaid,
            },
        ] if times.is_one() && paid.is_empty() && *who != Ref::You => {
            unless(ability, who, cost, unpaid, texts);
        }
        [
            before @ ..,
            Effect::Pay {
                who,
                cost,
                times,
                paid,
                unpaid,
            },
        ] if !before.is_empty() && times.is_one() && paid.is_empty() => {
            then_unless(ability, before, who, cost, unpaid, texts);
        }
        _ => {}
    }
    let heads: Vec<String> = texts[start..]
        .iter()
        .filter(|text| text.contains(" attacks one of your opponents, "))
        .map(|text| {
            text.replacen(
                " attacks one of your opponents, ",
                " attacks an opponent, ",
                1,
            )
        })
        .collect();
    texts.extend(heads);
}

/// "you may pay {C}. If you do, S", and "When you do, S" when the steps use a target: the targets of a reflexive
/// trigger are chosen once the cost is paid (CR 603.12).
fn may_pay(ability: &Ability, cost: &Cost, paid: &[Effect], texts: &mut Vec<String>) {
    let reflexive = uses_target(paid);
    for body in w4c_bodies::bodies(ability, paid) {
        let Some(text) = may_pay_text(cost, &body) else {
            continue;
        };
        if reflexive {
            let linked = text.replacen(". If you do, ", ". When you do, ", 1);
            texts.extend(standin::with_effects(ability, &linked));
        }
        texts.extend(standin::with_effects(ability, &text));
    }
}

/// Whether some step names a target of the ability.
fn uses_target(effects: &[Effect]) -> bool {
    serde_json::to_string(effects).is_ok_and(|json| json.contains("{\"Target\":"))
}

/// "counter that spell or ability unless its controller pays {2}": the step happens unless another player pays.
fn unless(ability: &Ability, who: &Ref, cost: &Cost, unpaid: &[Effect], texts: &mut Vec<String>) {
    let Some(payer) = payer_text(who) else {
        return;
    };
    let Some(tail) = unless_text(payer, who, cost) else {
        return;
    };
    for body in w4c_bodies::bodies(ability, unpaid) {
        texts.extend(standin::with_effects(ability, &format!("{body} {tail}")));
    }
}

/// The player who may pay, as the payer after "unless".
fn payer_text(who: &Ref) -> Option<&'static str> {
    match who {
        Ref::ControllerOf(of) if **of == Ref::Triggering => Some("its controller"),
        Ref::ControllerOf(of) if **of == Ref::Target(0) => Some("its controller"),
        Ref::Target(0) => Some("that player"),
        _ => None,
    }
}

/// "Target player draws three cards. Then that player discards two cards unless they discard a land card.": the steps
/// before, then the step of the target player that a payment of their own avoids.
fn then_unless(
    ability: &Ability,
    before: &[Effect],
    who: &Ref,
    cost: &Cost,
    unpaid: &[Effect],
    texts: &mut Vec<String>,
) {
    let [inner] = unpaid else {
        return;
    };
    if *who != Ref::Target(0) || !acts(inner, who) {
        return;
    }
    let Some(tail) = own_payment(cost) else {
        return;
    };
    let Some(head) = with_effects(ability, before.to_vec()) else {
        return;
    };
    let steps = Ability::Spell {
        targets: ability.targets().to_vec(),
        effects: vec![inner.clone()],
    };
    for step in variants(&steps) {
        let Some(rest) = step.strip_prefix("Target player ") else {
            continue;
        };
        let rest = lowered(rest.strip_suffix('.').unwrap_or(rest));
        for head in variants(&head) {
            texts.push(format!(
                "{head} Then that player {rest} unless they {tail}."
            ));
        }
    }
}

/// The payment as the verb phrase of its payer: "pay {2}", and "discard a land card" for a discard of a kind of card.
fn own_payment(cost: &Cost) -> Option<String> {
    if let [
        CostItem::Discard {
            count,
            filter: Some(Selector(kind)),
        },
    ] = cost.0.as_slice()
    {
        let kind = w4c_bodies::card_phrase(kind.clone())?;
        return count
            .is_one()
            .then(|| format!("discard {} {kind}", article(&kind)));
    }
    let tail = unless_text("you", &Ref::You, cost)?;
    tail.strip_prefix("unless you ").map(str::to_owned)
}

/// Whether the step is one the player does: a discard or a draw of theirs.
fn acts(effect: &Effect, who: &Ref) -> bool {
    match effect {
        Effect::Discard { who: player, .. } | Effect::Draw { who: player, .. } => player == who,
        _ => false,
    }
}
