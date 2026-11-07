//! "Max speed — A" (CR 702.179): the permanent, or the card in a graveyard, has the ability A as long as its
//! controller has max speed. The writer gives "Max speed — " before each wording of A.

use crate::ability::Ability;
use crate::modify::Modification;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::Condition;

/// The words that open the clause.
const MAX_SPEED: &str = "Max speed \u{2014} ";

/// The ability a "Max speed — " static grants its own object: its object is this one (in the graveyard, the card
/// itself), the condition is that you have max speed, and it changes nothing else.
fn e15a_granted(ability: &Ability) -> Option<&Ability> {
    let Ability::Static {
        affected,
        mods,
        condition: Some(Condition::MaxSpeed(Ref::You)),
        defining: false,
        from,
    } = ability
    else {
        return None;
    };
    let own = match from {
        ZoneKind::Battlefield => *affected == Ref::This,
        ZoneKind::Graveyard => {
            *affected
                == Ref::Each(Selector(vec![
                    Pred::This,
                    Pred::In(ZoneKind::Graveyard),
                ]))
        }
        _ => false,
    };
    let [Modification::GrantAbility(granted)] = mods.as_slice() else {
        return None;
    };
    own.then_some(granted.as_ref())
}

/// The wordings of a "Max speed — " static, `None` for any other ability: one for each wording of the ability it
/// grants. A granted ability that works from a graveyard says so in its own words ("Exile this card from your
/// graveyard"), so it must be the zone the static works from.
pub(super) fn e15a_variants(
    ability: &Ability,
    write: fn(&Ability) -> Vec<String>,
) -> Option<Vec<String>> {
    let granted = e15a_granted(ability)?;
    let Ability::Static { from, .. } = ability else {
        return None;
    };
    if granted_zone(granted) != *from {
        return Some(Vec::new());
    }
    let mut texts = write(granted);
    if texts.is_empty() {
        texts = e15a_bodies(granted, write);
    }
    Some(
        texts
            .into_iter()
            .map(|text| format!("{MAX_SPEED}{text}"))
            .collect(),
    )
}

/// The condition a granted static is written under when it has no wording of its own: the static writers give "As
/// long as C, B" and its body B is the same under any condition.
const STAND_IN: &str = "As long as you have the city's blessing, ";

/// The wordings of a granted static that has none alone ("This creature has double strike."): the bodies of the same
/// static under a stand-in condition.
fn e15a_bodies(granted: &Ability, write: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ability::Static {
        affected,
        mods,
        condition: None,
        defining,
        from,
    } = granted
    else {
        return Vec::new();
    };
    let conditioned = Ability::Static {
        affected: affected.clone(),
        mods: mods.clone(),
        condition: Some(Condition::CitysBlessing(Ref::You)),
        defining: *defining,
        from: *from,
    };
    write(&conditioned)
        .iter()
        .filter_map(|text| text.strip_prefix(STAND_IN))
        .map(capitalized)
        .collect()
}

/// Reads a granted static that has no reading alone ("This creature has double strike.") under the stand-in condition,
/// then takes the condition away again; `None` unless it reads as a static under exactly that condition.
pub(super) fn e15a_read_body(body: &str, read: impl Fn(&str) -> Option<Ability>) -> Option<Ability> {
    let mut lowered = body.chars();
    let first = lowered.next()?;
    let text = format!("{STAND_IN}{}{}", first.to_lowercase(), lowered.as_str());
    match read(&text)? {
        Ability::Static {
            affected,
            mods,
            condition: Some(Condition::CitysBlessing(Ref::You)),
            defining,
            from,
        } => Some(Ability::Static {
            affected,
            mods,
            condition: None,
            defining,
            from,
        }),
        _ => None,
    }
}

/// The text with its first letter upper case.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// The static the reader makes of "Max speed — A" once it has read A: its object grants A to itself, from the zone A
/// works from, as long as you have max speed.
pub(super) fn e15a_wrap(granted: Ability) -> Ability {
    let from = granted_zone(&granted);
    let affected = match from {
        ZoneKind::Battlefield => Ref::This,
        zone => Ref::Each(Selector(vec![
            Pred::This,
            Pred::In(zone),
        ])),
    };
    Ability::Static {
        affected,
        mods: vec![Modification::GrantAbility(std::sync::Arc::new(granted))],
        condition: Some(Condition::MaxSpeed(Ref::You)),
        defining: false,
        from,
    }
}

