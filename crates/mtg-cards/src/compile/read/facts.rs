//! Conditions on how the game stands: what happened this turn ("you attacked with three or more creatures this turn", "an
//! opponent was dealt 7 or more damage this turn"), the total power of a group, the hands of every player and the cards
//! exiled with the card. Read from the wording and written back the same way.

use mtg_core::card::CardType;

use super::amounts::{count_word, number_word};
use super::tokens::Reader;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::{Amount, Cmp, Condition, StatKind, TurnTally};

/// A number written as digits or as a word.
pub(super) fn quantity(r: &mut Reader<'_>) -> Option<u32> {
    r.number()
        .or_else(|| r.attempt(|r| number_word(r.take_word()?)))
}

/// The ways a card writes a number: a word up to ten and digits beyond, then the other way ("twenty or more", "10 or
/// greater").
pub(super) fn quantity_texts(number: u32) -> Vec<String> {
    let word = count_word(number).map(str::to_owned);
    let digits = number.to_string();
    match word {
        Some(word) if number <= 10 => vec![word, digits],
        Some(word) => vec![digits, word],
        None => vec![digits],
    }
}

/// "you attacked with three or more creatures this turn", "an opponent was dealt 7 or more damage this turn",
/// "creatures you control have total power 10 or greater", "each player has no cards in hand", "there are cards exiled
/// with it".
pub(super) fn read(r: &mut Reader<'_>) -> Option<Condition> {
    r.attempt(this_turn)
        .or_else(|| r.attempt(total_power))
        .or_else(|| r.attempt(empty_hands))
        .or_else(|| r.attempt(exiled_with_it))
        .or_else(|| r.attempt(counter_this_turn))
}

/// The cards exiled with the card, as hideaway links them.
fn exiled_with_this() -> Selector {
    Selector(vec![Pred::In(ZoneKind::Exile), Pred::ExiledWith(Ref::This)])
}

/// The creatures a player controls.
fn creatures_you_control() -> Selector {
    Selector(vec![
        Pred::Type(CardType::Creature),
        Pred::ControlledBy(Ref::You),
    ])
}

/// Every card in a hand, whoever holds it.
fn every_hand() -> Selector {
    Selector(vec![Pred::In(ZoneKind::Hand)])
}

fn at_least(left: Amount, number: u32) -> Option<Condition> {
    Some(Condition::Compare {
        left,
        op: Cmp::Ge,
        right: Amount::Fixed(i32::try_from(number).ok()?),
    })
}

/// "you attacked with three or more creatures this turn", "an opponent was dealt 7 or more damage this turn".
fn this_turn(r: &mut Reader<'_>) -> Option<Condition> {
    if r.words("you gained life this turn") {
        return at_least(
            Amount::Tallied {
                who: Ref::You,
                what: TurnTally::LifeGained,
            },
            1,
        );
    }
    if r.words("you gained") {
        // Life is counted in digits ("4 or more life"), the creatures of the next wording in words.
        let number = r.number().filter(|number| *number > 1)?;
        r.need("or more life this turn")?;
        return at_least(
            Amount::Tallied {
                who: Ref::You,
                what: TurnTally::LifeGainedTotal,
            },
            number,
        );
    }
    if r.words("you attacked with") {
        let number = quantity(r).filter(|number| *number > 1)?;
        r.need("or more creatures this turn")?;
        return at_least(
            Amount::Tallied {
                who: Ref::You,
                what: TurnTally::Attackers,
            },
            number,
        );
    }
    r.need("an opponent was dealt")?;
    let number = quantity(r)?;
    r.need("or more damage this turn")?;
    at_least(
        Amount::Tallied {
            who: Ref::Opponents,
            what: TurnTally::DamageTaken,
        },
        number,
    )
}

/// "creatures you control have total power 10 or greater".
fn total_power(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("creatures you control have total power")?;
    let number = quantity(r)?;
    r.need("or greater")?;
    at_least(
        Amount::Total {
            of: creatures_you_control(),
            stat: StatKind::Power,
        },
        number,
    )
}

/// "each player has no cards in hand".
fn empty_hands(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("each player has no cards in hand")?;
    Some(Condition::Compare {
        left: Amount::Count(every_hand()),
        op: Cmp::Eq,
        right: Amount::Fixed(0),
    })
}

/// "there are cards exiled with it" (also "with ~"): hideaway or another effect linked a card to this one.
fn exiled_with_it(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("there are cards exiled with")?;
    (r.word("it") || r.source()).then_some(())?;
    at_least(Amount::Count(exiled_with_this()), 1)
}

/// "a counter was put on ~ this turn" (also "on it"): counters of any kind put on the card itself.
fn counter_this_turn(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("a counter was put on")?;
    (r.word("it") || r.source()).then_some(())?;
    r.need("this turn")?;
    at_least(
        Amount::Tallied {
            who: Ref::This,
            what: TurnTally::CountersPut,
        },
        1,
    )
}

/// What `read` reads, written back after "if" or "as long as", one text for each wording.
pub(super) fn texts(condition: &Condition) -> Vec<String> {
    let Condition::Compare { left, op, right } = condition else {
        return Vec::new();
    };
    let Amount::Fixed(number) = right else {
        return Vec::new();
    };
    let Ok(number) = u32::try_from(*number) else {
        return Vec::new();
    };
    let counted = |pattern: &dyn Fn(&str) -> String| -> Vec<String> {
        quantity_texts(number)
            .iter()
            .map(|words| pattern(words))
            .collect()
    };
    match (left, op) {
        (
            Amount::Tallied {
                who: Ref::You,
                what: TurnTally::Attackers,
            },
            Cmp::Ge,
        ) if number > 1 => {
            counted(&|n| format!("you attacked with {n} or more creatures this turn"))
        }
        (
            Amount::Tallied {
                who: Ref::You,
                what: TurnTally::LifeGained,
            },
            Cmp::Ge,
        ) if number == 1 => vec!["you gained life this turn".to_owned()],
        (
            Amount::Tallied {
                who: Ref::You,
                what: TurnTally::LifeGainedTotal,
            },
            Cmp::Ge,
        ) if number > 1 => counted(&|n| format!("you gained {n} or more life this turn")),
        (
            Amount::Tallied {
                who: Ref::EachPlayer,
                what: TurnTally::CreaturesDied,
            },
            Cmp::Ge,
        ) if number == 1 => vec!["a creature died this turn".to_owned()],
        (
            Amount::Tallied {
                who: Ref::Opponents,
                what: TurnTally::DamageTaken,
            },
            Cmp::Ge,
        ) => counted(&|n| format!("an opponent was dealt {n} or more damage this turn")),
        (
            Amount::Total {
                of,
                stat: StatKind::Power,
            },
            Cmp::Ge,
        ) if *of == creatures_you_control() => {
            counted(&|n| format!("creatures you control have total power {n} or greater"))
        }
        (Amount::Count(of), Cmp::Eq) if number == 0 && *of == every_hand() => {
            vec!["each player has no cards in hand".to_owned()]
        }
        (
            Amount::Tallied {
                who: Ref::This,
                what: TurnTally::CountersPut,
            },
            Cmp::Ge,
        ) if number == 1 => vec![
            "a counter was put on ~ this turn".to_owned(),
            "a counter was put on it this turn".to_owned(),
        ],
        (Amount::Count(of), Cmp::Ge) if number == 1 && *of == exiled_with_this() => vec![
            "there are cards exiled with it".to_owned(),
            "there are cards exiled with ~".to_owned(),
        ],
        _ => Vec::new(),
    }
}
