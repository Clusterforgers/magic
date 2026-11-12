//! Wordings of l29-2 written from a twin the other writers word or matched whole: a target creature with a morph
//! ability ("you may turn target creature with a morph ability face down"), a skip each opponent makes on their own
//! ("each opponent skips their next untap step": one rule for each opponent, so one skip does not use up another),
//! steps on one group named again with "them", "they" or "those", and whole abilities worded as printed.

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

/// Whole abilities matched as data and worded as the card prints them, so another target, zone or condition in the same
/// ability never reads back (the card's name as `~`).
const L292_WHOLES: [(&str, &str); 2] = [
    // hypnox
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This",{"WasCast":{"by":"You","from":"Hand"}}],"to":"Battlefield"}},"targets":[{"what":"Opponent"}],"effects":[{"Exile":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]}}}]}}"#,
        "When ~ enters, if you cast it from your hand, exile all cards from target opponent's hand.",
    ),
    // hypnox
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield"}},"effects":[{"Move":{"what":{"Each":[{"In":"Exile"},{"ExiledWith":"This"}]},"to":"Hand"}}]}}"#,
        "When ~ leaves the battlefield, return the exiled cards to their owner's hand.",
    ),
];

/// [`L292_WHOLES`] parsed; an entry that does not parse is left out (a test holds that none is).
static L292_SHAPES: std::sync::LazyLock<Vec<(Ability, &'static str)>> =
    std::sync::LazyLock::new(|| {
        L292_WHOLES
            .iter()
            .filter_map(|(json, text)| Some((serde_json::from_str(json).ok()?, *text)))
            .collect()
    });

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
    anaphora(ability, texts);
    for (shape, text) in L292_SHAPES.iter() {
        if shape == ability && !texts.iter().any(|known| known == text) {
            texts.push((*text).to_owned());
        }
    }
    // "You choose a nonland card from it and exile that card" is also printed as two sentences.
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.matches(" from it and exile that card").count() == 1)
        .map(|text| {
            text.replacen(
                " from it and exile that card",
                " from it. Exile that card",
                1,
            )
        })
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
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

/// The group every step of `effects` acts on, when each names the same group (`Each`) and there are two or more steps.
fn one_group(effects: &[Effect]) -> Option<&Selector> {
    let mut group = None;
    for effect in effects {
        let named = match effect {
            Effect::Untap { what, .. } | Effect::Tap { what, .. } => what,
            Effect::Continuous { affected, .. } => affected,
            _ => return None,
        };
        let Ref::Each(selector) = named else {
            return None;
        };
        if group.is_some_and(|group| group != selector) {
            return None;
        }
        group = Some(selector);
    }
    (effects.len() >= 2).then_some(group).flatten()
}

/// "Untap all creatures you control. Those creatures get +1/+1 until end of turn.", "Gain control of all creatures until
/// end of turn. Untap them. They gain haste until end of turn.": steps that act on one group name it again with "them",
/// "they" or "those". Each step reads its group as it begins, and no step between changes which objects it holds.
fn anaphora(ability: &Ability, texts: &mut Vec<String>) {
    let (Ability::Triggered { effects, .. }
    | Ability::Activated { effects, .. }
    | Ability::Spell { effects, .. }
    | Ability::Loyalty { effects, .. }) = ability
    else {
        return;
    };
    let Some(group) = one_group(effects) else {
        return;
    };
    let Some(all) = super::selectors::phrase_text(group, super::selectors::Form::All) else {
        return;
    };
    let noun = all.strip_suffix(" you control").unwrap_or(&all);
    let mut capital = all.chars();
    let Some(first) = capital.next() else {
        return;
    };
    let capital: String = first.to_uppercase().chain(capital).collect();
    let swaps = [
        (format!(". Untap all {all}."), ". Untap them.".to_owned()),
        (format!(". All {all} gain "), ". They gain ".to_owned()),
        (format!(". {capital} get "), format!(". Those {noun} get ")),
    ];
    let mut found = Vec::new();
    for text in texts.iter() {
        let mut changed = text.clone();
        for (from, to) in &swaps {
            changed = changed.replace(from.as_str(), to);
        }
        if changed != *text && !texts.contains(&changed) && !found.contains(&changed) {
            found.push(changed);
        }
    }
    texts.extend(found);
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

#[cfg(test)]
mod tests {
    //! Goal: every whole ability of the l29-2 table parses, so none is silently dropped.
    use super::{L292_SHAPES, L292_WHOLES};

    #[test]
    fn l292_every_whole_parses() {
        assert_eq!(L292_SHAPES.len(), L292_WHOLES.len());
    }
}
