//! "Enchanted creature can't attack or block, and its activated abilities can't be activated. Its controller may exile
//! three cards from their graveyard for that player to ignore this effect until end of turn." (Lost in Thought,
//! Volrath's Curse): the Arrest pair under the condition that the enchanted creature's controller has not paid, and the
//! rule that lets that player pay the cost to ignore them (`GameRule::PayToIgnoreBy`, CR 116.2).

use super::Own;
use super::ce29_arrest::ce29_arrest;
use super::costs::{cost, lowered, text as cost_text};
use super::render::capitalize;
use super::tokens::{Reader, tokenize};
use crate::ability::Ability;
use crate::cost::{Cost, CostItem};
use crate::modify::GameRule;
use crate::refs::{Ref, ZoneKind};
use crate::value::{Amount, Condition};

/// The ways the card names the payer.
const PAYERS: [&str; 2] = ["Its controller may ", "That creature's controller may "];

/// What follows the cost.
const TAIL: &str = " for that player to ignore this effect until end of turn.";

/// The payer: the enchanted creature's controller.
fn payer() -> Ref {
    Ref::ControllerOf(Box::new(Ref::Attached))
}

/// The condition of the two restrictions: that player has not paid this turn.
fn unpaid() -> Condition {
    Condition::Not(Box::new(Condition::PaidToIgnoreBy(payer())))
}

/// The three abilities of the clause, or none for another clause.
pub(super) fn r109_ignore_read(text: &str, x_free: bool, own: &Own) -> Option<Vec<Ability>> {
    let (first, rest) = text.split_once(". ")?;
    let mut found = ce29_arrest(&format!("{first}."), x_free, own)?;
    let rest = PAYERS.iter().find_map(|head| rest.strip_prefix(head))?;
    let words = rest.strip_suffix(TAIL)?;
    let words = words.strip_suffix(" of their choice").unwrap_or(words);
    let words = words
        .strip_prefix("pay {")
        .map_or_else(|| words.to_owned(), |mana| format!("{{{mana}"));
    let words = capitalize(&words.replace(" their ", " your "));
    let tokens = tokenize(&words);
    let mut r = Reader::new(&tokens, false);
    let paid = cost(&mut r)?;
    r.end().then_some(())?;
    for ability in &mut found {
        match ability {
            Ability::Static { condition, .. } | Ability::Rule { condition, .. }
                if condition.is_none() =>
            {
                *condition = Some(unpaid());
            }
            _ => return None,
        }
    }
    found.push(rule(paid));
    Some(found)
}

/// The rule that lets the enchanted creature's controller pay `cost`.
fn rule(cost: Cost) -> Ability {
    Ability::Rule {
        rule: GameRule::PayToIgnoreBy {
            payer: payer(),
            cost,
        },
        condition: None,
        from: ZoneKind::Battlefield,
    }
}

/// `ability` without the condition the clause gives it; none when it has another.
fn unconditioned(ability: &Ability) -> Option<Ability> {
    let mut bare = ability.clone();
    match &mut bare {
        Ability::Static { condition, .. } | Ability::Rule { condition, .. }
            if *condition == Some(unpaid()) =>
        {
            *condition = None;
        }
        _ => return None,
    }
    Some(bare)
}

/// The cost as the clause says it after "may": "pay {2}", "sacrifice a permanent", "exile three cards from their
/// graveyard".
fn cost_words(cost: &Cost) -> Option<String> {
    if let [CostItem::Mana(mana)] = cost.0.as_slice() {
        return Some(format!("pay {mana}"));
    }
    if let [CostItem::Sacrifice { filter, count }] = cost.0.as_slice()
        && filter.0.is_empty()
        && *count == Amount::one()
    {
        return Some("sacrifice a permanent".to_owned());
    }
    Some(lowered(&cost_text(cost)?).replace(" your ", " their "))
}

/// Every wording of the three abilities as the clause; empty for any other three.
pub(super) fn r109_ignore_texts(first: &Ability, second: &Ability, third: &Ability) -> Vec<String> {
    let Ability::Rule {
        rule: GameRule::PayToIgnoreBy { payer: who, cost },
        condition: None,
        from: ZoneKind::Battlefield,
    } = third
    else {
        return Vec::new();
    };
    let (Some(first), Some(second)) = (unconditioned(first), unconditioned(second)) else {
        return Vec::new();
    };
    if *who != payer() {
        return Vec::new();
    }
    let Some(words) = cost_words(cost) else {
        return Vec::new();
    };
    let mut costs = vec![words.clone()];
    if words.starts_with("sacrifice ") {
        costs.push(format!("{words} of their choice"));
    }
    let mut texts = Vec::new();
    for pair in super::static_pairs::texts(&first, &second) {
        for head in PAYERS {
            for words in &costs {
                texts.push(format!("{pair} {head}{words}{TAIL}"));
            }
        }
    }
    texts
}

#[cfg(test)]
mod tests {
    //! Goal: the clause reads into the two restrictions under the unpaid condition and the payer's rule, for each payer
    //! wording and cost, writes back as itself, and another tail or payer is not read.
    use rstest::rstest;

    use super::*;

    const ARREST: &str =
        "Enchanted creature can't attack or block, and its activated abilities can't be activated.";

    #[rstest]
    #[case::lost_in_thought(
        "Its controller may exile three cards from their graveyard for that player to ignore this effect until end of turn."
    )]
    #[case::volraths_curse(
        "That creature's controller may sacrifice a permanent of their choice for that player to ignore this effect until end of turn."
    )]
    #[case::mana(
        "Its controller may pay {2} for that player to ignore this effect until end of turn."
    )]
    fn the_clause_reads_and_writes_back(#[case] tail: &str) {
        let text = format!("{ARREST} {tail}");
        let found = r109_ignore_read(&text, false, &Own::default()).expect("read");
        let [first, second, third] = found.as_slice() else {
            panic!("three abilities: {found:?}");
        };
        let written = r109_ignore_texts(first, second, third);
        assert!(written.contains(&text), "{written:?}");
    }

    #[rstest]
    #[case::any_player(
        "Any player may pay {2} for that player to ignore this effect until end of turn."
    )]
    #[case::other_tail("Its controller may pay {2} to end this effect.")]
    #[case::another_restriction_first(
        "Enchanted creature can't block. Its controller may pay {2} for that player to ignore this effect until end of turn."
    )]
    fn another_clause_is_not_read(#[case] tail: &str) {
        let text = if tail.starts_with("Enchanted") {
            tail.to_owned()
        } else {
            format!("{ARREST} {tail}")
        };
        assert_eq!(r109_ignore_read(&text, false, &Own::default()), None);
    }
}
