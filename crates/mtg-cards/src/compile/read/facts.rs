//! Conditions on how the game stands: what happened this turn ("you attacked with three or more creatures this turn", "an
//! opponent was dealt 7 or more damage this turn"), the total power of a group, the hands of every player and the cards
//! exiled with the card. Read from the wording and written back the same way.

use mtg_core::card::CardType;

use super::amounts::{count_word, number_word};
use super::selectors::{Form, phrase_text};
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
        .or_else(|| r.attempt(died_under_you))
        .or_else(|| r.attempt(full_party))
        .or_else(|| r.attempt(super::c4_hands::read))
}

/// What `died_under_you` reads, written back.
fn died_under_texts(what: TurnTally, number: u32) -> Vec<String> {
    let kind = if what == TurnTally::CreaturesDiedUnder {
        ""
    } else {
        "nontoken "
    };
    if number == 1 {
        return vec![format!(
            "a {kind}creature died under your control this turn"
        )];
    }
    quantity_texts(number)
        .iter()
        .map(|n| format!("{n} or more {kind}creatures died under your control this turn"))
        .collect()
}

/// "a creature died under your control this turn", "two or more creatures died under your control this turn".
fn died_under_you(r: &mut Reader<'_>) -> Option<Condition> {
    let (number, noun) = if r.words("a nontoken creature") {
        (1, TurnTally::NontokenCreaturesDiedUnder)
    } else if r.words("a creature") {
        (1, TurnTally::CreaturesDiedUnder)
    } else {
        let number = quantity(r).filter(|number| *number > 1)?;
        r.need("or more")?;
        let nontoken = r.word("nontoken");
        r.need("creatures")?;
        let noun = if nontoken {
            TurnTally::NontokenCreaturesDiedUnder
        } else {
            TurnTally::CreaturesDiedUnder
        };
        (number, noun)
    };
    r.need("died under your control this turn")?;
    at_least(
        Amount::Tallied {
            who: Ref::You,
            what: noun,
        },
        number,
    )
}

