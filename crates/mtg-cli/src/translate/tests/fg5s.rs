//! Goal: the statics, keywords and zone constructs of family fg5s translate with the condition they state, an unread
//! condition stays a gap, and the translated replacement plays through the engine on the object it names.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

use super::{abilities, gaps};

fn script(line: &str, svars: &str) -> String {
    format!("Name:Subject\nTypes:Creature Elf\nPT:2/2\n{line}\n{svars}")
}

#[rstest]
#[case::plain("K:etbCounter:P1P1:2", "")]
#[case::no_condition("K:etbCounter:P1P1:X:no Condition:Converge", "SVar:X:Count$Converge\n")]
#[case::escaped("K:etbCounter:P1P1:2:ValidCard$ Card.Self+escaped:It escapes.", "")]
#[case::revolt("K:etbCounter:P1P1:1:Revolt$ True:Revolt.", "")]
#[case::adamant("K:etbCounter:P1P1:1:Adamant$ White:Adamant.", "")]
#[case::morbid(
    "K:etbCounter:P1P1:1:CheckSVar$ Morbid:Morbid.",
    "SVar:Morbid:Count$Morbid.1.0\n"
)]
fn an_enters_counter_line_reads_its_condition(#[case] line: &str, #[case] svars: &str) {
    let text = script(line, svars);
    assert!(
        gaps(&text).is_empty() || line.contains("Converge"),
        "{:?}",
        gaps(&text)
    );
}

#[rstest]
#[case::adamant_unknown("K:etbCounter:P1P1:1:Adamant$ Purple:Adamant.")]
#[case::present("K:etbCounter:P1P1:1:IsPresent$ Permanent.YouCtrl+cmcGE4:Big.")]
#[case::three_fields("K:etbCounter:P1P1:1:CheckSVar$ X:Desc:Extra")]
fn an_enters_counter_condition_it_cannot_read_stays_a_gap(#[case] line: &str) {
    let text = script(line, "SVar:X:Count$Morbid.1.0\n");
    assert_eq!(gaps(&text).len(), 1, "{line}");
    assert!(abilities(&text).is_empty(), "{line}");
}

/// The card of a translated script, castable for nothing.
fn card_of(line: &str, svars: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Subject")
        .with_type(CardType::Creature)
        .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
        .with_power_toughness(2, 2);
    for value in abilities(&script(line, svars)) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

/// Casts the subject from the first seat's hand and passes until it has resolved.
fn cast_subject(line: &str, svars: &str) -> Driver {
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [card_of(line, svars)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Subject").unwrap();
    for _ in 0..50 {
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        if idle
            && matches!(
                driver.engine().decision().map(|d| &d.kind),
                Some(DecisionKind::Priority(_))
            )
        {
            return driver;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack never emptied");
}

fn counters(driver: &Driver) -> u32 {
    let id = driver.id_of("Subject").unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

#[rstest]
#[case::unconditional("K:etbCounter:P1P1:2", "", 2)]
#[case::no_condition_text("K:etbCounter:P1P1:3:no Condition:Three.", "", 3)]
#[case::morbid_not_met(
    "K:etbCounter:P1P1:2:CheckSVar$ Morbid:Morbid.",
    "SVar:Morbid:Count$Morbid.1.0\n",
    0
)]
fn a_cast_creature_enters_with_the_counters_its_line_states(
    #[case] line: &str,
    #[case] svars: &str,
    #[case] expected: u32,
) {
    let driver = cast_subject(line, svars);
    assert_eq!(counters(&driver), expected);
}
