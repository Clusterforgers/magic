//! Life gain that a card changes: "If you would gain life, you gain that much life plus 1 instead.", "If you would gain
//! life, you gain twice that much life instead." (a `Replacement` of `LifeGain`), and "Players can't gain life.", "Your
//! opponents can't gain life." (the rule `CantGainLife`).

use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::GameRule;
use crate::refs::{Ref, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::Amount;

/// "If you would gain life, you gain that much life plus 1 instead.", "... you gain twice that much life instead."
pub(super) fn replacement(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("if you would gain life")?;
    r.punct(',').then_some(())?;
    r.need("you gain")?;
    let by = if r.word("twice") {
        r.need("that much life")?;
        ReplaceBy::Multiply(2)
    } else {
        r.need("that much life plus")?;
        ReplaceBy::Add(r.number().filter(|more| *more > 0)?)
    };
    r.word("instead").then_some(())?;
    Some(Ability::Replacement {
        when: ReplaceWhen::LifeGain {
            who: Ref::You,
            caused_own: false,
        },
        by,
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// What `replacement` reads, written back; empty for any other ability.
pub(super) fn replacement_sentences(ability: &Ability) -> Vec<String> {
    let Ability::Replacement {
        when:
            ReplaceWhen::LifeGain {
                who: Ref::You,
                caused_own: false,
            },
        by,
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    match by {
        ReplaceBy::Multiply(2) => {
            vec!["if you would gain life, you gain twice that much life instead".to_owned()]
        }
        ReplaceBy::Add(more) if *more > 0 => vec![format!(
            "if you would gain life, you gain that much life plus {more} instead"
        )],
        ReplaceBy::Instead(effects)
            if matches!(
                effects.as_slice(),
                [Effect::Draw {
                    who: Ref::You,
                    count: Amount::Triggering,
                    bind: None,
                }]
            ) =>
        {
            vec!["if you would gain life, draw that many cards instead".to_owned()]
        }
        _ => Vec::new(),
    }
}

/// "Players can't gain life.", "Your opponents can't gain life.", "You can't gain life."
pub(super) fn rule(r: &mut Reader<'_>) -> Option<GameRule> {
    let who = if r.words("players can't gain life") {
        Ref::EachPlayer
    } else if r.words("your opponents can't gain life") {
        Ref::Opponents
    } else if r.words("you can't gain life") {
        Ref::You
    } else {
        return None;
    };
    Some(GameRule::CantGainLife(who))
}

/// What `rule` reads, written back.
pub(super) fn rule_sentences(who: &Ref) -> Vec<String> {
    let text = match who {
        Ref::EachPlayer => "players can't gain life",
        Ref::Opponents => "your opponents can't gain life",
        Ref::You => "you can't gain life",
        _ => return Vec::new(),
    };
    vec![text.to_owned()]
}

#[cfg(test)]
mod r304_tests {
    //! Goal: life the controller would gain becomes that many cards drawn only for the controller's own gain and only
    //! for a draw of exactly that many.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::draw_that_many(r#"{"Replacement":{"when":{"LifeGain":{"who":"You"}},"by":{"Instead":[{"Draw":{"count":"Triggering"}}]}}}"#, true)]
    #[case::draw_one(
        r#"{"Replacement":{"when":{"LifeGain":{"who":"You"}},"by":{"Instead":[{"Draw":{}}]}}}"#,
        false
    )]
    #[case::opponents_gain(r#"{"Replacement":{"when":{"LifeGain":{"who":"Opponents"}},"by":{"Instead":[{"Draw":{"count":"Triggering"}}]}}}"#, false)]
    fn gained_life_becomes_cards(#[case] json: &str, #[case] written: bool) {
        let ability: Ability = serde_json::from_str(json).expect("ability");
        let texts = replacement_sentences(&ability);
        assert_eq!(
            texts.contains(&"if you would gain life, draw that many cards instead".to_owned()),
            written,
            "{texts:?}"
        );
    }
}
