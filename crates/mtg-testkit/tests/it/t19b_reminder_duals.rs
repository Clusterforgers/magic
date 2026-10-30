//! Goal: the ten original dual lands, whose whole text is the reminder text of their basic land types, are `Verified` with no rules and play as printed at tables of two and four seats: each is a land of both its types, offers one mana ability for each type and no other, taps for either (and only once), and a land type an effect gives it does not take the two it has away. (t19b)
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;
use std::sync::Arc;

use common::name_of;
use mtg_cards::CardDefinition;
use mtg_cards::assemble::assemble;
use mtg_cards::support::{PrintedCard, PrintedFace, Support};
use mtg_core::card::{CardType, Subtype};
use mtg_core::mana::ManaType::{self, Black, Blue, Green, Red, White};
use mtg_engine::decision::DecisionKind;
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::turn::Step;
use mtg_engine::{Event, characteristics};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// The land as the server assembles it from its stored card: no rules, the printed side alone.
fn dual(name: &str, types: &str, text: &str) -> Arc<CardDefinition> {
    let printed = PrintedCard {
        oracle_id: "00000000-0000-0000-0000-000000000019".to_owned(),
        name: name.to_owned(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            type_line: format!("Land \u{2014} {types}"),
            oracle_text: text.to_owned(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let assembled = assemble(&printed, None, &mtg_engine::caps());
    assert_eq!(assembled.support.support, Support::Verified, "{name}");
    Arc::new(assembled.definition)
}

fn table(seats: usize, land: Arc<CardDefinition>) -> Driver {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(land)]);
    Driver::from_scenario(scenario).unwrap()
}

/// The types of mana the named permanent offers to the player holding priority.
fn offered(driver: &Driver, name: &str) -> BTreeSet<ManaType> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    options
        .mana_abilities
        .iter()
        .filter(|m| name_of(driver.state(), m.source.object) == name)
        .map(|m| m.produces)
        .collect()
}

/// The subtypes of the named permanent: the printed ones of its card and the ones effects gave it.
fn subtypes(driver: &Driver, name: &str, card: &CardDefinition) -> BTreeSet<String> {
    let id = driver.id_of(name).unwrap();
    let chars = characteristics::characteristics(driver.state(), id).unwrap();
    assert!(chars.types.contains(CardType::Land));
    let printed = card.subtypes.iter();
    chars
        .subtypes
        .iter()
        .chain(printed)
        .map(|s| s.0.clone())
        .collect()
}

const VOLCANIC: &str = "({T}: Add {U} or {R}.)";

#[rstest]
#[case::volcanic_island("Volcanic Island", "Island Mountain", "({T}: Add {U} or {R}.)", [Blue, Red])]
#[case::tundra("Tundra", "Plains Island", "({T}: Add {W} or {U}.)", [White, Blue])]
#[case::tropical_island("Tropical Island", "Forest Island", "({T}: Add {G} or {U}.)", [Green, Blue])]
#[case::badlands("Badlands", "Swamp Mountain", "({T}: Add {B} or {R}.)", [Black, Red])]
#[case::savannah("Savannah", "Forest Plains", "({T}: Add {G} or {W}.)", [Green, White])]
#[case::taiga("Taiga", "Mountain Forest", "({T}: Add {R} or {G}.)", [Red, Green])]
#[case::bayou("Bayou", "Swamp Forest", "({T}: Add {B} or {G}.)", [Black, Green])]
#[case::scrubland("Scrubland", "Plains Swamp", "({T}: Add {W} or {B}.)", [White, Black])]
#[case::plateau("Plateau", "Mountain Plains", "({T}: Add {R} or {W}.)", [Red, White])]
#[case::underground_sea("Underground Sea", "Island Swamp", "({T}: Add {U} or {B}.)", [Blue, Black])]
fn an_original_dual_is_both_its_land_types_and_taps_for_either_once(
    #[values(2, 4)] seats: usize,
    #[case] name: &str,
    #[case] types: &str,
    #[case] text: &str,
    #[case] mana: [ManaType; 2],
) {
    let land = dual(name, types, text);
    let driver = table(seats, land.clone());
    let wanted: BTreeSet<String> = types.split(' ').map(str::to_owned).collect();
    assert_eq!(subtypes(&driver, name, &land), wanted);
    assert_eq!(offered(&driver, name), BTreeSet::from(mana));
    for one in mana {
        let mut driver = table(seats, land.clone());
        let events = driver.tap_for_mana_of(name, one).unwrap();
        assert!(
            events
                .iter()
                .any(|event| matches!(event, Event::ManaAdded { mana, .. } if *mana == one)),
            "{name} adds {one:?}"
        );
        let id = driver.id_of(name).unwrap();
        assert!(
            driver
                .state()
                .object(id)
                .unwrap()
                .permanent()
                .unwrap()
                .tapped()
        );
        assert!(
            offered(&driver, name).is_empty(),
            "a tapped land offers nothing"
        );
        assert!(driver.tap_for_mana_of(name, one).is_err());
    }
}

#[rstest]
#[case::another_color(Black)]
#[case::colorless(ManaType::Colorless)]
fn a_dual_offers_no_mana_its_types_do_not_give(
    #[values(2, 4)] seats: usize,
    #[case] other: ManaType,
) {
    let mut driver = table(seats, dual("Volcanic Island", "Island Mountain", VOLCANIC));
    assert!(driver.tap_for_mana_of("Volcanic Island", other).is_err());
}

#[test]
fn a_land_type_an_effect_gives_adds_its_mana_to_the_two_the_land_has() {
    let land = dual("Volcanic Island", "Island Mountain", VOLCANIC);
    let mut driver = table(2, land.clone());
    let id = driver.id_of("Volcanic Island").unwrap();
    driver
        .add_effect(
            driver.on(id),
            Modification::AddSubtype(Subtype("Swamp".to_owned())),
            Duration::Forever,
        )
        .unwrap();
    assert_eq!(
        subtypes(&driver, "Volcanic Island", &land),
        BTreeSet::from(["Island", "Mountain", "Swamp"].map(str::to_owned))
    );
    let given = offered(&driver, "Volcanic Island");
    assert_eq!(given, BTreeSet::from([Blue, Red, Black]), "{given:?}");
}
