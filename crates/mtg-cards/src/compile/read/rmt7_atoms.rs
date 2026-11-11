//! The noun-phrase words of filter atoms the remote card pack rmt-7 writes back after the noun: "named Wood" (a token
//! the card makes, or another card's name).

use super::selectors::{Form, granted_text, phrase_text};
use super::triggers::article;
use crate::ability::{Ability, Activators};
use crate::modify::Modification;
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, Cmp, Condition};

/// A kind with one name said after its noun ("a token named Wood", "a creature named Mowu"): the phrase of the other
/// atoms and the name, or `None` when the selector names no one name or its other atoms have no phrase.
pub(super) fn rmt7_named_phrase(selector: &Selector, form: Form) -> Option<String> {
    let names: Vec<&String> = selector
        .0
        .iter()
        .filter_map(|pred| match pred {
            Pred::Named(name) => Some(name),
            _ => None,
        })
        .collect();
    let [name] = names.as_slice() else {
        return None;
    };
    (!form.plural() && !name.is_empty()).then_some(())?;
    let rest = Selector(
        selector
            .0
            .iter()
            .filter(|pred| !matches!(pred, Pred::Named(_)))
            .cloned()
            .collect(),
    );
    (!rest.0.is_empty()).then_some(())?;
    Some(format!("{} named {name}", phrase_text(&rest, form)?))
}

/// The keywords a change takes away, after "loses": "hexproof and indestructible", "all landwalk abilities",
/// "hexproof, indestructible, protection, shroud, and ward". A kind of keyword is said by its kind words.
pub(super) fn rmt7_lost_text(group: &[Modification]) -> Option<String> {
    let keywords: Option<Vec<Modification>> = group
        .iter()
        .map(|gone| match gone {
            Modification::RemoveKeyword(keyword) => Some(Modification::AddKeyword(keyword.clone())),
            _ => None,
        })
        .collect();
    if let Some(keywords) = keywords {
        return granted_text(&keywords);
    }
    let mut parts = Vec::with_capacity(group.len());
    for gone in group {
        parts.push(match gone {
            Modification::RemoveKeyword(keyword) => {
                granted_text(&[Modification::AddKeyword(keyword.clone())])?
            }
            Modification::RemoveKeywordKind(kind) => kind.lost_words().to_owned(),
            _ => return None,
        });
    }
    Some(match parts.as_slice() {
        [rest @ .., last] if rest.len() >= 2 => format!("{}, and {last}", rest.join(", ")),
        _ => parts.join(" and "),
    })
}

/// A loyalty ability another permanent is given is printed with its cost in brackets: "[−8]: Create an 8/8 ...". Any other
/// ability's words are kept as they are. A given loyalty ability that names its own permanent ("this planeswalker") has no
/// words here: the renderer would name the card that gives it.
pub(super) fn rmt7_bracketed_loyalty(ability: &Ability, text: String) -> Option<String> {
    if !matches!(ability, Ability::Loyalty { .. }) {
        return Some(text);
    }
    if text.contains('~') {
        return None;
    }
    let (cost, rest) = text.split_once(": ")?;
    Some(format!("[{cost}]: {rest}"))
}

/// "you don't control a creature named Mowu": no permanent of a kind with one name under your control (`Count` below 1,
/// or not at least 1), the kind said in the singular.
pub(crate) fn rmt7_none_controlled(condition: &Condition) -> Option<String> {
    let selector = match condition {
        Condition::Compare {
            left: Amount::Count(selector),
            op: Cmp::Lt,
            right: Amount::Fixed(1),
        } => selector,
        Condition::Not(inner) => match &**inner {
            Condition::Compare {
                left: Amount::Count(selector),
                op: Cmp::Ge,
                right: Amount::Fixed(1),
            } => selector,
            _ => return None,
        },
        _ => return None,
    };
    let mine = Pred::ControlledBy(Ref::You);
    (selector.0.iter().filter(|pred| **pred == mine).count() == 1).then_some(())?;
    let kind = Selector(
        selector
            .0
            .iter()
            .filter(|pred| **pred != mine)
            .cloned()
            .collect(),
    );
    let text = rmt7_named_phrase(&kind, Form::Subject)?;
    Some(format!("you don't control {} {text}", article(&text)))
}

/// A change of an ability's cost while it is its controller's turn also reads "during your turn": "This ability costs {1}
/// less to activate during your turn." The sentence itself comes first.
pub(super) fn rmt7_during_your_turn(sentence: String) -> Vec<String> {
    let during = sentence
        .strip_suffix(" if it's your turn.")
        .map(|head| format!("{head} during your turn."));
    std::iter::once(sentence).chain(during).collect()
}

/// The predicate of a block capacity counted by an amount: "can block an additional creature each combat for each
/// Equipment attached to ~".
pub(super) fn rmt7_blocks_per_text(per: &Amount) -> Option<String> {
    Some(format!(
        "can block an additional creature each combat for each {}",
        super::amounts::for_each_text(per)?
    ))
}

/// An activated ability any player may activate: the words of the same ability its controller activates, then "Any player
/// may activate this ability.", or "... but only as a sorcery." in place of its "Activate only ...".
pub(super) fn rmt7_any_player_texts(
    ability: &Ability,
    texts: impl Fn(&Ability) -> Vec<String>,
) -> Vec<String> {
    let Ability::Activated {
        cost,
        timing,
        condition,
        limit,
        by: Activators::Anyone,
        from,
        targets,
        effects,
    } = ability
    else {
        return Vec::new();
    };
    let controller = Ability::Activated {
        cost: cost.clone(),
        timing: *timing,
        condition: condition.clone(),
        limit: *limit,
        by: Activators::Controller,
        from: *from,
        targets: targets.clone(),
        effects: effects.clone(),
    };
    texts(&controller)
        .into_iter()
        .map(|text| match text.split_once(" Activate only ") {
            // "Any player may activate this ability but only as a sorcery.", "... but only during their turn."
            Some((body, only)) => format!(
                "{body} Any player may activate this ability but only {}",
                only.replace("during your turn", "during their turn")
            ),
            None => format!("{text} Any player may activate this ability."),
        })
        .collect()
}

/// The words of an ability a permanent loses, in quotes after "loses": its shortest wording that names the card, as cards
/// print the ability ("Prevent all damage that would be dealt to this creature.").
pub(super) fn rmt7_quoted_lost(lost: &Ability) -> Option<String> {
    super::render::written_variants(lost)
        .into_iter()
        .filter(|text| !text.contains(" him") && !text.contains(" her"))
        .min_by_key(String::len)
}

/// The tail of a static ability's sentence for a condition no "as long as" says: " during extra turns".
pub(super) fn rmt7_condition_tail(condition: &Condition) -> Option<String> {
    (*condition == Condition::ExtraTurn).then(|| " during extra turns".to_owned())
}

/// "Enchanted player can't activate abilities that aren't mana abilities or loyalty abilities."
pub(super) fn rmt7_cant_activate_text(
    who: &Ref,
    action: &crate::modify::Prohibited,
) -> Option<String> {
    let crate::modify::Prohibited::ActivateKind {
        kind: crate::ability::ActivationKind::AnyButLoyalty,
        sources,
    } = action
    else {
        return None;
    };
    (*who == Ref::AttachedPlayer && sources.0.is_empty()).then(|| {
        "Enchanted player can't activate abilities that aren't mana abilities or loyalty abilities"
            .to_owned()
    })
}
