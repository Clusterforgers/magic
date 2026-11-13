//! Goal: the facts of the turn and the game the r55-22 pack reads hold exactly when the game's history makes them true,
//! at two to four seats: a card put into a graveyard this combat is no target of Storrev, Devkarin Lich; Spider-Man
//! 2099 deals damage only after a land played or a spell cast from anywhere other than the hand; Valkmira, Protector's
//! Shield taxes a spell that targets its controller or another permanent of theirs, never one that targets itself.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, elf, giant, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// The ability of a corpus card's face, by index.
pub(crate) fn r5522_ability(name: &str, face: usize, index: usize) -> Ability {
    let library = mtg_cards::builtin::library().unwrap();
    library.card_by_name(name).unwrap().faces[face].abilities[index].clone()
}

/// A permanent of the type with one ability of a corpus card.
pub(crate) fn r5522_holder(name: &str, kind: CardType, ability: Ability) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_power_toughness(0, 4)
            .with_rules(ability),
    )
}

/// Answers until the stack is empty with priority pending; `blocks` answers a blocker question and `targets` picks from
/// the legal targets offered (their names given).
pub(crate) fn r5522_settle(
    driver: &mut Driver,
    blocks: &[(&str, &str)],
    targets: &mut dyn FnMut(&Driver, &[Target]) -> Vec<Target>,
) {
    // A combat goes on to its end; any other moment stops once the stack is empty.
    let in_combat = driver.state().turn().step().phase() == mtg_engine::turn::Phase::Combat;
    for _ in 0..80 {
        let decision = driver.decision().unwrap().clone();
        let combat_over = !in_combat || driver.state().turn().step() == Step::Main2;
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() && combat_over => break,
            DecisionKind::DeclareBlockers(_) => {
                driver.block(blocks).unwrap();
            }
            DecisionKind::ChooseTargets(options) => {
                let chosen = targets(driver, &options.legal);
                driver.choose_targets(&chosen).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
}

/// The name of an object target.
pub(crate) fn r5522_name(driver: &Driver, target: &Target) -> String {
    target
        .object()
        .and_then(|object| driver.state().object(object.id))
        .map(|object| object.card().name.clone())
        .unwrap_or_default()
}

/// Whether a permanent of the name is on the battlefield.
fn r5522_on_battlefield(driver: &Driver, name: &str) -> bool {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| driver.state().object(*id).unwrap().card().name == name)
}

#[rstest]
fn r5522_storrev_skips_what_died_this_combat(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Storrev, Devkarin Lich").unwrap()),
                ready(bear()),
            ],
        )
        .battlefield(1, [ready(giant())])
        .graveyard(0, [elf()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    // The card in the graveyard was put there before this combat began.
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver
        .attack(&[("Storrev, Devkarin Lich", 1), ("Grizzly Bears", 1)])
        .unwrap();
    let mut offered: Vec<String> = Vec::new();
    r5522_settle(
        &mut driver,
        &[("Craw Wurm", "Grizzly Bears")],
        &mut |driver, legal| {
            offered = legal.iter().map(|t| r5522_name(driver, t)).collect();
            legal.iter().take(1).copied().collect()
        },
    );
    // A single legal target may be chosen without a question; when asked, the Bears are not offered.
    assert!(
        offered.is_empty() || offered == ["Llanowar Elf"],
        "the Bears died this combat: {offered:?}"
    );
    let hand = driver
        .state()
        .zone(Zone::Hand(mtg_core::ids::PlayerId::from_index(0)));
    assert!(
        hand.iter()
            .any(|id| driver.state().object(*id).unwrap().card().name == "Llanowar Elf")
    );
    let graveyard = driver
        .state()
        .zone(Zone::Graveyard(mtg_core::ids::PlayerId::from_index(0)));
    assert!(
        graveyard
            .iter()
            .any(|id| driver.state().object(*id).unwrap().card().name == "Grizzly Bears"),
        "the Bears stay in the graveyard"
    );
}

#[rstest]
fn r5522_spider_man_needs_a_play_from_elsewhere(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] from_graveyard: bool,
) {
    // A land played from the hand never counts; a spell cast from the graveyard (flashback) does.
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Spider-Man 2099").unwrap()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .hand(0, [swamp()])
        .graveyard(0, [card("Faithless Looting").unwrap()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.play_land("Swamp").unwrap();
    if from_graveyard {
        driver.cast("Faithless Looting").unwrap();
        r5522_settle(&mut driver, &[], &mut |_, legal| {
            legal.iter().take(1).copied().collect()
        });
    }
    driver.idle_until(3, Step::End).unwrap();
    let foe = driver.target_player(1);
    let mut asked = false;
    r5522_settle(&mut driver, &[], &mut |_, _| {
        asked = true;
        vec![foe]
    });
    assert_eq!(asked, from_graveyard);
    let life = driver
        .state()
        .player(mtg_core::ids::PlayerId::from_index(1))
        .unwrap()
        .life();
    assert_eq!(life, if from_graveyard { 20 - 2 } else { 20 });
}

#[rstest]
#[case::another_permanent("Grizzly Bears", true)]
#[case::the_shield_itself("Shield", false)]
#[case::its_controller("", true)]
fn r5522_valkmira_taxes_what_targets_you_or_another_permanent(
    #[values(2, 3, 4)] seats: usize,
    #[case] at: &str,
    #[case] countered: bool,
) {
    let trigger = r5522_ability(
        "Reidane, God of the Worthy // Valkmira, Protector's Shield",
        1,
        1,
    );
    let scenario = Scenario::new(seats)
        .at(3, 1, Step::Main1)
        .battlefield(
            0,
            [
                ready(r5522_holder("Shield", CardType::Creature, trigger)),
                ready(bear()),
            ],
        )
        .battlefield(1, [ready(mountain())])
        .hand(1, [card("Lightning Bolt").unwrap()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = if at.is_empty() {
        driver.target_player(0)
    } else {
        driver.target_named(at).unwrap()
    };
    driver.cast_at("Lightning Bolt", &[target]).unwrap();
    r5522_settle(&mut driver, &[], &mut |_, legal| {
        legal.iter().take(1).copied().collect()
    });
    let life = driver
        .state()
        .player(mtg_core::ids::PlayerId::from_index(0))
        .unwrap()
        .life();
    match at {
        "Grizzly Bears" => assert_eq!(r5522_on_battlefield(&driver, "Grizzly Bears"), countered),
        "" => assert_eq!(life, if countered { 20 } else { 17 }),
        _ => {
            let shield = driver.id_of("Shield").unwrap();
            let damage = driver
                .state()
                .object(shield)
                .unwrap()
                .permanent()
                .unwrap()
                .damage();
            assert_eq!(damage, if countered { 0 } else { 3 });
        }
    }
}
