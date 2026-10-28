//! The steps under a choice or a payment, each worded in lower case without its period: the wordings of the steps as a
//! spell of their own, and the pieces no template writes ("put a land card from your hand or graveyard onto the
//! battlefield tapped", "exile another target creature you control, then return that card to the battlefield under its
//! owner's control", "target creature with haste can't be blocked this turn except by creatures with haste", "have the
//! base power and toughness of other creatures you control become equal to ~'s power and toughness until end of turn",
//! "counter that spell or ability", "Those permanents don't untap during their controllers' next untap steps" and a
//! returned card that "has base power and toughness 4/4" and "gains haste until end of turn").

use super::super::super::selectors::{Form, keyword_text, phrase_text};
use super::super::super::triggers::article;
use super::super::{variants, written_variants};
use super::bare::lowered;
use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, StatKind};

/// The wordings of the steps after the head of the ability, in lower case and without the last period.
pub(super) fn bodies(ability: &Ability, steps: &[Effect]) -> Vec<String> {
    let spell = Ability::Spell {
        targets: ability.targets().to_vec(),
        effects: steps.to_vec(),
    };
    let mut out: Vec<String> = written_variants(&spell)
        .iter()
        .filter_map(|text| text.strip_suffix('.'))
        .map(lowered)
        .collect();
    let target = target_phrase(ability);
    out.extend(put_from_zone(ability, steps));
    out.extend(target.as_deref().and_then(|target| flicker(target, steps)));
    out.extend(
        target
            .as_deref()
            .and_then(|target| unblockable_except(target, steps)),
    );
    out.extend(returned_and_changed(ability, steps));
    out.extend(base_like_this(steps));
    out.extend(counter_targeting(ability, steps));
    out.extend(damage_to_itself(steps));
    let plural: Vec<String> = out
        .iter()
        .filter_map(|body| those_targets(ability, body))
        .collect();
    out.extend(plural);
    out.sort();
    out.dedup();
    out
}

/// The first target as a phrase: "another target creature you control", read off the exile of it as a spell.
fn target_phrase(ability: &Ability) -> Option<String> {
    let spell = Ability::Spell {
        targets: ability.targets().to_vec(),
        effects: vec![Effect::Exile {
            what: Ref::Target(0),
            bind: None,
            duration: None,
        }],
    };
    let text = variants(&spell).into_iter().next()?;
    Some(text.strip_prefix("Exile ")?.strip_suffix('.')?.to_owned())
}

/// "put a land card from your hand onto the battlefield", "... from your hand or graveyard ... tapped", and "... tapped
/// and attacking that opponent" when the card joins the attack of the creature of an attack trigger.
fn put_from_zone(ability: &Ability, steps: &[Effect]) -> Option<String> {
    let (choose, moved, attacking) = match steps {
        [choose, moved] => (choose, moved, false),
        [choose, moved, Effect::JoinAttack { what }] => (choose, moved, *what == bound(choose)?),
        _ => return None,
    };
    let Effect::Choose {
        who: Ref::You,
        from: Ref::Each(Selector(preds)),
        count,
        bind,
        rest_as: None,
    } = choose
    else {
        return None;
    };
    let Effect::Move {
        what: Ref::Bound(name),
        to:
            Destination::Battlefield {
                tapped,
                under: None,
                transformed: false,
                counters,
            },
    } = moved
    else {
        return None;
    };
    if name != bind
        || !count.is_one()
        || !counters.is_empty()
        || !preds.contains(&Pred::OwnedBy(Ref::You))
    {
        return None;
    }
    let both = Pred::AnyOf(vec![
        Pred::In(ZoneKind::Hand),
        Pred::In(ZoneKind::Graveyard),
    ]);
    let zones = if preds.contains(&Pred::In(ZoneKind::Hand)) {
        "your hand"
    } else if preds.contains(&both) {
        "your hand or graveyard"
    } else {
        return None;
    };
    let rest: Vec<Pred> = preds
        .iter()
        .filter(|pred| !matches!(pred, Pred::OwnedBy(_) | Pred::In(_)) && **pred != both)
        .cloned()
        .collect();
    let phrase = card_phrase(rest)?;
    let phrase = phrase.as_str();
    let tapped = if *tapped { " tapped" } else { "" };
    let joined = match (attacking, ability) {
        (false, _) => "",
        (
            true,
            Ability::Triggered {
                when: TriggerPattern::AttacksPlayer { .. },
                ..
            },
        ) => " and attacking that opponent",
        (true, _) => return None,
    };
    Some(format!(
        "put {} {phrase} from {zones} onto the battlefield{tapped}{joined}",
        article(phrase)
    ))
}

/// The kind of card a selector names without its zone: "land card", "Angel, Demon, or Dragon creature card".
pub(super) fn card_phrase(kind: Vec<Pred>) -> Option<String> {
    let mut held = vec![Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)];
    held.extend(kind);
    let phrase = phrase_text(&Selector(held), Form::Subject)?;
    phrase.strip_suffix(" from your hand").map(str::to_owned)
}

/// The name a choice binds, as a reference.
fn bound(choose: &Effect) -> Option<Ref> {
    match choose {
        Effect::Choose { bind, .. } => Some(Ref::Bound(bind.clone())),
        _ => None,
    }
}

