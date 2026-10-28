//! Cost changes that count what happened this turn: "Creature spells you cast cost {1} less to cast for each 1 life your
//! opponents have lost this turn.", "This spell costs {1} less to cast for each creature that attacked this turn.",
//! "for each creature that died this turn", "for each 1 life you gained this turn" and "This spell costs {X} less to
//! cast, where X is the amount of life you gained this turn." Each is a tally of the turn (`Amount::Tallied`), read
//! and written by the cost modifier templates of `statics.rs` and of `c11a_ability_less.rs`.

use super::statics::cost_mana;
use super::tokens::Reader;
use crate::ability::{Ability, CostChange};
use crate::mana::ManaCostX;
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, Cmp, Condition, TurnTally};
use mtg_core::card::CardType;

/// The tallies after "for each": the words and what they count.
const C11A_EACH: [(&str, Ref, TurnTally); 4] = [
    (
        "1 life your opponents have lost this turn",
        Ref::Opponents,
        TurnTally::LifeLost,
    ),
    (
        "1 life you gained this turn",
        Ref::You,
        TurnTally::LifeGainedTotal,
    ),
    (
        "creature that attacked this turn",
        Ref::EachPlayer,
        TurnTally::CreaturesAttacked,
    ),
    (
        "creature that died this turn",
        Ref::EachPlayer,
        TurnTally::CreaturesDied,
    ),
];

/// The tallies after "where X is".
const C11A_X: [(&str, Ref, TurnTally); 2] = [
    (
        "the amount of life you gained this turn",
        Ref::You,
        TurnTally::LifeGainedTotal,
    ),
    (
        "the amount of life your opponents have lost this turn",
        Ref::Opponents,
        TurnTally::LifeLost,
    ),
];

/// A tally of the turn as a cost change counts it: "for each ..." after a fixed mana, ", where X is ..." after `{X}`.
pub(super) fn c11a_turn_count(r: &mut Reader<'_>, x: bool) -> Option<Amount> {
    let table: &[(&str, Ref, TurnTally)] = if x {
        r.punct(',').then_some(())?;
        r.need("where x is")?;
        &C11A_X
    } else {
        r.need("for each")?;
        &C11A_EACH
    };
    table.iter().find_map(|(words, who, what)| {
        r.attempt(|r| {
            // "1 life" opens with a number token.
            let words = match words.strip_prefix("1 ") {
                Some(rest) => (r.number()? == 1).then_some(rest)?,
                None => words,
            };
            r.words(words).then(|| Amount::Tallied {
                who: who.clone(),
                what: *what,
            })
        })
    })
}

/// The mana and the words after "to cast" of a cost change that counts a tally of the turn, in every wording; `None`
/// for any other count.
pub(super) fn c11a_turn_wordings(mana: &ManaCostX, per: &Amount) -> Option<Vec<(String, String)>> {
    let Amount::Tallied { who, what } = per else {
        return None;
    };
    let named = |table: &[(&'static str, Ref, TurnTally)]| {
        table
            .iter()
            .find(|(_, w, t)| w == who && t == what)
            .map(|(words, ..)| *words)
    };
    let mut wordings = Vec::new();
    if let Some(words) = named(&C11A_EACH) {
        wordings.push((mana.to_string(), format!(" for each {words}")));
    }
    if let Some(words) = named(&C11A_X).filter(|_| mana.to_string() == "{1}") {
        wordings.push(("{X}".to_owned(), format!(", where X is {words}")));
    }
    (!wordings.is_empty()).then_some(wordings)
}

/// The ordinals of "the first spell you cast each turn": the word and how many such spells were cast before it.
const C11A_ORDINALS: [(&str, i32); 2] = [("first", 0), ("second", 1)];

/// The kinds of spell an ordinal counts: the words before "spell", the type, and the tally of the turn.
const C11A_KINDS: [(&str, Option<CardType>, TurnTally); 3] = [
    (
        "creature",
        Some(CardType::Creature),
        TurnTally::CreatureSpellsCast,
    ),
    (
        "artifact",
        Some(CardType::Artifact),
        TurnTally::ArtifactSpellsCast,
    ),
    ("", None, TurnTally::SpellsCast),
];

/// The spells of a kind the caster casts.
fn c11a_spells_of(kind: Option<CardType>) -> Selector {
    let mut spells = vec![Pred::Spell];
    spells.extend(kind.map(Pred::Type));
    spells.push(Pred::ControlledBy(Ref::You));
    Selector(spells)
}

/// "The first creature spell you cast each turn costs {2} less to cast.": a cost modifier of the caster's spells of the
/// kind while none of that kind was cast this turn (one for "the second").
pub(super) fn c11a_first_spell(r: &mut Reader<'_>) -> Option<Ability> {
    r.word("the").then_some(())?;
    let before = C11A_ORDINALS
        .iter()
        .find(|(word, _)| r.word(word))
        .map(|(_, before)| *before)?;
    let (kind, tally) = C11A_KINDS
        .iter()
        .find(|(words, ..)| words.is_empty() || r.word(words))
        .map(|(_, kind, tally)| (*kind, *tally))?;
    r.words("spell you cast each turn costs").then_some(())?;
    let (mana, x) = cost_mana(r)?;
    let change = if r.word("less") {
        CostChange::Reduce
    } else if r.word("more") {
        CostChange::Increase
    } else {
        return None;
    };
    (!x && r.words("to cast")).then_some(())?;
    Some(Ability::CostModifier {
        applies_to: c11a_spells_of(kind),
        change,
        mana,
        per: Amount::one(),
        condition: Some(Condition::Compare {
            left: Amount::Tallied {
                who: Ref::You,
                what: tally,
            },
            op: Cmp::Eq,
            right: Amount::Fixed(before),
        }),
    })
}

/// The wording of a cost modifier [`c11a_first_spell`] reads; `None` for any other.
pub(super) fn c11a_first_spell_text(
    applies_to: &Selector,
    change: CostChange,
    mana: &ManaCostX,
    per: &Amount,
    condition: Option<&Condition>,
) -> Option<String> {
    let Some(Condition::Compare {
        left: Amount::Tallied {
            who: Ref::You,
            what,
        },
        op: Cmp::Eq,
        right: Amount::Fixed(before),
    }) = condition
    else {
        return None;
    };
    per.is_one().then_some(())?;
    let ordinal = C11A_ORDINALS.iter().find(|(_, n)| n == before)?.0;
    let (words, kind, _) = C11A_KINDS.iter().find(|(.., tally)| tally == what)?;
    (*applies_to == c11a_spells_of(*kind)).then_some(())?;
    let more = match change {
        CostChange::Increase => "more",
        CostChange::Reduce => "less",
    };
    let kind = if words.is_empty() {
        String::new()
    } else {
        format!("{words} ")
    };
    Some(format!(
        "The {ordinal} {kind}spell you cast each turn costs {mana} {more} to cast"
    ))
}
