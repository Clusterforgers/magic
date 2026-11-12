//! Goal: the party cards of the corpus work as printed at two, three and four seats: Squad Commander makes a token for each creature in its controller's party, Burakos is all four roles and drains the defending player and makes Treasures for them, Nalia puts counters on a full party at the beginning of combat and not on three roles and casts party spells from the top of the library, and Tazri costs {1} less for each creature in the party and finds party cards among the top six.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{island, mountain, plains, swamp};
use mtg_testkit::{Driver, Scenario, ready, typed_creature};
use rstest::rstest;

use crate::common::{names, pid};

const SQUAD: &str = "Squad Commander";
const BURAKOS: &str = "Burakos, Party Leader";
const NALIA: &str = "Nalia de'Arnise";
const TAZRI: &str = "Tazri, Beacon of Unity";

/// A creature of these types, ready.
fn member(name: &str, kinds: &[&str]) -> mtg_engine::setup::PresetPermanent {
    ready(typed_creature(name, 0, &[], kinds, &[]))
}

/// The party members of seat 0 for a party of `size` others, each of a role of its own, and an Elf that is none.
fn others(size: usize) -> Vec<mtg_engine::setup::PresetPermanent> {
    let roles = ["Cleric", "Rogue", "Wizard", "Warrior"];
    let mut members: Vec<_> = roles[..size]
        .iter()
        .map(|role| member(&format!("Member {role}"), &[role]))
        .collect();
    members.push(member("Bystander Elf", &["Elf"]));
    members
}

/// `seats` players with libraries of Plains, so nothing runs out.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..16).map(|_| plains()));
    }
    scenario
}

fn count(driver: &Driver, name: &str) -> usize {
    names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|found| *found == name)
        .count()
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn resolve_all(driver: &mut Driver) {
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
}

#[rstest]
fn squad_commander_makes_a_token_for_each_creature_in_the_party(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] size: usize,
) {
    let mut battlefield = others(size);
    battlefield.extend((0..4).map(|_| ready(plains())));
    let scenario = table(seats)
        .hand_named(0, [SQUAD])
        .battlefield(0, battlefield);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast_tapping(SQUAD, &["Plains"; 4]).unwrap();
    resolve_all(&mut driver);
    // Squad Commander is a Warrior itself, so the party is one larger than the others; the Elf is no role.
    let party = if size == 4 { 4 } else { size + 1 };
    assert_eq!(count(&driver, "Kor Warrior"), party);
}

#[rstest]
fn burakos_fills_the_role_the_others_leave_and_drains_the_defending_player_for_the_party(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] defender: usize,
    #[values(0, 1, 2, 3)] size: usize,
) {
    if defender >= seats {
        return;
    }
    // One creature fills one role however many it has: Burakos alone is a party of one, and each creature of a role of
    // its own beside it adds one up to four.
    let roles = ["Cleric", "Rogue", "Warrior"];
    let mut battlefield = vec![ready(mtg_testkit::corpus::card(BURAKOS).unwrap())];
    battlefield.extend(
        roles[..size]
            .iter()
            .map(|role| member(&format!("Member {role}"), &[role])),
    );
    let scenario = table(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, battlefield);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.idle_until(1, Step::DeclareAttackers).unwrap();
    driver.attack(&[(BURAKOS, defender)]).unwrap();
    resolve_all(&mut driver);
    let party = i32::try_from(size).unwrap() + 1;
    assert_eq!(life(&driver, defender), 20 - party);
    for other in (1..seats).filter(|seat| *seat != defender) {
        assert_eq!(life(&driver, other), 20, "only the defending player");
    }
    assert_eq!(count(&driver, "Treasure"), usize::try_from(party).unwrap());
}

#[rstest]
fn nalia_counts_a_full_party_at_the_beginning_of_combat(
    #[values(2, 3, 4)] seats: usize,
    #[values(2, 3)] others_count: usize,
) {
    // Nalia is a Rogue: with a Cleric, a Warrior and a Wizard the party is full; without the Wizard it is not.
    let mut battlefield = vec![ready(mtg_testkit::corpus::card(NALIA).unwrap())];
    let roles = ["Cleric", "Warrior", "Wizard"];
    battlefield.extend(
        roles[..others_count]
            .iter()
            .map(|role| member(&format!("Member {role}"), &[role])),
    );
    let scenario = table(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, battlefield);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.idle_until(1, Step::BeginCombat).unwrap();
    resolve_all(&mut driver);
    let counters = |name: &str| {
        let id = driver.id_of(name).unwrap();
        mtg_engine::characteristics::characteristics(driver.state(), id)
            .unwrap()
            .power
            .unwrap()
    };
    let full = others_count == 3;
    assert_eq!(
        counters("Member Cleric"),
        1 + i32::from(full),
        "a 1/1 and the counter"
    );
    assert_eq!(counters("Member Warrior"), 1 + i32::from(full));
}

