//! Characteristic-defining abilities of power and toughness (CR 604.3): "~'s power and toughness are each equal to the
//! number of lands you control.", "~'s power is equal to the number of cards in your hand.", "~'s power is equal to the
//! number of card types among cards in all graveyards and its toughness is equal to that number plus 1." They are a
//! `Static` of the card itself with `defining` and one `SetPT`; a stat the ability leaves out is the number printed on
//! the card.

use mtg_core::color::Color;

use super::selectors::{Form, color, color_name, phrase, phrase_in, phrase_text, stat_word};
use super::tokens::Reader;
use crate::ability::Ability;
use crate::modify::Modification;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::{Amount, StatKind};

/// "~'s power and toughness are each equal to ...", "~'s power is equal to ... [and its toughness is equal to ...]",
/// "~'s toughness is equal to ...".
pub(super) fn ability(r: &mut Reader<'_>) -> Option<Ability> {
    r.source().then_some(())?;
    r.punct('\'').then_some(())?;
    r.word("s").then_some(())?;
    let (power, toughness) = if r.words("power and toughness are each equal to") {
        let both = value(r)?;
        (both.clone(), both)
    } else if r.words("power is equal to") {
        let power = value(r)?;
        let toughness = if r.words("and its toughness is equal to") {
            let after = r.attempt(|r| {
                r.need("that number plus")?;
                let more = i32::try_from(r.number()?).ok()?;
                Some(Amount::Sum(vec![power.clone(), Amount::Fixed(more)]))
            });
            after.or_else(|| value(r))?
        } else {
            Amount::Fixed(r.toughness?)
        };
        (power, toughness)
    } else if r.words("toughness is equal to") {
        let toughness = value(r)?;
        (Amount::Fixed(r.power?), toughness)
    } else {
        return None;
    };
    Some(Ability::Static {
        affected: Ref::This,
        mods: vec![Modification::SetPT { power, toughness }],
        condition: None,
        defining: true,
        from: ZoneKind::Battlefield,
    })
}

/// A number the ability counts: parts joined by "plus", each "twice" a count, a count or a plain number.
fn value(r: &mut Reader<'_>) -> Option<Amount> {
    let mut parts = vec![part(r)?];
    while r.words("plus") {
        parts.push(part(r)?);
    }
    // A plain number alone defines nothing worth a count; it is the "plus 1" of a sum.
    if parts.iter().all(|part| matches!(part, Amount::Fixed(_))) {
        return None;
    }
    if parts.len() == 1 {
        parts.pop()
    } else {
        Some(Amount::Sum(parts))
    }
}

fn part(r: &mut Reader<'_>) -> Option<Amount> {
    if let Some(number) = r.number() {
        return Some(Amount::Fixed(i32::try_from(number).ok()?));
    }
    if r.word("twice") {
        let count = counting(r)?;
        return Some(Amount::Sum(vec![count.clone(), count]));
    }
    counting(r)
}

/// What is counted: "the number of ...", "the number of card types among ...", "your life total", "your devotion to
/// black", "the greatest mana value among ...", "the total power of ...".
pub(super) fn counting(r: &mut Reader<'_>) -> Option<Amount> {
    if r.words("your life total") {
        return Some(Amount::Life(Ref::You));
    }
    if r.words("the number of card types among") {
        return Some(Amount::CardTypes(counted(r)?));
    }
    if r.words("the number of") {
        return Some(Amount::Count(counted(r)?));
    }
    if r.words("the greatest") {
        let stat = stat_word(r)?;
        r.need("among")?;
        return Some(Amount::Greatest {
            of: counted(r)?,
            stat,
        });
    }
    if r.words("the total") {
        let stat = stat_word(r)?;
        r.need("of")?;
        return Some(Amount::Total {
            of: counted(r)?,
            stat,
        });
    }
    if r.words("your devotion to") {
        let mut colors = vec![color(r.take_word()?)?];
        while r.word("and") {
            colors.push(color(r.take_word()?)?);
        }
        return Some(Amount::Devotion(colors));
    }
    None
}

/// The objects a count looks at: "creatures you control", "Forests on the battlefield", "cards in your hand", "creature
/// cards in all graveyards".
fn counted(r: &mut Reader<'_>) -> Option<Selector> {
    if let Some(Selector(preds)) = r.attempt(|r| {
        let selector = phrase(r, Form::All)?;
        r.words("on the battlefield");
        Some(selector)
    }) {
        return Some(Selector(preds));
    }
    r.attempt(|r| {
        let Selector(mut preds) = phrase_in(r, Form::All, true)?;
        preds.extend(in_zone(r)?);
        Some(Selector(preds))
    })
}

/// "in your hand", "in your graveyard", "in all graveyards", "in your opponents' graveyards".
fn in_zone(r: &mut Reader<'_>) -> Option<Vec<Pred>> {
    if r.words("in your hand") {
        return Some(vec![Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)]);
    }
    if r.words("in your graveyard") {
        return Some(vec![Pred::In(ZoneKind::Graveyard), Pred::OwnedBy(Ref::You)]);
    }
    if r.words("in all graveyards") {
        return Some(vec![Pred::In(ZoneKind::Graveyard)]);
    }
    r.attempt(|r| {
        r.need("in your opponents")?;
        r.punct('\'').then_some(())?;
        r.word("graveyards").then_some(())?;
        Some(vec![
            Pred::In(ZoneKind::Graveyard),
            Pred::OwnedBy(Ref::Opponents),
        ])
    })
}

