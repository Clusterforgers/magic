//! Goal: translated steps that remember what they drew or revealed play through: the cards a draw named are the ones a later step reads (and only those, none when no draw happened), and a revealed hand is the hand as it was when it was shown.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, island, plains};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::r3p3_runs::{settle, translated};

const EXCAVATION: &str = "Name:Dig\nManaCost:W\nTypes:Instant\nA:SP$ Draw | NumCards$ X | SubAbility$ DBDiscard | RememberDrawn$ AllReplaced\nSVar:DBDiscard:DB$ Discard | Defined$ You | Mode$ TgtChoose | NumCards$ Y | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nSVar:X:Count$ValidHand Card.YouOwn\nSVar:Y:Remembered$Amount\n";
const BONES: &str = "Name:Bones\nManaCost:W\nTypes:Sorcery\nA:SP$ Draw | NumCards$ 3 | RememberDrawn$ True | SubAbility$ DBDiscard\nSVar:DBDiscard:DB$ Discard | Defined$ You | Mode$ TgtChoose | DiscardValid$ Card.IsRemembered | NumCards$ 1 | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\n";
const ENVOY: &str = "Name:Envoy\nManaCost:W\nTypes:Sorcery\nA:SP$ Draw | Defined$ You | NumCards$ X | RememberDrawn$ True | SubAbility$ DBLife\nSVar:DBLife:DB$ GainLife | LifeAmount$ 3 | ConditionDefined$ Remembered | ConditionPresent$ Card | ConditionCompare$ EQ0 | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nSVar:X:Count$ValidBattlefield Creature.YouCtrl\n";

fn size(driver: &Driver, zone: Zone) -> usize {
    driver.state().zone(zone).len()
}

/// Casts `script` with `held` other cards in hand and a library of five, and returns the hand and graveyard sizes.
fn dig(script: &str, name: &str, types: &str, held: usize) -> (usize, usize, usize) {
    let card = translated(script, name, types, "{W}");
    let mut hand = vec![card];
    hand.extend((0..held).map(|_| bear()));
    let scenario = Scenario::new(2)
        .hand(0, hand)
        .battlefield(0, [ready(plains())])
        .library(0, [forest(), island(), forest(), island(), forest()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast(name).unwrap();
    settle(&mut driver);
    let me = PlayerId::from_index(0);
    let bears = driver
        .state()
        .zone(Zone::Hand(me))
        .iter()
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .is_some_and(|o| o.card().name == "Grizzly Bears")
        })
        .count();
    (
        size(&driver, Zone::Hand(me)),
        size(&driver, Zone::Graveyard(me)),
        bears,
    )
}

#[rstest]
#[case::as_many_discarded_as_drawn(2, (2, 3))]
#[case::nothing_drawn_nothing_discarded(0, (0, 1))]
fn the_number_drawn_is_the_number_discarded(#[case] held: usize, #[case] want: (usize, usize)) {
    let (hand, graveyard, _) = dig(EXCAVATION, "Dig", "Instant", held);
    assert_eq!((hand, graveyard), want);
}

#[test]
fn one_of_the_drawn_cards_is_discarded_not_one_of_the_old_hand() {
    let (hand, graveyard, bears) = dig(BONES, "Bones", "Sorcery", 2);
    // three cards drawn, one of them goes, and both bears stay
    assert_eq!((hand, graveyard, bears), (4, 2, 2));
}

