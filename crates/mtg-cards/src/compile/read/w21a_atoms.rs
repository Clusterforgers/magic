//! The open atoms of the shared noun-phrase reader and writer (`selectors::phrase` and `phrase_text`): adjectives and
//! suffixes beyond the reader's own words, consulted only after them, so every phrase read before reads the same.
//!
//! "modified", "face-down" and "noncommander" before the noun; after it "that targets you or a creature you control",
//! "that were put into your graveyard from the battlefield this turn", "with no counters on it", "of the chosen type" and
//! a stat compared with any amount ("with mana value less than or equal to the number of lands you control", "with power
//! greater than ~'s power").

use super::amounts::stat_name;
use super::defining;
use super::selectors::{Form, phrase, stat_word};
use super::tokens::Reader;
use crate::refs::{Pred, Ref, Selector, ZoneKind};

/// "that were put into your graveyard from the battlefield this turn" ("that was" for one, "a graveyard" for any): the
/// zone, its owner and `GravedThisTurn`.
pub(super) fn w21a_graved(r: &mut Reader<'_>) -> Option<Vec<Pred>> {
    r.need("that")?;
    (r.word("were") || r.word("was")).then_some(())?;
    r.need("put into")?;
    let mut preds = vec![Pred::In(ZoneKind::Graveyard)];
    if r.words("your graveyard") {
        preds.push(Pred::OwnedBy(Ref::You));
    } else {
        r.need("a graveyard")?;
    }
    r.need("from the battlefield this turn")?;
    preds.push(Pred::GravedThisTurn);
    Some(preds)
}

/// A cost reduction sentence, and for a reduction of colored mana alone also the sentence with the rule it follows said
/// after it (CR 601.2f: "This effect reduces only the amount of colored mana you pay.").
pub(super) fn w21a_colored_only(
    sentence: String,
    change: crate::ability::CostChange,
    mana: &crate::mana::ManaCostX,
) -> Vec<String> {
    let symbols = String::from(mana.clone());
    let colored = mana.x() == 0
        && !symbols.is_empty()
        && !symbols.chars().any(|c| c.is_ascii_digit() || c == 'C');
    let reminder = (change == crate::ability::CostChange::Reduce && colored).then(|| {
        format!("{sentence}. This effect reduces only the amount of colored mana you pay")
    });
    std::iter::once(sentence).chain(reminder).collect()
}

/// The suffixes and the zone of a phrase as `phrase_text` says them last: `GravedThisTurn` turns the zone into
/// "that were put into your graveyard from the battlefield this turn"; `None` when it names no zone it can say so.
pub(super) fn w21a_place(
    suffixes: Vec<String>,
    zone: Option<String>,
    plural: bool,
) -> Option<(Vec<String>, Option<String>)> {
    let (graved, suffixes): (Vec<String>, Vec<String>) = suffixes
        .into_iter()
        .partition(|suffix| suffix == W21A_GRAVED);
    if graved.is_empty() {
        return Some((suffixes, zone));
    }
    let place = w21a_graved_text(zone.as_deref(), plural)?;
    Some((suffixes, Some(place)))
}

/// The tally a condition counts, as the turn's facts say it: "a card left your graveyard this turn".
const W21A_LEFT_GRAVEYARD: &str = "a card left your graveyard this turn";

/// "a card left your graveyard this turn": at least one card left the controller's graveyard this turn.
pub(super) fn w21a_condition(r: &mut Reader<'_>) -> Option<crate::value::Condition> {
    (r.words(W21A_LEFT_GRAVEYARD) || r.words("one or more cards left your graveyard this turn"))
        .then_some(crate::value::Condition::Compare {
            left: Amount::Tallied {
                who: Ref::You,
                what: crate::value::TurnTally::CardsLeftGraveyard,
            },
            op: Cmp::Ge,
            right: Amount::Fixed(1),
        })
}

/// The condition `w21a_condition` reads, written back.
pub(super) fn w21a_condition_text(left: &Amount, op: Cmp, number: u32) -> Option<String> {
    let counted = Amount::Tallied {
        who: Ref::You,
        what: crate::value::TurnTally::CardsLeftGraveyard,
    };
    (*left == counted && op == Cmp::Ge && number == 1).then(|| W21A_LEFT_GRAVEYARD.to_owned())
}

/// The zone of `w21a_graved` as it is said, from the zone the writer gave the card ("from your graveyard").
pub(super) fn w21a_graved_text(zone: Option<&str>, plural: bool) -> Option<String> {
    let whose = match zone? {
        "from your graveyard" => "your",
        "from a graveyard" => "a",
        _ => return None,
    };
    let verb = if plural { "were" } else { "was" };
    Some(format!(
        "that {verb} put into {whose} graveyard from the battlefield this turn"
    ))
}
use crate::value::{Amount, Cmp};

/// The comparisons a stat is said to have with an amount, longest first.
const W21A_COMPARISONS: [(&str, Cmp); 5] = [
    ("less than or equal to", Cmp::Le),
    ("greater than or equal to", Cmp::Ge),
    ("less than", Cmp::Lt),
    ("greater than", Cmp::Gt),
    ("equal to", Cmp::Eq),
];

