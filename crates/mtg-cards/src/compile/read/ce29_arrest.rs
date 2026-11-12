//! "Enchanted creature can't attack or block, and its activated abilities can't be activated." (Arrest, Stasis Cocoon):
//! a static ability of the attached permanent and the rule that its activated abilities, mana abilities included, can't
//! be activated by any player.

use mtg_core::card::CardType;

use super::{ClauseKind, Own, clause_with};
use crate::ability::{Ability, ActivationKind};
use crate::modify::{GameRule, Prohibited};
use crate::refs::{Pred, Ref, Selector, ZoneKind};

/// The tail that names the rule.
const TAIL: &str = ", and its activated abilities can't be activated.";

/// The sentence as the writers say it: another noun than "creature" after "Enchanted" is said as "permanent".
pub(super) fn ce29_said(text: &str) -> String {
    match text
        .strip_prefix("Enchanted ")
        .and_then(|rest| rest.split_once(' '))
    {
        Some((noun, rest)) if noun != "creature" && text.ends_with(TAIL) => {
            format!("Enchanted permanent {rest}")
        }
        _ => text.to_owned(),
    }
}

/// The two abilities of the sentence, or none for another sentence.
pub(super) fn ce29_arrest(text: &str, x_free: bool, own: &Own) -> Option<Vec<Ability>> {
    let front = text.strip_suffix(TAIL)?;
    let creature = front.starts_with("Enchanted creature ");
    // The static ability is written of "Enchanted creature"; the noun of another subject only repeats what the Aura
    // enchants.
    let rest = front.strip_prefix("Enchanted ")?.split_once(' ')?.1;
    let first = clause_with(
        &format!("Enchanted creature {rest}."),
        ClauseKind::Static,
        x_free,
        own,
    )?;
    let Ability::Static {
        affected: Ref::Attached,
        ..
    } = &first
    else {
        return None;
    };
    // "Enchanted creature" names the kind of its sources; another noun only repeats what the Aura enchants.
    let mut sources = Vec::new();
    if creature {
        sources.push(Pred::Type(CardType::Creature));
    }
    sources.push(Pred::Is(Ref::Attached));
    let rule = Ability::Rule {
        rule: GameRule::Cant {
            who: Ref::EachPlayer,
            action: Prohibited::ActivateKind {
                kind: ActivationKind::Every,
                sources: Selector(sources),
            },
        },
        condition: None,
        from: ZoneKind::Battlefield,
    };
    Some(vec![first, rule])
}

#[cfg(test)]
mod tests {
    //! Goal: the sentence reads into the static ability of the attached permanent and the activation rule, with the
    //! creature type in the sources only where the subject names a creature; another tail is not read.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::arrest(
        "Enchanted creature can't attack or block, and its activated abilities can't be activated.",
        Some(2)
    )]
    #[case::stasis_cocoon(
        "Enchanted artifact can't attack or block, and its activated abilities can't be activated.",
        Some(1)
    )]
    #[case::another_tail(
        "Enchanted creature can't attack or block, and it loses all abilities.",
        None
    )]
    fn the_sentence_reads(#[case] text: &str, #[case] sources: Option<usize>) {
        let found = ce29_arrest(text, false, &Own::default());
        let count = found.as_ref().and_then(|found| match found.as_slice() {
            [
                _,
                Ability::Rule {
                    rule:
                        GameRule::Cant {
                            action: Prohibited::ActivateKind { sources, .. },
                            ..
                        },
                    ..
                },
            ] => Some(sources.0.len()),
            _ => None,
        });
        assert_eq!(count, sources, "{text}: {found:?}");
    }
}