#[rstest]
fn nalia_casts_party_spells_from_the_top_of_the_library_and_no_others(
    #[values(2, 3, 4)] seats: usize,
    #[values("Wizard", "Elf")] top: &str,
) {
    let library = std::iter::once(typed_creature("Top Card", 1, &[], &[top], &[]))
        .chain((0..8).map(|_| plains()));
    let scenario = table(seats).library(0, library).battlefield(
        0,
        [
            ready(mtg_testkit::corpus::card(NALIA).unwrap()),
            ready(plains()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let cast = driver.cast_tapping("Top Card", &["Plains"]);
    assert_eq!(cast.is_ok(), top == "Wizard", "{top}");
    if top == "Wizard" {
        resolve_all(&mut driver);
        assert_eq!(count(&driver, "Top Card"), 1);
    }
}

#[rstest]
fn tazri_costs_one_less_for_each_creature_in_the_party(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3, 4)] size: usize,
) {
    let roles = ["Cleric", "Rogue", "Wizard", "Warrior"];
    let mut battlefield: Vec<_> = roles[..size]
        .iter()
        .map(|role| member(&format!("Member {role}"), &[role]))
        .collect();
    battlefield.extend((0..5).map(|_| ready(plains())));
    let scenario = table(seats)
        .hand_named(0, [TAZRI])
        .battlefield(0, battlefield);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    // {4}{W} less one for each role: the cost in lands is five less the party.
    let lands = vec!["Plains"; 5 - size.min(4)];
    let enough = driver.cast_tapping(TAZRI, &lands);
    assert!(
        enough.is_ok(),
        "{size} roles, {} lands: {enough:?}",
        lands.len()
    );
    resolve_all(&mut driver);
    assert_eq!(count(&driver, TAZRI), 1);
}

#[rstest]
fn tazri_one_land_short_is_not_enough(#[values(2, 3, 4)] seats: usize) {
    let mut battlefield: Vec<_> = ["Cleric", "Rogue"]
        .iter()
        .map(|role| member(&format!("Member {role}"), &[role]))
        .collect();
    battlefield.extend((0..3).map(|_| ready(plains())));
    let scenario = table(seats)
        .hand_named(0, [TAZRI])
        .battlefield(0, battlefield);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    assert!(driver.cast_tapping(TAZRI, &["Plains"; 2]).is_err());
}

#[rstest]
fn tazri_finds_up_to_two_party_cards_among_the_top_six_and_buries_the_rest(
    #[values(2, 3, 4)] seats: usize,
) {
    let top = [
        ("Found Cleric", "Cleric"),
        ("Passed Elf", "Elf"),
        ("Found Wizard", "Wizard"),
        ("Extra Rogue", "Rogue"),
        ("Passed Elf Too", "Elf"),
        ("Found Ally", "Ally"),
    ];
    let library = top
        .iter()
        .map(|(name, kind)| typed_creature(name, 1, &[], &[kind], &[]))
        .chain((0..10).map(|_| plains()));
    let lands = [island(), swamp(), mountain(), mtg_testkit::cards::forest()];
    let scenario = table(seats).library(0, library).battlefield(
        0,
        std::iter::once(ready(mtg_testkit::corpus::card(TAZRI).unwrap()))
            .chain(lands.into_iter().map(ready)),
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation(TAZRI).unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    resolve_all_until_cards(&mut driver);
    let decision = driver.decision().unwrap().clone();
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!("up to two cards are chosen, not {:?}", decision.kind);
    };
    // The Elves are no party cards: they are not offered.
    let offered: Vec<String> = options
        .from
        .iter()
        .map(|id| names_of(&driver, *id))
        .collect();
    assert!(
        offered.iter().all(|name| !name.contains("Elf")),
        "{offered:?}"
    );
    assert_eq!(offered.len(), 4, "{offered:?}");
    let chosen: Vec<ObjectId> = options
        .from
        .iter()
        .copied()
        .filter(|id| names_of(&driver, *id).starts_with("Found"))
        .take(2)
        .collect();
    driver
        .answer(Answer::ChooseCards(CardsChoice(chosen)))
        .unwrap();
    resolve_all(&mut driver);
    let hand = names(driver.state(), Zone::Hand(pid(0)));
    assert_eq!(hand.len(), 2, "{hand:?}");
    assert!(hand.iter().all(|name| name.starts_with("Found")));
    assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), 14);
}

fn names_of(driver: &Driver, id: ObjectId) -> String {
    crate::common::name_of(driver.state(), id)
}

/// Passes until a decision about cards is asked or the stack is empty.
fn resolve_all_until_cards(driver: &mut Driver) {
    while !driver.state().stack().is_empty() {
        if matches!(
            driver.decision().map(|decision| &decision.kind),
            Ok(DecisionKind::ChooseCards(_))
        ) {
            return;
        }
        driver.pass().unwrap();
    }
}