/// The marker the writer gives `GravedThisTurn`, which `phrase_text` turns into the zone it is said with.
pub(super) const W21A_GRAVED: &str = "put into a graveyard from the battlefield this turn";

/// An adjective of this module before the noun.
pub(super) fn w21a_adjective(word: &str) -> Option<Pred> {
    Some(match word {
        "modified" => Pred::Modified,
        "face-down" => Pred::FaceDown,
        "noncommander" => Pred::Not(Box::new(Pred::Commander)),
        _ => return None,
    })
}

/// The adjective of this module for a predicate.
pub(super) fn w21a_adjective_text(pred: &Pred) -> Option<String> {
    Some(
        match pred {
            Pred::Modified => "modified",
            Pred::FaceDown => "face-down",
            _ => return None,
        }
        .to_owned(),
    )
}

/// A suffix of this module after the noun.
pub(super) fn w21a_suffix(r: &mut Reader<'_>) -> Option<Pred> {
    if r.words("of the chosen type") {
        return Some(Pred::ChosenSubtype);
    }
    if r.words("of the chosen color") {
        return Some(Pred::ChosenColor);
    }
    if let Some(pred) = r.attempt(|r| w21a_targets(r)) {
        return Some(pred);
    }
    r.attempt(|r| {
        r.need("with")?;
        if r.words("no counters on it") || r.words("no counters on them") {
            return Some(Pred::Not(Box::new(Pred::HasAnyCounter)));
        }
        let stat = stat_word(r)?;
        let op = W21A_COMPARISONS
            .iter()
            .find_map(|(words, op)| r.words(words).then_some(*op))?;
        let to = w21a_amount(r)?;
        Some(Pred::Compare { stat, op, to })
    })
}

/// "that targets you", "that targets a creature you control", "that targets you or a creature you control".
fn w21a_targets(r: &mut Reader<'_>) -> Option<Pred> {
    r.need("that targets")?;
    let you = r.word("you");
    if you && r.peek_word() != Some("or") {
        return Some(Pred::TargetsPlayer(Ref::You));
    }
    if you {
        r.need("or")?;
    }
    (r.word("a") || r.word("an")).then_some(())?;
    let objects = Pred::Targeting(phrase(r, Form::Subject)?);
    Some(if you {
        Pred::AnyOf(vec![Pred::TargetsPlayer(Ref::You), objects])
    } else {
        objects
    })
}

/// The amount a stat is compared with: the source's own stat ("~'s power"), the triggering creature's ("that creature's
/// power") or a count the shared amount reader reads.
fn w21a_amount(r: &mut Reader<'_>) -> Option<Amount> {
    if let Some(stat) = r.attempt(|r| {
        r.source().then_some(())?;
        (r.punct('\'') && r.word("s")).then_some(())?;
        stat_word(r)
    }) {
        return Some(Amount::Stat {
            of: Ref::This,
            stat,
        });
    }
    if r.triggered
        && let Some(stat) = r.attempt(|r| {
            r.need("that creature's")?;
            stat_word(r)
        })
    {
        return Some(Amount::Stat {
            of: Ref::Triggering,
            stat,
        });
    }
    defining::value(r)
}

/// The amount as `w21a_amount` reads it.
fn w21a_amount_text(amount: &Amount) -> Option<String> {
    match amount {
        Amount::Stat {
            of: Ref::This,
            stat,
        } => Some(format!("~'s {}", stat_name(*stat)?)),
        Amount::Stat {
            of: Ref::Triggering,
            stat,
        } => Some(format!("that creature's {}", stat_name(*stat)?)),
        Amount::Fixed(_) | Amount::X => None,
        amount => defining::value_text(amount),
    }
}

/// The suffix of this module for a predicate.
pub(super) fn w21a_suffix_text(pred: &Pred, form: Form) -> Option<String> {
    Some(match pred {
        Pred::ChosenSubtype => "of the chosen type".to_owned(),
        Pred::ChosenColor => "of the chosen color".to_owned(),
        Pred::GravedThisTurn => W21A_GRAVED.to_owned(),
        Pred::TargetsPlayer(Ref::You) => "that targets you".to_owned(),
        Pred::Targeting(objects) => format!("that targets {}", w21a_a(objects)?),
        Pred::AnyOf(any) => match any.as_slice() {
            [Pred::TargetsPlayer(Ref::You), Pred::Targeting(objects)] => {
                format!("that targets you or {}", w21a_a(objects)?)
            }
            _ => return None,
        },
        Pred::Not(inner) if **inner == Pred::HasAnyCounter => {
            if form.plural() {
                "with no counters on them".to_owned()
            } else {
                "with no counters on it".to_owned()
            }
        }
        Pred::Compare { stat, op, to } => {
            let (words, _) = W21A_COMPARISONS.iter().find(|(_, held)| held == op)?;
            let name = stat_name(*stat)?;
            format!("with {name} {words} {}", w21a_amount_text(to)?)
        }
        _ => return None,
    })
}

/// "a creature you control": a singular phrase with its article.
fn w21a_a(objects: &Selector) -> Option<String> {
    let text = super::selectors::phrase_text(objects, Form::Subject)?;
    let article = if text.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    Some(format!("{article} {text}"))
}