/// The sentence of a defining ability of power and toughness, without its period; empty for any other ability.
pub(super) fn sentences(ability: &Ability) -> Vec<String> {
    let Ability::Static {
        affected: Ref::This,
        mods,
        condition: None,
        defining: true,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    let [Modification::SetPT { power, toughness }] = mods.as_slice() else {
        return Vec::new();
    };
    sentence(power, toughness).into_iter().collect()
}

fn sentence(power: &Amount, toughness: &Amount) -> Option<String> {
    if power == toughness {
        let both = value_text(power)?;
        return Some(format!("~'s power and toughness are each equal to {both}"));
    }
    if matches!(toughness, Amount::Fixed(_)) {
        return Some(format!("~'s power is equal to {}", value_text(power)?));
    }
    if matches!(power, Amount::Fixed(_)) {
        return Some(format!(
            "~'s toughness is equal to {}",
            value_text(toughness)?
        ));
    }
    let after = match toughness {
        Amount::Sum(parts) => match parts.as_slice() {
            [first, Amount::Fixed(more)] if first == power && *more > 0 => {
                Some(format!("that number plus {more}"))
            }
            _ => None,
        },
        _ => None,
    };
    let toughness = match after {
        Some(text) => text,
        None => value_text(toughness)?,
    };
    Some(format!(
        "~'s power is equal to {} and its toughness is equal to {toughness}",
        value_text(power)?
    ))
}

fn value_text(amount: &Amount) -> Option<String> {
    let Amount::Sum(parts) = amount else {
        return counting_text(amount);
    };
    if let [first, second] = parts.as_slice()
        && first == second
        && !matches!(first, Amount::Fixed(_))
    {
        return Some(format!("twice {}", counting_text(first)?));
    }
    let texts: Option<Vec<String>> = parts
        .iter()
        .map(|part| match part {
            Amount::Fixed(number) if *number > 0 => Some(number.to_string()),
            other => counting_text(other),
        })
        .collect();
    let texts = texts?;
    (texts.len() > 1 && !parts.iter().all(|part| matches!(part, Amount::Fixed(_))))
        .then(|| texts.join(" plus "))
}

pub(super) fn counting_text(amount: &Amount) -> Option<String> {
    match amount {
        Amount::Life(Ref::You) => Some("your life total".to_owned()),
        Amount::CardTypes(of) => Some(format!(
            "the number of card types among {}",
            counted_text(of)?
        )),
        Amount::Count(of) => Some(format!("the number of {}", counted_text(of)?)),
        Amount::Greatest { of, stat } => Some(format!(
            "the greatest {} among {}",
            stat_text(*stat)?,
            counted_text(of)?
        )),
        Amount::Total { of, stat } => Some(format!(
            "the total {} of {}",
            stat_text(*stat)?,
            counted_text(of)?
        )),
        Amount::Devotion(colors) if !colors.is_empty() => {
            let names: Vec<&str> = colors
                .iter()
                .map(|found: &Color| color_name(*found))
                .collect();
            Some(format!("your devotion to {}", names.join(" and ")))
        }
        _ => None,
    }
}

fn stat_text(stat: StatKind) -> Option<&'static str> {
    super::amounts::stat_name(stat)
}

/// What `counted` reads, written back.
fn counted_text(selector: &Selector) -> Option<String> {
    let preds = &selector.0;
    let Some(at) = preds.iter().position(|pred| matches!(pred, Pred::In(_))) else {
        let text = phrase_text(selector, Form::All)?;
        let placed = preds
            .iter()
            .any(|pred| matches!(pred, Pred::ControlledBy(_) | Pred::OwnedBy(_)));
        return Some(if placed {
            text
        } else {
            format!("{text} on the battlefield")
        });
    };
    let Some(Pred::In(zone)) = preds.get(at) else {
        return None;
    };
    let words = match (zone, &preds[at + 1..]) {
        (ZoneKind::Hand, [Pred::OwnedBy(Ref::You)]) => " in your hand",
        (ZoneKind::Graveyard, [Pred::OwnedBy(Ref::You)]) => " in your graveyard",
        (ZoneKind::Graveyard, []) => " in all graveyards",
        (ZoneKind::Graveyard, [Pred::OwnedBy(Ref::Opponents)]) => " in your opponents' graveyards",
        _ => return None,
    };
    // The phrase writer words a card of your own zone; the zone words are replaced after it.
    let mut own_zone = preds[..at].to_vec();
    own_zone.extend([Pred::In(*zone), Pred::OwnedBy(Ref::You)]);
    let text = phrase_text(&Selector(own_zone), Form::All)?;
    let head = text
        .strip_suffix(" from your hand")
        .or_else(|| text.strip_suffix(" from your graveyard"))?;
    Some(format!("{head}{words}"))
}