fn envoy(creatures: usize) -> (i32, usize) {
    let card = translated(ENVOY, "Envoy", "Sorcery", "{W}");
    let bears = (0..creatures).map(|_| ready(bear()));
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .battlefield(0, std::iter::once(ready(plains())).chain(bears))
        .library(0, [forest(), island()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Envoy").unwrap();
    settle(&mut driver);
    let me = PlayerId::from_index(0);
    (
        driver.state().player(me).unwrap().life(),
        size(&driver, Zone::Hand(me)),
    )
}

#[rstest]
#[case::drew_so_no_life(1, (20, 1))]
#[case::did_not_draw_so_life(0, (23, 0))]
fn a_draw_of_nothing_remembers_nothing(#[case] creatures: usize, #[case] want: (i32, usize)) {
    assert_eq!(envoy(creatures), want);
}

const PEEK: &str = "Name:Peek\nManaCost:W\nTypes:Sorcery\nA:SP$ RevealHand | ValidTgts$ Player | RememberRevealed$ True | SubAbility$ Draw\nSVar:Draw:DB$ Draw | Defined$ Targeted | SubAbility$ Damage\nSVar:Damage:DB$ DealDamage | Defined$ Targeted | NumDmg$ X | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nSVar:X:Remembered$Valid Card.Creature\n";

#[rstest]
#[case::two_creatures_shown(vec![bear(), bear(), forest()], 18)]
#[case::no_creature_shown(vec![forest()], 20)]
fn a_revealed_hand_counts_the_cards_shown_not_the_one_drawn_after(
    #[case] hand: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
    #[case] life: i32,
) {
    let card = translated(PEEK, "Peek", "Sorcery", "{W}");
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .hand(1, hand)
        .battlefield(0, [ready(plains())])
        .library(1, [bear(), bear()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_player(1);
    driver.cast_at("Peek", &[target]).unwrap();
    settle(&mut driver);
    let foe = driver.state().player(PlayerId::from_index(1)).unwrap();
    assert_eq!(foe.life(), life);
}

fn storm(damage: u32) -> String {
    format!(
        "Name:Storm\nManaCost:R\nTypes:Sorcery\nA:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ {damage} | ExcessDamage$ TargetedController\n"
    )
}

#[rstest]
#[case::short_of_lethal(1, 20, true)]
#[case::exactly_lethal(2, 20, false)]
#[case::three_beyond(5, 17, false)]
fn excess_damage_goes_to_the_controller_of_the_creature(
    #[case] damage: u32,
    #[case] life: i32,
    #[case] survives: bool,
) {
    let card = translated(&storm(damage), "Storm", "Sorcery", "{R}");
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .battlefield(0, [ready(mtg_testkit::cards::mountain())])
        .battlefield(1, [ready(bear())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_named("Grizzly Bears").unwrap();
    driver.cast_at("Storm", &[target]).unwrap();
    settle(&mut driver);
    let foe = PlayerId::from_index(1);
    assert_eq!(driver.state().player(foe).unwrap().life(), life);
    let alive = driver.state().zone(Zone::Battlefield).iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|o| o.card().name == "Grizzly Bears")
    });
    assert_eq!(alive, survives);
}

const SIPHON: &str = "Name:Siphon\nManaCost:R\nTypes:Sorcery\nA:SP$ DealDamage | ValidTgts$ Creature,Planeswalker | NumDmg$ 5 | ExcessSVar$ Excess | SubAbility$ Gain\nSVar:Gain:DB$ GainLife | LifeAmount$ Excess | ConditionCheckSVar$ Excess\n";

#[rstest]
#[case::creature("Grizzly Bears", 23)]
#[case::planeswalker_of_three_loyalty("Walker", 22)]
fn the_excess_damage_is_the_life_gained(#[case] victim: &str, #[case] life: i32) {
    let card = translated(SIPHON, "Siphon", "Sorcery", "{R}");
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .battlefield(0, [ready(mtg_testkit::cards::mountain())])
        .battlefield(
            1,
            [
                ready(bear()),
                ready(mtg_testkit::cards::walker("Walker", 2)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_named(victim).unwrap();
    driver.cast_at("Siphon", &[target]).unwrap();
    settle(&mut driver);
    assert_eq!(
        driver
            .state()
            .player(PlayerId::from_index(0))
            .unwrap()
            .life(),
        life
    );
}

#[test]
fn damage_that_is_not_beyond_lethal_gains_nothing() {
    let card = translated(
        &SIPHON.replace("NumDmg$ 5", "NumDmg$ 2"),
        "Siphon",
        "Sorcery",
        "{R}",
    );
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .battlefield(0, [ready(mtg_testkit::cards::mountain())])
        .battlefield(1, [ready(bear())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_named("Grizzly Bears").unwrap();
    driver.cast_at("Siphon", &[target]).unwrap();
    settle(&mut driver);
    assert_eq!(
        driver
            .state()
            .player(PlayerId::from_index(0))
            .unwrap()
            .life(),
        20
    );
}
