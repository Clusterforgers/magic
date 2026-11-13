//! r47-03 wordings of whole abilities: a loss of life made a multiple ("If an opponent would lose life during your turn,
//! they lose twice that much life instead.").

use crate::ability::Ability;
use crate::compile::read::r4703_reads::{R4703_LOSERS, R4703_TIMES};
use crate::refs::ZoneKind;
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::Condition;

/// Adds the wordings of the r47-03 whole abilities.
pub(super) fn r4703_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    texts.extend(r4703_loss_times(ability));
    texts.extend(r4703_counts_as_named(ability));
}

/// "If this card is in a graveyard, effects from spells named <name> count it as a card named <name>."
fn r4703_counts_as_named(ability: &Ability) -> Vec<String> {
    let Ability::Rule {
        rule: crate::modify::GameRule::CountsAsNamed(name),
        condition: None,
        from: ZoneKind::Graveyard,
    } = ability
    else {
        return Vec::new();
    };
    ["this card", "~"]
        .iter()
        .map(|card| {
            format!(
                "If {card} is in a graveyard, effects from spells named {name} count it as a card named {name}."
            )
        })
        .collect()
}

/// "If <player> would lose life[ during your turn], <they> lose <twice> that much life instead."
fn r4703_loss_times(ability: &Ability) -> Option<String> {
    let Ability::Replacement {
        when: ReplaceWhen::LifeLoss { who },
        by: ReplaceBy::LossTimes(times),
        condition,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return None;
    };
    let (words, _, pronoun) = R4703_LOSERS.iter().find(|(_, named, _)| named == who)?;
    let (factor, _) = R4703_TIMES.iter().find(|(_, factor)| factor == times)?;
    let during = match condition {
        None => "",
        Some(Condition::YourTurn) => " during your turn",
        Some(_) => return None,
    };
    Some(format!(
        "If {words} would lose life{during}, {pronoun} lose {factor} that much life instead."
    ))
}

#[cfg(test)]
mod tests {
    //! Goal: a loss of life made a multiple is written as the cards print it, and one under another condition is not
    //! written.
    use super::*;
    use crate::refs::Ref;
    use rstest::rstest;

    fn r4703_loss(who: Ref, times: u32, condition: Option<Condition>) -> Ability {
        Ability::Replacement {
            when: ReplaceWhen::LifeLoss { who },
            by: ReplaceBy::LossTimes(times),
            condition,
            from: ZoneKind::Battlefield,
        }
    }

    #[rstest]
    #[case::bloodletter(
        r4703_loss(Ref::Opponents, 2, Some(Condition::YourTurn)),
        Some(
            "If an opponent would lose life during your turn, they lose twice that much life instead."
        )
    )]
    #[case::you(
        r4703_loss(Ref::You, 2, None),
        Some("If you would lose life, you lose twice that much life instead.")
    )]
    #[case::four_times(r4703_loss(Ref::Opponents, 4, None), None)]
    #[case::other_condition(r4703_loss(Ref::Opponents, 2, Some(Condition::AfterCombat)), None)]
    fn r4703_loss_is_written_as_printed(#[case] ability: Ability, #[case] text: Option<&str>) {
        assert_eq!(r4703_loss_times(&ability).as_deref(), text);
    }

    #[rstest]
    #[case::graveyard(ZoneKind::Graveyard, true)]
    #[case::battlefield(ZoneKind::Battlefield, false)]
    fn r4703_counts_as_named_is_written_from_the_graveyard(
        #[case] from: ZoneKind,
        #[case] written: bool,
    ) {
        let ability = Ability::Rule {
            rule: crate::modify::GameRule::CountsAsNamed("Muscle Burst".to_owned()),
            condition: None,
            from,
        };
        let texts = r4703_counts_as_named(&ability);
        assert_eq!(
            texts
                .iter()
                .any(|text| text.contains("~ is in a graveyard")),
            written
        );
    }
}