/// "exile another target creature you control, then return that card to the battlefield under its owner's control".
fn flicker(target: &str, steps: &[Effect]) -> Option<String> {
    let [
        Effect::Exile {
            what: Ref::Target(0),
            bind: None,
            duration: None,
        },
        Effect::Move {
            what: Ref::Target(0),
            to:
                Destination::Battlefield {
                    tapped: false,
                    under: None,
                    transformed: false,
                    counters,
                },
        },
    ] = steps
    else {
        return None;
    };
    counters.is_empty().then(|| {
        format!(
            "exile {target}, then return that card to the battlefield under its owner's control"
        )
    })
}

/// "target creature with haste can't be blocked this turn except by creatures with haste".
fn unblockable_except(target: &str, steps: &[Effect]) -> Option<String> {
    let [
        Effect::Continuous {
            affected: Ref::Target(0),
            mods,
            duration: Duration::UntilEndOfTurn,
        },
    ] = steps
    else {
        return None;
    };
    let [Modification::CantBeBlockedBy(Selector(preds))] = mods.as_slice() else {
        return None;
    };
    let [Pred::Not(inner)] = preds.as_slice() else {
        return None;
    };
    let mut blockers = vec![Pred::Type(mtg_core::card::CardType::Creature)];
    blockers.push((**inner).clone());
    let blockers = phrase_text(&Selector(blockers), Form::All)?;
    Some(format!(
        "{target} can't be blocked this turn except by {blockers}"
    ))
}

/// "have the base power and toughness of other creatures you control become equal to ~'s power and toughness until end
/// of turn".
fn base_like_this(steps: &[Effect]) -> Option<String> {
    let [
        Effect::Continuous {
            affected: Ref::Each(group),
            mods,
            duration: Duration::UntilEndOfTurn,
        },
    ] = steps
    else {
        return None;
    };
    let [Modification::SetPT { power, toughness }] = mods.as_slice() else {
        return None;
    };
    let of_this = |amount: &Amount, wanted: StatKind| matches!(amount, Amount::Stat { of: Ref::This, stat } if *stat == wanted);
    if !of_this(power, StatKind::Power) || !of_this(toughness, StatKind::Toughness) {
        return None;
    }
    let group = phrase_text(group, Form::All)?;
    Some(format!(
        "have the base power and toughness of {group} become equal to ~'s power and toughness until end of turn"
    ))
}

/// "counter that spell or ability": the spell or ability that made the source of the trigger a target.
fn counter_targeting(ability: &Ability, steps: &[Effect]) -> Option<String> {
    let Ability::Triggered {
        when: TriggerPattern::BecomesTarget { .. },
        ..
    } = ability
    else {
        return None;
    };
    matches!(
        steps,
        [Effect::Counter {
            what: Ref::Triggering
        }]
    )
    .then(|| "counter that spell or ability".to_owned())
}

/// "have ~ deal 1 damage to itself": the source is made to deal damage to itself.
fn damage_to_itself(steps: &[Effect]) -> Option<String> {
    let [
        Effect::DealDamage {
            source: Some(Ref::This),
            to: Ref::This,
            amount: Amount::Fixed(amount),
        },
    ] = steps
    else {
        return None;
    };
    Some(format!("have ~ deal {amount} damage to itself"))
}

/// "Those permanents don't untap during their controllers' next untap steps": the targets of "up to N" are several
/// permanents, each with its own controller.
fn those_targets(ability: &Ability, body: &str) -> Option<String> {
    let [spec] = ability.targets() else {
        return None;
    };
    (spec.bounds.max != Some(Amount::one())).then_some(())?;
    let (_, after) = body.split_once(" target ")?;
    let noun = after.split(['.', ' ']).next()?;
    body.contains(". They don't untap during their controller's next untap step")
        .then(|| {
            body.replacen(
                ". They don't untap during their controller's next untap step",
                &format!(". Those {noun} don't untap during their controllers' next untap steps"),
                1,
            )
        })
}

/// "return target Pirate creature card from your graveyard to the battlefield with a finality counter on it. It has
/// base power and toughness 4/4. It gains haste until end of turn.": the return as a spell, then what the card becomes.
fn returned_and_changed(ability: &Ability, steps: &[Effect]) -> Option<String> {
    let (
        moved @ Effect::Move {
            what: Ref::Target(0),
            ..
        },
        changes,
    ) = steps.split_first()?
    else {
        return None;
    };
    if changes.is_empty() {
        return None;
    }
    let mut sentences = Vec::new();
    for change in changes {
        sentences.push(change_of_it(change)?);
    }
    let spell = Ability::Spell {
        targets: ability.targets().to_vec(),
        effects: vec![moved.clone()],
    };
    let first = variants(&spell).into_iter().next()?;
    let text = format!("{} {}", lowered(&first), sentences.join(" "));
    text.strip_suffix('.').map(str::to_owned)
}

/// "It has base power and toughness 4/4." and "It gains haste until end of turn." about the target.
fn change_of_it(change: &Effect) -> Option<String> {
    let Effect::Continuous {
        affected: Ref::Target(0),
        mods,
        duration,
    } = change
    else {
        return None;
    };
    match (mods.as_slice(), duration) {
        (
            [
                Modification::SetPT {
                    power: Amount::Fixed(power),
                    toughness: Amount::Fixed(toughness),
                },
            ],
            Duration::Forever,
        ) => Some(format!(
            "It has base power and toughness {power}/{toughness}."
        )),
        ([Modification::AddKeyword(keyword)], Duration::UntilEndOfTurn) => Some(format!(
            "It gains {} until end of turn.",
            keyword_text(keyword)?
        )),
        _ => None,
    }
}
