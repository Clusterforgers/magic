//! The spells a static ability gives a keyword to, in the form the Forge translator writes them (a selector in the zone
//! of the stack): "Artifact spells you cast have convoke.", "Instant and sorcery spells you control have lifelink.",
//! "The first historic spell you cast each turn has convoke." Written back in the card's words.

use super::render::capitalize;
use super::selectors::{Form, phrase_text};
use super::statics::cost_subject_text;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::{Amount, Cmp, Condition, TallyEvent};
use mtg_core::card::CardType;

/// The spells a selector on the stack names: the kinds of spell, who cast or controls them and where they were cast from.
struct Spells {
    /// `Spell` first, then the kinds, then `ControlledBy(You)` for a spell the player cast.
    kinds: Selector,
    /// The spell is named by who cast it ("you cast"), else by who controls it.
    cast: bool,
    /// Where the spells were cast from, when the wording says so.
    from: Option<ZoneKind>,
}

/// The spells of a selector whose cards are on the stack and belong to the player; `None` for any other selector.
fn spells(preds: &[Pred]) -> Option<Spells> {
    let mut stacked = false;
    let (mut yours, mut owned, mut cast, mut from) = (false, false, false, None);
    let mut kinds = vec![Pred::Spell];
    for pred in preds {
        match pred {
            Pred::In(ZoneKind::Stack) if !stacked => stacked = true,
            Pred::ControlledBy(Ref::You) if !yours => yours = true,
            Pred::OwnedBy(Ref::You) if !owned => owned = true,
            Pred::WasCast { by, from: place } if from.is_none() || place.is_none() => {
                cast = true;
                yours |= *by == Some(Ref::You);
                if let Some(place) = place {
                    from = Some(*place);
                }
                (*by == Some(Ref::You) || by.is_none()).then_some(())?;
            }
            Pred::In(_) | Pred::ControlledBy(_) | Pred::OwnedBy(_) | Pred::WasCast { .. } => {
                return None;
            }
            other => kinds.push(other.clone()),
        }
    }
    // "From your hand" is the cards of the player's own hand; "from exile" any exile.
    let owner_ok = match from {
        Some(ZoneKind::Hand) => owned,
        Some(ZoneKind::Exile) => !owned,
        Some(_) => false,
        None => !owned,
    };
    (stacked && yours && owner_ok).then_some(())?;
    Some(Spells {
        kinds: Selector(kinds),
        cast,
        from,
    })
}

/// The kinds of a spell group with the plural noun: "artifact spells", "instant and sorcery spells you cast".
fn group(found: &Spells, cast: bool) -> Option<String> {
    let place = match found.from {
        Some(ZoneKind::Hand) => " from your hand",
        Some(ZoneKind::Exile) => " from exile",
        _ => "",
    };
    if cast {
        let mut kinds = found.kinds.clone();
        kinds.0.push(Pred::ControlledBy(Ref::You));
        let text = cost_subject_text(&kinds)?;
        // "Spells you cast with mana value 6 or greater": the number comes after who cast them.
        return Some(match text.split_once(" with ") {
            Some((head, number)) => {
                let number = number.strip_suffix(" you cast")?;
                format!("{head} you cast{place} with {number}")
            }
            None => format!("{text}{place}"),
        });
    }
    (found.from.is_none()).then_some(())?;
    phrase_text(&found.kinds, Form::All).map(|text| format!("{text} you control"))
}

/// "Creatures blocking or blocked by this creature": the creatures in a fight with the card itself.
pub(super) fn blockers(selector: &Selector) -> Vec<(String, bool)> {
    let fighting = Pred::AnyOf(vec![Pred::BlockedBy(Ref::This), Pred::Blocks(Ref::This)]);
    match selector.0.as_slice() {
        [Pred::Type(CardType::Creature), pred] if *pred == fighting => {
            vec![(
                "Creatures blocking or blocked by this creature".to_owned(),
                true,
            )]
        }
        _ => Vec::new(),
    }
}

/// "Each noncreature spell you cast": the group in the singular.
fn each(text: &str) -> String {
    format!("Each {}", text.replacen(" spells", " spell", 1))
}

/// The subjects of a static ability over the spells a selector names, each with whether it is plural.
pub(super) fn subjects(selector: &Selector) -> Vec<(String, bool)> {
    let Some(found) = spells(&selector.0) else {
        return Vec::new();
    };
    let Some(text) = group(&found, found.cast) else {
        return Vec::new();
    };
    let singular = (found.kinds.0.len() > 1 || found.cast).then(|| (each(&text), false));
    std::iter::once((capitalize(&text), true))
        .chain(singular)
        .collect()
}

/// "The first historic spell you cast each turn has convoke": the spell is the first of its kind the player cast this turn,
/// when the condition counts the spells of that same kind cast before it.
pub(super) fn first_subjects(selector: &Selector, condition: &Condition) -> Vec<(String, bool)> {
    let Condition::Compare {
        left:
            Amount::Tally {
                event: TallyEvent::SpellCast,
                who: Ref::EachPlayer,
                filter,
            },
        op: Cmp::Eq,
        right: Amount::Fixed(0),
    } = condition
    else {
        return Vec::new();
    };
    let Some(found) = spells(&selector.0).filter(|found| found.cast || found.from.is_none()) else {
        return Vec::new();
    };
    // The filter is the spell's own kinds, who cast it and where from, and not the spell itself.
    let mut same: Vec<Pred> = selector
        .0
        .iter()
        .filter(|pred| **pred != Pred::In(ZoneKind::Stack))
        .cloned()
        .collect();
    same.push(Pred::Not(Box::new(Pred::Is(Ref::Affected))));
    if filter.0 != same {
        return Vec::new();
    }
    let Some(text) = group(&found, true) else {
        return Vec::new();
    };
    // "The first spell you cast each turn", "The first instant or sorcery spell you cast each turn".
    let Some((noun, cast)) = text.split_once(" you cast") else {
        return Vec::new();
    };
    let Some(noun) = noun.strip_suffix(" spells") else {
        return if noun == "spells" {
            vec![(format!("The first spell you cast{cast} each turn"), false)]
        } else {
            Vec::new()
        };
    };
    let kinds = noun.replace(" and ", " or ");
    vec![(
        format!("The first {kinds} spell you cast{cast} each turn"),
        false,
    )]
}
