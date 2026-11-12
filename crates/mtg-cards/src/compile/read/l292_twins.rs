//! Wordings of l29-2 written from a twin the other writers word: a target creature with a morph ability ("you may turn
//! target creature with a morph ability face down"), and a skip each opponent makes on their own ("each opponent skips
//! their next untap step": one rule for each opponent, so one opponent's skip does not use up another's).

use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::GameRule;
use crate::refs::{Pred, Ref, Selector, TargetSpec, Targetable};
use mtg_core::card::CardType;

/// A twin of the ability, the words it says that the ability says otherwise, and the ability's words.
struct Twin {
    /// The twin.
    ability: Ability,
    /// The words of the twin to replace; every wording of the twin that says them once counts.
    said: &'static str,
    /// The ability's words for them.
    words: &'static str,
}

/// The morph abilities (CR 702.37b: megamorph is a variant of morph).
fn morph_abilities() -> Pred {
    Pred::AnyOf(vec![
        Pred::HasKeywordNamed("Morph".to_owned()),
        Pred::HasKeywordNamed("Megamorph".to_owned()),
    ])
}

/// Adds the wordings of the twins of `ability`, each written by `written` with its words replaced.
pub(super) fn l292_twin_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    for twin in l292_twins(ability) {
        for text in written(&twin.ability) {
            let text = if twin.said.is_empty() {
                text
            } else if text.matches(twin.said).count() == 1 {
                text.replacen(twin.said, twin.words, 1)
            } else {
                continue;
            };
            if !texts.contains(&text) {
                texts.push(text);
            }
        }
    }
}

/// The twins of `ability`, none when no shape fits.
fn l292_twins(ability: &Ability) -> Vec<Twin> {
    [with_morph(ability), each_skips(ability)]
        .into_iter()
        .flatten()
        .collect()
}

/// "target creature with a morph ability": the twin targets a plain creature.
fn with_morph(ability: &Ability) -> Option<Twin> {
    let mut twin = ability.clone();
    let (Ability::Triggered { targets, .. }
    | Ability::Activated { targets, .. }
    | Ability::Spell { targets, .. }) = &mut twin
    else {
        return None;
    };
    let [spec] = targets.as_mut_slice() else {
        return None;
    };
    let Targetable::Object(selector) = &spec.what else {
        return None;
    };
    let creature = Pred::Type(CardType::Creature);
    (selector.0 == [creature.clone(), morph_abilities()]).then_some(())?;
    *spec = TargetSpec {
        what: Targetable::Object(Selector(vec![creature])),
        ..spec.clone()
    };
    Some(Twin {
        ability: twin,
        said: "target creature",
        words: "target creature with a morph ability",
    })
}

/// "each opponent skips their next untap step": a rule made for each opponent in turn is worded as one rule about them all.
fn each_skips(ability: &Ability) -> Option<Twin> {
    let mut twin = ability.clone();
    let (Ability::Triggered { effects, .. }
    | Ability::Activated { effects, .. }
    | Ability::Spell { effects, .. }) = &mut twin
    else {
        return None;
    };
    let mut changed = false;
    for effect in effects.iter_mut() {
        let Effect::RepeatEach {
            over: Ref::Opponents,
            bind,
            effects: inner,
        } = effect
        else {
            continue;
        };
        let [
            Effect::Rule {
                rule: GameRule::Cant { who, action },
                duration,
            },
        ] = inner.as_slice()
        else {
            continue;
        };
        if *who != Ref::Bound(bind.clone()) {
            continue;
        }
        *effect = Effect::Rule {
            rule: GameRule::Cant {
                who: Ref::Opponents,
                action: action.clone(),
            },
            duration: duration.clone(),
        };
        changed = true;
    }
    changed.then_some(Twin {
        ability: twin,
        said: "",
        words: "",
    })
}
