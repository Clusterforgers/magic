//! Groups and counts that follow an attachment or the object a static ability or an effect acts on: "Enchanted creatures
//! you control", "Equipped Warriors you control", "Aura attached to it", "of its colors" and "creature that's a
//! Barbarian, a Warrior, or a Berserker".

use mtg_core::card::Subtype;

use super::selectors::Form;
use super::tokens::Reader;
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, StatKind};

/// An Aura or an Equipment attached to the object.
fn attached(kind: &str) -> Pred {
    Pred::HasAttached(Selector(vec![Pred::Subtype(Subtype(kind.to_owned()))]))
}

/// Whether the form is a group or a count of them; a single "enchanted creature" is the host of an Aura (`Ref::Attached`).
const fn grouped(form: Form) -> bool {
    matches!(form, Form::All | Form::Targets | Form::Each | Form::Count)
}

/// "Enchanted creatures you control" and "equipped Warriors you control": the permanents that have an Aura or an
/// Equipment attached.
pub(super) fn adjective(word: &str, form: Form) -> Option<Pred> {
    if !grouped(form) {
        return None;
    }
    match word {
        "enchanted" => Some(attached("Aura")),
        "equipped" => Some(attached("Equipment")),
        _ => None,
    }
}

/// The adjective for an Aura or an Equipment attached.
pub(super) fn adjective_text(pred: &Pred, form: Form) -> Option<String> {
    if !grouped(form) {
        return None;
    }
    match pred {
        pred if *pred == attached("Aura") => Some("enchanted".to_owned()),
        pred if *pred == attached("Equipment") => Some("equipped".to_owned()),
        _ => None,
    }
}

/// What "it" and "its" name where a static ability or an effect reads: the object the last sentence acted on, else the
/// subject.
pub(super) fn its(r: &Reader<'_>) -> Option<Ref> {
    r.it.clone().or_else(|| r.subject.clone())
}

/// The phrases after the noun: "attached to it" and "that's a Barbarian, a Warrior, or a Berserker".
pub(super) fn suffix(r: &mut Reader<'_>, form: Form) -> Option<Pred> {
    if r.words("attached to it") {
        return its(r).map(Pred::AttachedTo);
    }
    if form.plural() {
        return None;
    }
    r.attempt(kinds)
}

/// "that's a Barbarian, a Warrior, or a Berserker": two or more creature types, any of them.
fn kinds(r: &mut Reader<'_>) -> Option<Pred> {
    r.need("that's")?;
    let mut found = Vec::new();
    loop {
        (r.word("a") || r.word("an")).then_some(())?;
        found.push(Pred::Subtype(Subtype(super::selectors::subtype(r)?)));
        let more = r.punct(',');
        if r.word("or") {
            continue;
        }
        if !more {
            break;
        }
    }
    (found.len() >= 2).then_some(Pred::AnyOf(found))
}

/// Whether the predicate is a list of creature types of a creature group in the singular, which the phrase says after the
/// noun ("creature that's a Goblin or a Warrior") and not as the noun ("Faerie or Wizard spell").
pub(super) fn is_kinds(pred: &Pred, all: &[Pred], form: Form) -> bool {
    matches!(pred, Pred::AnyOf(list) if list.len() >= 2 && list.iter().all(|kind| matches!(kind, Pred::Subtype(_))))
        && !form.plural()
        && all.contains(&Pred::Type(mtg_core::card::CardType::Creature))
        && !all
            .iter()
            .any(|other| matches!(other, Pred::Spell | Pred::In(_)))
}

/// The words after the noun for a predicate of this module.
pub(super) fn suffix_text(pred: &Pred, form: Form) -> Option<String> {
    match pred {
        Pred::AttachedTo(Ref::This | Ref::Attached | Ref::Target(0)) => {
            Some("attached to it".to_owned())
        }
        Pred::AnyOf(list) if !form.plural() && list.len() >= 2 => {
            let names: Vec<String> = list
                .iter()
                .map(|pred| match pred {
                    Pred::Subtype(Subtype(name)) => {
                        Some(format!("{} {name}", super::triggers::article(name)))
                    }
                    _ => None,
                })
                .collect::<Option<_>>()?;
            let (last, rest) = names.split_last()?;
            Some(match rest {
                [one] => format!("that's {one} or {last}"),
                _ => format!("that's {}, or {last}", rest.join(", ")),
            })
        }
        _ => None,
    }
}

/// "of its colors" after "for each": the colors of the object the static ability or effect acts on.
pub(super) fn of_its_colors(r: &mut Reader<'_>) -> Option<Amount> {
    r.words("of its colors").then_some(())?;
    Some(Amount::Stat {
        of: colored(r)?,
        stat: StatKind::Colors,
    })
}

/// Whose colors "its" names: the card or the permanent it is attached to in a static ability, the only target of a spell
/// or ability; a trigger leaves it open.
fn colored(r: &Reader<'_>) -> Option<Ref> {
    if r.static_grant {
        return its(r).filter(|of| matches!(of, Ref::This | Ref::Attached));
    }
    (r.targets.len() == 1).then_some(Ref::Target(0))
}

/// The words after "for each" for the colors of an object.
pub(super) fn of_its_colors_text(amount: &Amount) -> Option<String> {
    match amount {
        Amount::Stat {
            of: Ref::This | Ref::Attached | Ref::Target(0),
            stat: StatKind::Colors,
        } => Some("of its colors".to_owned()),
        _ => None,
    }
}