/// "you have a full party": the party of four roles (CR 700.8) is complete.
fn full_party(r: &mut Reader<'_>) -> Option<Condition> {
    r.words("you have a full party").then_some(())?;
    at_least(Amount::Party(Ref::You), 4)
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

/// The words of a noncreature spell cast this turn.
const C10_ANY_NONCREATURE_CAST: &str = "you've cast a noncreature spell this turn";

/// The noncreature spells you cast this turn.
const fn c10_any_noncreature_cast() -> Amount {
    Amount::Tallied {
        who: Ref::You,
        what: TurnTally::NoncreatureSpellsCast,
    }
}

/// "you attacked with three or more creatures this turn", "an opponent was dealt 7 or more damage this turn".
fn this_turn(r: &mut Reader<'_>) -> Option<Condition> {
    if r.words(C10_ANY_NONCREATURE_CAST) {
        return at_least(c10_any_noncreature_cast(), 1);
    }
    if r.words("you gained life this turn") {
        return at_least(
            Amount::Tallied {
                who: Ref::You,
                what: TurnTally::LifeGained,
            },
            1,
        );
    }
    if r.words("you played a land this turn") {
        return at_least(
            Amount::Tallied {
                who: Ref::You,
                what: TurnTally::LandsPlayed,
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

/// The objects a player controls, written for "you" in the data, as the plural a card names them: "lands",
/// "nonbasic lands"; `None` when they are not the player's.
fn controlled_group(of: &Selector) -> Option<String> {
    let yours = Pred::ControlledBy(Ref::You);
    of.0.contains(&yours).then_some(())?;
    let group = Selector(
        of.0.iter()
            .filter(|pred| **pred != yours)
            .cloned()
            .collect(),
    );
    phrase_text(&group, Form::All)
}

/// What `read` reads, written back after "if" or "as long as", one text for each wording.
pub(super) fn texts(condition: &Condition) -> Vec<String> {
    let Condition::Compare { left, op, right } = condition else {
        return Vec::new();
    };
    let Amount::Fixed(number) = right else {
        return more_than_you(left, *op, right);
    };
    let Ok(number) = u32::try_from(*number) else {
        return Vec::new();
    };
    if *left == c10_any_noncreature_cast() && *op == Cmp::Ge && number == 1 {
        return vec![C10_ANY_NONCREATURE_CAST.to_owned()];
    }
    let counted = |pattern: &dyn Fn(&str) -> String| -> Vec<String> {
        quantity_texts(number)
            .iter()
            .map(|words| pattern(words))
            .collect()
    };
    match (left, op) {
        (Amount::Party(Ref::You), Cmp::Ge) if number == 4 => {
            vec!["you have a full party".to_owned()]
        }
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
                what: TurnTally::LandsPlayed,
            },
            Cmp::Ge,
        ) if number == 1 => vec!["you played a land this turn".to_owned()],
        (
            Amount::Tallied {
                who: Ref::You,
                what: TurnTally::LifeGainedTotal,
            },
            Cmp::Ge,
        ) if number > 1 => counted(&|n| format!("you gained {n} or more life this turn")),
        (
            Amount::Tallied {
                what:
                    TurnTally::CreaturesDied
                    | TurnTally::CreaturesDiedUnder
                    | TurnTally::NontokenCreaturesDiedUnder,
                ..
            },
            Cmp::Ge,
        ) => died_texts(left, number),
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
        // The largest number any one opponent controls: "an opponent controls four or more lands", never "your
        // opponents control", which is every opponent together.
        (
            Amount::Largest {
                who: Ref::Opponents,
                of,
            },
            Cmp::Ge,
        ) if number > 1 => match controlled_group(of) {
            Some(group) => counted(&|n| format!("an opponent controls {n} or more {group}")),
            None => Vec::new(),
        },
        _ => state_texts(left, *op, number),
    }
}

/// "an opponent controls more lands than you": the largest count among the opponents against the count you control.
fn more_than_you(left: &Amount, op: Cmp, right: &Amount) -> Vec<String> {
    match (left, op, right) {
        (
            Amount::Largest {
                who: Ref::Opponents,
                of,
            },
            Cmp::Gt,
            Amount::Count(yours),
        ) if of == yours => controlled_group(of)
            .map(|group| format!("an opponent controls more {group} than you"))
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

/// "each player has no cards in hand", "a counter was put on ~ this turn" and "there are cards exiled with it".
fn state_texts(left: &Amount, op: Cmp, number: u32) -> Vec<String> {
    match (left, op) {
        (
            Amount::Tallied {
                who: Ref::You,
                what: TurnTally::Attackers,
            },
            Cmp::Ge,
        ) if number == 1 => vec!["you attacked this turn".to_owned()],
        (
            Amount::Tallied {
                who: Ref::Opponents,
                what: TurnTally::LifeLost,
            },
            Cmp::Ge,
        ) if number > 1 => quantity_texts(number)
            .iter()
            .map(|n| format!("an opponent lost {n} or more life this turn"))
            .collect(),
        (Amount::Count(of), Cmp::Eq) if number == 0 && *of == every_hand() => {
            vec!["each player has no cards in hand".to_owned()]
        }
        (Amount::LowestHand(who), Cmp::Le) => super::c4_hands::texts(who, number),
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

/// "a creature died this turn", "N or more creatures died this turn" and the same under the player's control.
fn died_texts(left: &Amount, number: u32) -> Vec<String> {
    match left {
        Amount::Tallied {
            who: Ref::EachPlayer,
            what: TurnTally::CreaturesDied,
        } if number == 1 => vec!["a creature died this turn".to_owned()],
        Amount::Tallied {
            who: Ref::EachPlayer,
            what: TurnTally::CreaturesDied,
        } if number > 1 => quantity_texts(number)
            .iter()
            .map(|n| format!("{n} or more creatures died this turn"))
            .collect(),
        Amount::Tallied {
            who: Ref::You,
            what: what @ (TurnTally::CreaturesDiedUnder | TurnTally::NontokenCreaturesDiedUnder),
        } => died_under_texts(*what, number),
        _ => Vec::new(),
    }
}
