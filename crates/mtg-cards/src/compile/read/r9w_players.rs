//! The players for whom a fact holds (`Ref::PlayersWhere`), written back: "for each opponent who lost life this
//! turn", "for each graveyard with seven or more cards in it", "where X is the number of opponents who have three or
//! more poison counters", "if an opponent was dealt damage this turn" and "a player ... if it's not their turn". No
//! template reads them: only the proposal checker writes them.

use super::amounts::count_word;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::{Amount, Cmp, Condition, CounterKind, TurnTally};

/// A fact about one player, as the words after the singular and the plural noun.
struct Fact {
    one: String,
    many: Vec<String>,
}

/// "seven or more": a number at least 2 in words, then "or more".
fn or_more(right: &Amount) -> Option<String> {
    let Amount::Fixed(number) = right else {
        return None;
    };
    let number = u32::try_from(*number).ok().filter(|number| *number > 1)?;
    Some(format!("{} or more", count_word(number)?))
}

/// The cards a player owns in one zone: `[OwnedBy You, In zone]`.
fn owned_in(selector: &Selector) -> Option<ZoneKind> {
    match selector.0.as_slice() {
        [Pred::OwnedBy(Ref::You), Pred::In(zone)] => Some(*zone),
        _ => None,
    }
}

/// The fact the test states about the player "you" names.
fn fact(test: &Condition) -> Option<Fact> {
    let Condition::Compare { left, op, right } = test else {
        return None;
    };
    if *op != Cmp::Ge {
        return None;
    }
    let one = |text: &str| {
        Some(Fact {
            one: text.to_owned(),
            many: vec![text.to_owned()],
        })
    };
    match left {
        Amount::Tallied {
            who: Ref::You,
            what: TurnTally::LifeLost,
        } if *right == Amount::Fixed(1) => one("who lost life this turn"),
        Amount::Tallied {
            who: Ref::You,
            what: TurnTally::DamageTaken,
        } if *right == Amount::Fixed(1) => Some(Fact {
            one: "who was dealt damage this turn".to_owned(),
            many: vec!["who were dealt damage this turn".to_owned()],
        }),
        Amount::Counters {
            on: Ref::You,
            counter: CounterKind::Poison,
        } => {
            let n = or_more(right)?;
            Some(Fact {
                one: format!("with {n} poison counters"),
                many: vec![
                    format!("who have {n} poison counters"),
                    format!("with {n} poison counters"),
                ],
            })
        }
        Amount::Count(of) if owned_in(of) == Some(ZoneKind::Hand) => {
            one(&format!("with {} cards in hand", or_more(right)?))
        }
        _ => None,
    }
}

/// "for each ..." of the players a fact holds for: "opponent who lost life this turn", "graveyard with seven or more
/// cards in it".
pub(super) fn r9w_each_text(counted: &Amount) -> Option<String> {
    let Amount::Players(Ref::PlayersWhere { who, test }) = counted else {
        return None;
    };
    if let Some(graveyard) = graveyards(who, test, false) {
        return Some(graveyard);
    }
    Some(format!("{} {}", noun(who)?.0, fact(test)?.one))
}

/// "the number of ..." of the players a fact holds for: "the number of opponents who have three or more poison
/// counters", "the number of your opponents with four or more cards in hand".
pub(super) fn r9w_number_texts(counted: &Amount) -> Vec<String> {
    let Amount::Players(Ref::PlayersWhere { who, test }) = counted else {
        return Vec::new();
    };
    if let Some(graveyard) = graveyards(who, test, true) {
        return vec![format!("the number of {graveyard}")];
    }
    let (Some((_, many)), Some(fact)) = (noun(who), fact(test)) else {
        return Vec::new();
    };
    let mut texts = Vec::new();
    for tail in &fact.many {
        texts.push(format!("the number of {many} {tail}"));
        if **who == Ref::Opponents {
            texts.push(format!("the number of your {many} {tail}"));
        }
    }
    texts
}

/// The players as a noun: ("opponent", "opponents"), ("player", "players").
fn noun(who: &Ref) -> Option<(&'static str, &'static str)> {
    match who {
        Ref::Opponents => Some(("opponent", "opponents")),
        Ref::EachPlayer => Some(("player", "players")),
        _ => None,
    }
}

/// Each player's graveyard with N or more cards: "graveyard with seven or more cards in it".
fn graveyards(who: &Ref, test: &Condition, many: bool) -> Option<String> {
    let Condition::Compare {
        left: Amount::Count(of),
        op: Cmp::Ge,
        right,
    } = test
    else {
        return None;
    };
    (*who == Ref::EachPlayer && owned_in(of) == Some(ZoneKind::Graveyard)).then_some(())?;
    let n = or_more(right)?;
    Some(if many {
        format!("graveyards with {n} cards in them")
    } else {
        format!("graveyard with {n} cards in it")
    })
}

/// At least one of the players has the fact of the turn: "an opponent was dealt damage this turn" (its negation is
/// "unless" the same words).
pub(super) fn r9w_some_text(left: &Amount, op: Cmp, right: &Amount) -> Option<String> {
    let Amount::Players(Ref::PlayersWhere { who, test }) = left else {
        return None;
    };
    (op == Cmp::Ge && *right == Amount::Fixed(1)).then_some(())?;
    let (one, _) = noun(who)?;
    let Condition::Compare {
        left: Amount::Tallied {
            who: Ref::You,
            what,
        },
        op: Cmp::Ge,
        right: tested,
    } = &**test
    else {
        return None;
    };
    (*tested == Amount::Fixed(1)).then_some(())?;
    let verb = match what {
        TurnTally::DamageTaken => "was dealt damage this turn",
        TurnTally::LifeLost => "lost life this turn",
        _ => return None,
    };
    let article = if one.starts_with('o') { "an" } else { "a" };
    Some(format!("{article} {one} {verb}"))
}

/// The players whose turn it is not: "a player" with "if it's not their turn", and who they are.
pub(super) fn r9w_off_turn(who: &Ref) -> Option<&'static str> {
    let Ref::PlayersWhere { who, test } = who else {
        return None;
    };
    let Condition::Not(inner) = &**test else {
        return None;
    };
    (**inner == Condition::YourTurn).then_some(())?;
    noun(who).map(|(one, _)| one)
}

/// "..., ~ deals 4 damage to them.": the one player of the event ("a player casts", "each player's upkeep") is "them"
/// where the data writes "that player".
pub(super) fn r9w_damage_them(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter(|text| {
            text.matches("that player").count() == 1
                && (text.starts_with("Whenever a player ")
                    || text.starts_with("At the beginning of each player's "))
        })
        .filter_map(|text| {
            let at = text.find(" damage to that player")?;
            let (head, tail) = text.split_at(at);
            Some(format!(
                "{head}{}",
                tail.replacen(" damage to that player", " damage to them", 1)
            ))
        })
        .collect()
}

/// "Whenever a player casts a spell, if it's not their turn": a spell cast by a player out of their own turn.
pub(super) fn r9w_cast_text(what: &Selector, by: &Ref) -> Option<String> {
    let Ref::PlayersWhere { who, .. } = by else {
        return None;
    };
    r9w_off_turn(by)?;
    let cast = super::c4_cast_hand::cast_text(what, who)?;
    Some(format!("{cast}, if it's not their turn"))
}
