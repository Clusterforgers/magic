//! Wordings of pack r47-00 that no template reads, only the proposal checker writes: "This land is the chosen type."
//! (its land types set to the one chosen as it entered) and two choices made as the card enters, written as one
//! sentence ("As this land enters, choose a basic land type. Then you may pay 2 life. If you don't, it enters
//! tapped.").

use crate::ability::Ability;
use crate::modify::Modification;
use crate::refs::{Ref, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// "~ is the chosen type.": the card's land types set to the basic land type chosen as it entered; and a trigger whose
/// condition only this pack words, with the condition after its event ("Whenever you cast a spell, if it's at least one
/// of the chosen colors, ...").
pub(super) fn r4700_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    if let Ability::Triggered {
        intervening_if: Some(held),
        ..
    } = ability
        && let Some(said) = r4700_condition_text(held)
    {
        let mut plain = ability.clone();
        if let Ability::Triggered { intervening_if, .. } = &mut plain {
            *intervening_if = None;
        }
        for text in written(&plain) {
            let text = text.replacen(", ", &format!(", if {said}, "), 1);
            if !texts.contains(&text) {
                texts.push(text);
            }
        }
        return;
    }
    let Ability::Static {
        affected: Ref::This,
        mods,
        condition: None,
        defining: false,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return;
    };
    if mods.as_slice()
        == [
            Modification::SetLandTypes(Vec::new()),
            Modification::AddChosenSubtype,
        ]
    {
        let text = "~ is the chosen type.".to_owned();
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// Whether the ability replaces the card's own entering, and how.
fn own_entering(ability: &Ability) -> Option<&ReplaceBy> {
    match ability {
        Ability::Replacement {
            when: ReplaceWhen::Enters { what },
            by,
            condition: None,
            from: ZoneKind::Battlefield,
        } if what.0 == [crate::refs::Pred::This] => Some(by),
        _ => None,
    }
}

/// "As ~ enters, choose a basic land type. Then you may pay 2 life. If you don't, it enters tapped.": a choice made as
/// the card enters, then a payment of life or the card enters tapped; empty for any other two abilities.
pub(super) fn r4700_pair_texts(
    first: &Ability,
    second: &Ability,
    written: fn(&Ability) -> Vec<String>,
) -> Vec<String> {
    let (Some(ReplaceBy::Choose(_)), Some(ReplaceBy::PayLifeOrTapped(life))) =
        (own_entering(first), own_entering(second))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for text in written(first) {
        let Some(choice) = text
            .strip_prefix("As ~ enters, choose ")
            .or_else(|| text.strip_prefix("As it enters, choose "))
        else {
            continue;
        };
        let text = format!(
            "As ~ enters, choose {}. Then you may pay {life} life. If you don't, it enters tapped.",
            choice.trim_end_matches('.')
        );
        if !out.contains(&text) {
            out.push(text);
        }
    }
    out
}

/// "create a 2/2 creature token of the chosen color and type" and "create an X/X creature token of the chosen color and
/// type, where X is the number of charge counters on ~": one token of the colour and type the source chose, made for its
/// controller untapped; `None` for any other.
pub(super) fn r4700_chosen_token_text(
    who: &Ref,
    size: &crate::value::Amount,
    count: &crate::value::Amount,
    effect: &crate::effect::Effect,
) -> Option<String> {
    let crate::effect::Effect::CreateToken {
        tapped: false,
        attacking: false,
        against: None,
        blocking: None,
        ..
    } = effect
    else {
        return None;
    };
    (*who == Ref::You && *count == crate::value::Amount::one()).then_some(())?;
    let what = "creature token of the chosen color and type";
    match size {
        crate::value::Amount::Fixed(n) => Some(format!("create a {n}/{n} {what}")),
        other => {
            let said = super::defining::value_texts(other).into_iter().next()?;
            Some(format!("create an X/X {what}, where X is {said}"))
        }
    }
}

/// "it's at least one of the chosen colors": the triggering spell has one of the colours the source chose as it
/// entered; `None` for any other condition.
pub(super) fn r4700_condition_text(condition: &crate::value::Condition) -> Option<String> {
    use crate::value::{Amount, Cmp, Condition, StatKind};
    let Condition::Compare {
        left:
            Amount::Stat {
                of: Ref::Triggering,
                stat: StatKind::R4700ChosenColors,
            },
        op: Cmp::Ge,
        right: Amount::Fixed(1),
    } = condition
    else {
        return None;
    };
    Some("it's at least one of the chosen colors".to_owned())
}
