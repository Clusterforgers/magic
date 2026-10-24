//! Characteristic-defining abilities of power and toughness (CR 604.3): "~'s power and toughness are each equal to the
//! number of lands you control.", "~'s power is equal to the number of cards in your hand.", "~'s power is equal to the
//! number of card types among cards in all graveyards and its toughness is equal to that number plus 1." They are a
//! `Static` of the card itself with `defining` and one `SetPT`; a stat the ability leaves out is the number printed on
//! the card.

use mtg_core::color::Color;

use super::counters;
use super::selectors::{Form, color, color_name, phrase, phrase_in, phrase_text, stat_word};
use super::tokens::Reader;
use super::where_x;
use crate::ability::Ability;
use crate::modify::Modification;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::{Amount, StatKind, TurnTally};

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

/// "equal to the number of opponents you have": the count an amount is.
pub(super) fn equal_to(r: &mut Reader<'_>) -> Option<Amount> {
    r.need("equal to")?;
    value(r)
}

/// A number the ability counts: parts joined by "plus", each "twice" a count, a count or a plain number.
pub(super) fn value(r: &mut Reader<'_>) -> Option<Amount> {
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
    if let Some(stat) = where_x::paid_stat(r) {
        return Some(Amount::Stat {
            of: Ref::Paid,
            stat,
        });
    }
    if r.words("your life total") {
        return Some(Amount::Life(Ref::You));
    }
    if r.words("the amount of life you gained this turn") {
        return Some(Amount::Tallied {
            who: Ref::You,
            what: TurnTally::LifeGained,
        });
    }
    if r.words("the number of card types among") {
        return Some(Amount::CardTypes(counted(r)?));
    }
    if r.words("the total number of") {
        return Some(Amount::Count(counted(r)?));
    }
    if r.words("the number of") {
        if let Some(held) = r.attempt(held) {
            return Some(held);
        }
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

/// What "the number of" counts besides objects: "opponents you have", "charge counters on ~".
fn held(r: &mut Reader<'_>) -> Option<Amount> {
    if r.words("creatures in your party") {
        return Some(Amount::Party(Ref::You));
    }
    if let Some(died) = r.attempt(died) {
        return Some(died);
    }
    if r.words("opponents you have") {
        return Some(Amount::Players(Ref::Opponents));
    }
    if r.attempt(|r| {
        r.need("times")?;
        r.source().then_some(())?;
        r.need("has mutated")
    })
    .is_some()
    {
        return Some(Amount::Mutations(Ref::This));
    }
    counters::counted_plural(r)
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
    if let Some(hands) = r.attempt(|r| {
        r.need("in all players")?;
        r.punct('\'').then_some(())?;
        r.word("hands").then_some(())?;
        Some(vec![Pred::In(ZoneKind::Hand)])
    }) {
        return Some(hands);
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

pub(super) fn value_text(amount: &Amount) -> Option<String> {
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
        Amount::Tallied {
            who: Ref::You,
            what: TurnTally::LifeGained,
        } => Some("the amount of life you gained this turn".to_owned()),
        Amount::Tallied { .. } => {
            died_text(amount, false).map(|text| format!("the number of {text}"))
        }
        Amount::Life(Ref::You) => Some("your life total".to_owned()),
        Amount::CardTypes(of) => Some(format!(
            "the number of card types among {}",
            counted_text(of)?
        )),
        // The cards of every hand are the "total" number.
        Amount::Count(of) if of.0.last() == Some(&Pred::In(ZoneKind::Hand)) => {
            Some(format!("the total number of {}", counted_text(of)?))
        }
        Amount::Count(of) => Some(format!("the number of {}", counted_text(of)?)),
        Amount::Players(Ref::Opponents) => Some("the number of opponents you have".to_owned()),
        Amount::Party(Ref::You) => Some("the number of creatures in your party".to_owned()),
        Amount::Mutations(Ref::This) => {
            Some("the number of times this creature has mutated".to_owned())
        }
        Amount::Counters {
            on: Ref::This,
            counter,
        } => Some(format!(
            "the number of {} counters on ~",
            counters::kind_text(counter)?
        )),
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
        // An attacking or blocking creature is on the battlefield, and the card does not say so.
        let placed = preds.iter().any(|pred| {
            matches!(
                pred,
                Pred::ControlledBy(_) | Pred::OwnedBy(_) | Pred::Attacking | Pred::Blocking
            )
        });
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
        (ZoneKind::Graveyard, [Pred::OwnedBy(Ref::Target(0))]) => " in that player's graveyard",
        (ZoneKind::Graveyard, []) => " in all graveyards",
        (ZoneKind::Hand, []) => " in all players' hands",
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

/// "creatures that died this turn" (every player), "nontoken creatures put into your graveyard from the battlefield
/// this turn" (yours): the creatures the game counted as they left the battlefield, after "the number of".
fn died(r: &mut Reader<'_>) -> Option<Amount> {
    if r.words("cards you've drawn this turn") {
        return Some(drawn());
    }
    let what = if r.word("nontoken") {
        TurnTally::NontokenCreaturesDied
    } else {
        TurnTally::CreaturesDied
    };
    r.word("creatures").then_some(())?;
    if r.words("that died under your control this turn") {
        return Some(Amount::Tallied {
            who: Ref::You,
            what: under(what),
        });
    }
    let who = if r.words("that died this turn") {
        Ref::EachPlayer
    } else {
        r.need("put into your graveyard from the battlefield this turn")?;
        Ref::You
    };
    Some(Amount::Tallied { who, what })
}

/// The cards the player drew this turn.
fn drawn() -> Amount {
    Amount::Tallied {
        who: Ref::You,
        what: TurnTally::Drawn,
    }
}

/// The tally of the creatures that died while the player controlled them, in place of the one counted by owner.
const fn under(what: TurnTally) -> TurnTally {
    match what {
        TurnTally::NontokenCreaturesDied => TurnTally::NontokenCreaturesDiedUnder,
        _ => TurnTally::CreaturesDiedUnder,
    }
}

/// "creature put into your graveyard from the battlefield this turn" or "card you've drawn this turn", after "for each".
pub(super) fn died_each(r: &mut Reader<'_>) -> Option<Amount> {
    if r.words("card you've drawn this turn") {
        return Some(drawn());
    }
    let what = if r.word("nontoken") {
        TurnTally::NontokenCreaturesDied
    } else {
        TurnTally::CreaturesDied
    };
    r.word("creature").then_some(())?;
    if r.words("that died under your control this turn") {
        return Some(Amount::Tallied {
            who: Ref::You,
            what: under(what),
        });
    }
    r.need("put into your graveyard from the battlefield this turn")?;
    Some(Amount::Tallied {
        who: Ref::You,
        what,
    })
}

/// What `died` and `died_each` read, written back: the creatures after "the number of" or the one creature after "for each".
pub(super) fn died_text(amount: &Amount, each: bool) -> Option<String> {
    let Amount::Tallied { who, what } = amount else {
        return None;
    };
    if *what == TurnTally::Drawn {
        let noun = if each { "card" } else { "cards" };
        return (*who == Ref::You).then(|| format!("{noun} you've drawn this turn"));
    }
    let kind = match what {
        TurnTally::CreaturesDied | TurnTally::CreaturesDiedUnder => "",
        TurnTally::NontokenCreaturesDied | TurnTally::NontokenCreaturesDiedUnder => "nontoken ",
        _ => return None,
    };
    let noun = if each { "creature" } else { "creatures" };
    if matches!(
        what,
        TurnTally::CreaturesDiedUnder | TurnTally::NontokenCreaturesDiedUnder
    ) {
        return (*who == Ref::You)
            .then(|| format!("{kind}{noun} that died under your control this turn"));
    }
    match who {
        Ref::You => Some(format!(
            "{kind}{noun} put into your graveyard from the battlefield this turn"
        )),
        Ref::EachPlayer if !each => Some(format!("{kind}{noun} that died this turn")),
        _ => None,
    }
}
