//! The mana spent to cast a spell (CR 601.2h): "if {B} was spent to cast it", "if {R} was spent to cast this spell" and
//! "Sacrifice it unless {G} was spent to cast it." read as `Condition::ManaSpent`, the last as a sacrifice that the
//! condition's absence causes.

use super::effects;
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::{Pred, Ref};
use crate::value::{Amount, Cmp, Condition, StatKind};
use mtg_core::mana::ManaType;

/// The mana a symbol names: `{G}` is green, `{C}` colorless.
fn mana_of(symbol: &str) -> Option<ManaType> {
    let mut letters = symbol.chars();
    let (letter, rest) = (letters.next()?, letters.next());
    if rest.is_some() {
        return None;
    }
    ManaType::ALL
        .into_iter()
        .find(|mana| mana.symbol() == letter)
}

/// "{G} was spent to cast it", or "{G} was spent to cast this spell" with `spell`; "{W}{W} was spent" is at least two
/// of that mana and "{C} wasn't spent" its absence.
pub fn was_spent(r: &mut Reader<'_>, spell: bool) -> Option<Condition> {
    let mana = mana_of(r.symbol()?)?;
    let mut least: i32 = 1;
    while let Some(more) = r.attempt(|r| mana_of(r.symbol()?)) {
        (more == mana).then_some(())?;
        least += 1;
    }
    let not = if r.word("wasn't") {
        (least == 1).then_some(true)?
    } else {
        r.word("was").then_some(false)?
    };
    r.need("spent to cast")?;
    if spell {
        r.source().then_some(())?;
    } else {
        r.word("it").then_some(())?;
    }
    let spent = if least == 1 {
        Condition::ManaSpent(mana)
    } else {
        Condition::Compare {
            left: Amount::ManaSpent {
                of: Some(mana),
                from: None,
            },
            op: Cmp::Ge,
            right: Amount::Fixed(least),
        }
    };
    Some(if not {
        Condition::Not(Box::new(spent))
    } else {
        spent
    })
}

/// The condition `was_spent` reads, written back for the same `spell`.
pub fn was_spent_text(condition: &Condition, spell: bool) -> Option<String> {
    let (mana, least, verb) = match condition {
        Condition::ManaSpent(mana) => (*mana, 1, "was"),
        Condition::Not(inner) => match inner.as_ref() {
            Condition::ManaSpent(mana) => (*mana, 1, "wasn't"),
            _ => return None,
        },
        Condition::Compare {
            left:
                Amount::ManaSpent {
                    of: Some(mana),
                    from: None,
                },
            op: Cmp::Ge,
            right: Amount::Fixed(least),
        } if (2..=5).contains(least) => (*mana, usize::try_from(*least).ok()?, "was"),
        _ => return None,
    };
    let symbols = format!("{{{}}}", mana.symbol()).repeat(least);
    Some(format!(
        "{symbols} {verb} spent to cast {}",
        if spell { "this spell" } else { "it" }
    ))
}

/// How much mana was spent to cast the spell of the event, as a cast trigger's condition: "at least four mana was spent to
/// cast it", "no mana was spent to cast that spell".
pub fn amount_spent(r: &mut Reader<'_>) -> Option<Condition> {
    let (op, right) = if r.word("no") {
        (Cmp::Eq, 0)
    } else {
        r.words("at least").then_some(())?;
        let number = super::amounts::number_word(r.peek_word()?)?;
        r.skip();
        (Cmp::Ge, i32::try_from(number).ok()?)
    };
    r.need("mana was spent to cast")?;
    (r.word("it") || r.words("that spell")).then_some(())?;
    Some(Condition::Compare {
        left: spent_on_the_cast(),
        op,
        right: Amount::Fixed(right),
    })
}

/// The condition `amount_spent` reads, as the card words it.
pub fn amount_spent_text(condition: &Condition) -> Option<String> {
    let Condition::Compare {
        left,
        op,
        right: Amount::Fixed(right),
    } = condition
    else {
        return None;
    };
    if *left != spent_on_the_cast() {
        return None;
    }
    let amount = match (op, *right) {
        (Cmp::Eq, 0) => "no".to_owned(),
        (Cmp::Ge, right) => format!(
            "at least {}",
            super::amounts::count_word(u32::try_from(right).ok()?)?
        ),
        _ => return None,
    };
    Some(format!("{amount} mana was spent to cast it"))
}

/// The mana spent to cast the spell of the event.
fn spent_on_the_cast() -> Amount {
    Amount::Stat {
        of: Ref::Triggering,
        stat: StatKind::ManaSpent,
    }
}

/// "Sacrifice it unless {G} was spent to cast it.": the card is sacrificed when that mana was not spent.
pub fn sacrifice_unless(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let sacrifice = effects::sacrifice_itself(r)?;
    r.need("unless")?;
    let spent = was_spent(r, false)?;
    Some(vec![Effect::If {
        condition: Condition::Not(Box::new(spent)),
        then: vec![sacrifice],
        otherwise: Vec::new(),
    }])
}

/// The sentence of an `If` that sacrifices the card unless mana was spent, as `sacrifice_unless` reads it.
pub fn unless_text(effect: &Effect) -> Option<String> {
    let Effect::If {
        condition: Condition::Not(spent),
        then,
        otherwise,
    } = effect
    else {
        return None;
    };
    let [
        Effect::Sacrifice {
            who: Ref::You,
            filter,
            count,
            ..
        },
    ] = then.as_slice()
    else {
        return None;
    };
    if !otherwise.is_empty() || filter.0 != [Pred::This] || *count != Amount::one() {
        return None;
    }
    Some(format!(
        "sacrifice ~ unless {}",
        was_spent_text(spent, false)?
    ))
}
