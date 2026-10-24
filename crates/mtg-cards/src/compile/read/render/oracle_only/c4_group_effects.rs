//! Continuous effects on a group or on a chosen object that the renderers do not word: control of every object a
//! selector names, a target chosen in a sentence of its own whose permanent is taken, silenced and held back, a random
//! creature taken for the turn with the rest destroyed, and the power and toughness of each creature doubled.

use super::super::super::selectors::{Form, phrase_text, spec_text};
use super::standin::with_effects;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::keyword::Keyword;
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, StatKind};
use mtg_core::card::CardType;

/// The effects and targets of an ability that does something when it resolves.
fn body(ability: &Ability) -> Option<(&[crate::refs::TargetSpec], &[Effect])> {
    match ability {
        Ability::Spell { targets, effects }
        | Ability::Activated {
            targets, effects, ..
        }
        | Ability::Triggered {
            targets, effects, ..
        } => Some((targets, effects)),
        _ => None,
    }
}

/// Adds the wordings of the effects this module knows.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((targets, effects)) = body(ability) else {
        return;
    };
    let sentence = match (targets, effects) {
        (_, [Effect::RepeatEach { over: Ref::Each(group), bind, effects }]) => doubled(group, bind, effects),
        (
            [],
            [Effect::Continuous {
                affected: Ref::Each(group),
                mods,
                duration: Duration::Forever,
            }],
        ) if matches!(mods.as_slice(), [Modification::SetController(Ref::You)]) => {
            phrase_text(group, Form::Each).map(|group| format!("gain control of each {group}"))
        }
        ([spec], [Effect::Continuous { affected: Ref::Target(0), mods, duration: Duration::WhileSourceOnBattlefield }])
            if matches!(
                mods.as_slice(),
                [
                    Modification::SetController(Ref::You),
                    Modification::LoseAbilities,
                    Modification::CantAttack,
                    Modification::CantBlock
                ]
            ) =>
        {
            spec_text(spec).map(|target| {
                format!("choose {target}. For as long as ~ remains on the battlefield, gain control of that permanent, it loses all abilities, and it can't attack or block")
            })
        }
        ([], effects) => lottery(effects),
        _ => None,
    };
    if let Some(sentence) = sentence {
        texts.extend(with_effects(ability, &sentence));
    }
}

/// "double the power and toughness of each creature you control until end of turn": a pass over the group that adds
/// each creature's own power and toughness to it.
fn doubled(group: &Selector, bind: &str, effects: &[Effect]) -> Option<String> {
    let own = |stat| Amount::Stat {
        of: Ref::Bound(bind.to_owned()),
        stat,
    };
    let [
        Effect::Continuous {
            affected: Ref::Bound(name),
            mods,
            duration: Duration::UntilEndOfTurn,
        },
    ] = effects
    else {
        return None;
    };
    let [Modification::ModifyPT { power, toughness }] = mods.as_slice() else {
        return None;
    };
    (name == bind && *power == own(StatKind::Power) && *toughness == own(StatKind::Toughness))
        .then_some(())?;
    Some(format!(
        "double the power and toughness of each {} until end of turn",
        phrase_text(group, Form::Each)?
    ))
}

/// "Choose a creature at random. You gain control of that creature until end of turn. Untap it. It gains haste until end
/// of turn. Then destroy all other creatures."
fn lottery(effects: &[Effect]) -> Option<String> {
    let creatures = Selector(vec![Pred::Type(CardType::Creature)]);
    let [
        Effect::ChooseAtRandom {
            from: Ref::Each(from),
            bind,
            ..
        },
        Effect::Continuous {
            affected: Ref::Bound(taken),
            mods,
            duration: Duration::UntilEndOfTurn,
        },
        Effect::Untap {
            what: Ref::Bound(untapped),
        },
        Effect::Destroy {
            what: Ref::Each(rest),
            ..
        },
    ] = effects
    else {
        return None;
    };
    let others = Selector(vec![
        Pred::Type(CardType::Creature),
        Pred::Not(Box::new(Pred::Is(Ref::Bound(bind.clone())))),
    ]);
    let haste = [
        Modification::SetController(Ref::You),
        Modification::AddKeyword(Keyword::Haste),
    ];
    (*from == creatures
        && taken == bind
        && untapped == bind
        && mods.as_slice() == haste
        && *rest == others)
        .then_some(
            "choose a creature at random. You gain control of that creature until end of turn. Untap it. It gains haste until end of turn. Then destroy all other creatures"
                .to_owned(),
        )
}