/// The zone a granted ability works from: an activated ability names it; any other works where its object is.
fn granted_zone(granted: &Ability) -> ZoneKind {
    match granted {
        Ability::Activated { from, .. } => *from,
        _ => ZoneKind::Battlefield,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a "Max speed — " static is written as those words and the wording of the ability it grants, and a static with another condition, another object, a zone the granted ability does not work from, or no grant is not.
    use rstest::rstest;

    use super::*;
    use crate::compile::{same_text, written_variants};

    fn ability(json: &str) -> Ability {
        serde_json::from_str(json).unwrap()
    }

    fn writes(json: &str, text: &str) -> bool {
        written_variants(&ability(json))
            .iter()
            .any(|written| same_text(written, text))
    }

    const DOUBLE_STRIKE: &str = r#"{"Static":{"affected":"This","condition":{"MaxSpeed":"You"},"mods":[{"GrantAbility":{"Static":{"affected":"This","mods":[{"AddKeyword":"DoubleStrike"}]}}}]}}"#;
    const MILL: &str = r#"{"Static":{"affected":"This","condition":{"MaxSpeed":"You"},"mods":[{"GrantAbility":{"Triggered":{"effects":[{"Mill":{"count":2,"who":"Opponents"}}],"when":{"Drew":{"who":"You"}}}}}]}}"#;
    const DRAW_FROM_GRAVEYARD: &str = r#"{"Static":{"affected":{"Each":["This",{"In":"Graveyard"}]},"condition":{"MaxSpeed":"You"},"from":"Graveyard","mods":[{"GrantAbility":{"Activated":{"cost":[{"Mana":"{3}"},"ExileSelf"],"effects":[{"Draw":{}}],"from":"Graveyard"}}}]}}"#;
    const TAP_FOR_HASTE: &str = r#"{"Static":{"affected":"This","condition":{"MaxSpeed":"You"},"mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"AddKeyword":"Haste"}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}}]}}"#;

    #[rstest]
    #[case::a_keyword(DOUBLE_STRIKE, "Max speed \u{2014} This creature has double strike.")]
    #[case::a_trigger(MILL, "Max speed \u{2014} Whenever you draw a card, each opponent mills two cards.")]
    #[case::from_the_graveyard(DRAW_FROM_GRAVEYARD, "Max speed \u{2014} {3}, Exile this card from your graveyard: Draw a card.")]
    #[case::a_tap_ability(TAP_FOR_HASTE, "Max speed \u{2014} {T}: Target creature gains haste until end of turn.")]
    fn a_max_speed_static_is_written_as_the_card_words_it(#[case] json: &str, #[case] text: &str) {
        assert!(writes(json, text), "{text}: {:?}", written_variants(&ability(json)));
    }

    #[rstest]
    #[case::without_the_words(DOUBLE_STRIKE, "This creature has double strike.")]
    #[case::another_keyword(DOUBLE_STRIKE, "Max speed \u{2014} This creature has first strike.")]
    #[case::the_blessing(&DOUBLE_STRIKE.replace(r#"{"MaxSpeed":"You"}"#, r#"{"CitysBlessing":"You"}"#), "Max speed \u{2014} This creature has double strike.")]
    #[case::an_opponents_speed(&DOUBLE_STRIKE.replace(r#"{"MaxSpeed":"You"}"#, r#"{"MaxSpeed":"Opponents"}"#), "Max speed \u{2014} This creature has double strike.")]
    #[case::no_condition(&DOUBLE_STRIKE.replace(r#""condition":{"MaxSpeed":"You"},"#, ""), "Max speed \u{2014} This creature has double strike.")]
    #[case::another_object(&DOUBLE_STRIKE.replace(r#""affected":"This","condition""#, r#""affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"condition""#), "Max speed \u{2014} This creature has double strike.")]
    #[case::granted_from_the_battlefield(&DRAW_FROM_GRAVEYARD.replace(r#","from":"Graveyard","mods""#, r#","mods""#), "Max speed \u{2014} {3}, Exile this card from your graveyard: Draw a card.")]
    #[case::two_cards(MILL, "Max speed \u{2014} Whenever you draw a card, each opponent mills a card.")]
    fn another_static_is_not_the_text(#[case] json: &str, #[case] text: &str) {
        assert!(!writes(json, text), "{text}");
    }

    #[rstest]
    #[case::a_keyword(DOUBLE_STRIKE, "Max speed \u{2014} This creature has double strike.")]
    #[case::a_trigger(MILL, "Max speed \u{2014} Whenever you draw a card, each opponent mills two cards.")]
    #[case::from_the_graveyard(DRAW_FROM_GRAVEYARD, "Max speed \u{2014} {3}, Exile this card from your graveyard: Draw a card.")]
    fn the_reader_reads_the_static_back(#[case] json: &str, #[case] text: &str) {
        let read = crate::compile::read::clause(text, crate::compile::ClauseKind::Static, false);
        assert_eq!(read, Some(ability(json)), "{text}");
    }

    #[test]
    fn the_words_are_no_other_kind_of_clause() {
        let text = "Max speed \u{2014} {T}: Draw a card.";
        assert_eq!(crate::compile::classify_clause(text, false), crate::compile::ClauseKind::Static);
        assert_eq!(crate::compile::read::clause(text, crate::compile::ClauseKind::Activated, false), None);
    }
}
