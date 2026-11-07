//! Goal: in a game of two to four seats, mana spendable only on colorless spells, on spells from anywhere other than the
//! hand, or on spells with devoid pays for exactly the spells its fact allows, and a spell it does not allow is not
//! offered when that mana is all its caster has.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_testkit::cards::instant;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A land that taps for two mana of any one color, spendable only as `only` says.
fn land(only: &str) -> Arc<CardDefinition> {
    let rules = format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{{"AnyColor":2}},"only":{only}}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Spender Land")
            .with_type(CardType::Land)
            .with_rules(serde_json::from_str(&rules).unwrap()),
    )
}

/// A permanent that lets its controller cast instants from their graveyard.
fn graveyard_caster() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Graveyard Caster")
            .with_type(CardType::Artifact)
            .with_rules(
                serde_json::from_str(
                    r#"{"Rule":{"rule":{"MayCast":{"from":"Graveyard","what":[{"Type":"Instant"}]}}}}"#,
                )
                .unwrap(),
            ),
    )
}

/// An instant costing two: colorless, or red with `{1}{R}`, and with devoid when asked.
fn spell(colorless: bool, devoid: bool) -> Arc<CardDefinition> {
    let base = if colorless {
        instant("Spell", 2, &[])
    } else {
        instant("Spell", 1, &[Color::Red])
    };
    let mut card = (*base).clone();
    if devoid {
        card = card.with_rules(Ability::Keyword(Keyword::Devoid));
    }
    Arc::new(card)
}

const COLORLESS: &str = r#"[{"Spell":{"facts":["Colorless"]}}]"#;
const DEVOID: &str = r#"[{"Spell":{"facts":["Devoid"]}}]"#;
const ELSEWHERE: &str = r#"[{"Spell":{"facts":["NotFromHand"]}},{"Ability":{}}]"#;

#[rstest]
#[case::a_colorless_spell(2, COLORLESS, false, false, true)]
#[case::a_red_spell_is_not_colorless(2, COLORLESS, true, false, false)]
#[case::devoid_makes_a_red_spell_colorless(3, COLORLESS, true, true, true)]
#[case::a_spell_with_devoid(4, DEVOID, true, true, true)]
#[case::a_spell_without_devoid(3, DEVOID, true, false, false)]
fn a_fact_of_the_spell_decides_whether_the_mana_pays_from_the_hand(
    #[case] seats: usize,
    #[case] only: &str,
    #[case] red: bool,
    #[case] devoid: bool,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let table = Scenario::new(seats)
        .hand(0, [spell(!red, devoid)])
        .battlefield(0, [ready(land(only))]);
    let mut driver = Driver::from_scenario(table)?;
    assert_eq!(driver.begin_cast("Spell").is_ok(), offered);
    Ok(())
}

#[rstest]
#[case::from_the_hand(2, false, false)]
#[case::from_a_graveyard(3, true, true)]
fn mana_that_cant_be_spent_from_the_hand_pays_a_spell_from_a_graveyard(
    #[case] seats: usize,
    #[case] in_graveyard: bool,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut table =
        Scenario::new(seats).battlefield(0, [ready(land(ELSEWHERE)), ready(graveyard_caster())]);
    table = if in_graveyard {
        table.graveyard(0, [spell(false, false)])
    } else {
        table.hand(0, [spell(false, false)])
    };
    let mut driver = Driver::from_scenario(table)?;
    let cast = driver.cast("Spell");
    assert_eq!(cast.is_ok(), offered, "{cast:?}");
    if offered {
        assert!(
            driver
                .state()
                .objects()
                .any(|object| object.card().name == "Spell"
                    && object.zone() == mtg_engine::zones::Zone::Stack),
            "the spell is on the stack"
        );
    }
    Ok(())
}
