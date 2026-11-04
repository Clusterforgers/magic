//! Goal: a library search for another player and an optional move run the way the card reads when the translated card is played.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, plains};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::r3p3_runs::translated;

const PATH: &str = "Name:Path to Exile\nManaCost:W\nTypes:Instant\nA:SP$ ChangeZone | Origin$ Battlefield | Destination$ Exile | ValidTgts$ Creature | SubAbility$ DBChange | SpellDescription$ Exile target creature. Its controller may search their library for a basic land card, put that card onto the battlefield tapped, then shuffle.\nSVar:DBChange:DB$ ChangeZone | Optional$ True | Origin$ Library | Destination$ Battlefield | Tapped$ True | ChangeType$ Land.Basic | ChangeTypeDesc$ basic land | DefinedPlayer$ TargetedController | ShuffleNonMandatory$ True | StackDescription$ None\n";

/// Plays Path to Exile on the opponent's bear, answering every question `yes`; the opponent's battlefield lands and
/// tapped state afterwards, and whether the bear is in exile.
fn path(yes: bool) -> (usize, usize, bool) {
    let card = translated(PATH, "Path to Exile", "Instant", "{W}");
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .battlefield(0, [ready(plains())])
        .battlefield(1, [ready(bear())])
        .library(1, [forest(), forest()])
        .library(0, [forest()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_named("Grizzly Bears").unwrap();
    driver.cast_at("Path to Exile", &[target]).unwrap();
    for _ in 0..100 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            DecisionKind::YesNo(_) => Answer::YesNo(mtg_engine::choose::YesNoChoice(yes)),
            DecisionKind::ChooseCards(o) => Answer::ChooseCards(mtg_engine::choose::CardsChoice(
                o.from.iter().copied().take(o.max as usize).collect(),
            )),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    let state = driver.state();
    let them = PlayerId::from_index(1);
    let lands: Vec<_> = state
        .zone(Zone::Battlefield)
        .iter()
        .map(|id| state.object(*id).unwrap())
        .filter(|object| object.card().name == "Forest")
        .collect();
    let mine = lands
        .iter()
        .filter(|object| object.controller() == PlayerId::from_index(0))
        .count();
    let theirs = lands
        .iter()
        .filter(|object| object.controller() == them)
        .count();
    let exiled = state
        .zone(Zone::Exile)
        .iter()
        .any(|id| state.object(*id).unwrap().card().name == "Grizzly Bears");
    (mine, theirs, exiled)
}

#[rstest]
#[case::the_controller_takes_the_land(true, (0, 1, true))]
#[case::the_controller_declines(false, (0, 0, true))]
fn path_to_exile_gives_the_creatures_controller_a_land(
    #[case] yes: bool,
    #[case] expected: (usize, usize, bool),
) {
    assert_eq!(path(yes), expected);
}

const FLICKER: &str = "Name:Flick\nManaCost:W\nTypes:Instant\nA:SP$ ChangeZone | ValidTgts$ Creature | Origin$ Battlefield | Destination$ Exile | RememberChanged$ True | SubAbility$ DBReturn\nSVar:DBReturn:DB$ ChangeZone | Defined$ Remembered | Origin$ All | Destination$ Battlefield | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\n";
const SWAP: &str = "Name:Flick Swap\nManaCost:W\nTypes:Instant\nA:SP$ ChangeZone | ValidTgts$ Creature | Origin$ Battlefield | Destination$ Exile | RememberChanged$ True | SubAbility$ DBReturn\nSVar:DBReturn:DB$ ChangeZone | Defined$ Remembered | Origin$ All | Destination$ Battlefield | GainControl$ True | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\n";

/// The player that controls the opponent's bear after the spell, and how many bears are on the battlefield.
fn flicker(script: &str, name: &str) -> (usize, usize) {
    let card = translated(script, name, "Instant", "{W}");
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .battlefield(0, [ready(plains())])
        .battlefield(1, [ready(bear())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_named("Grizzly Bears").unwrap();
    driver.cast_at(name, &[target]).unwrap();
    crate::r3p3_runs::settle(&mut driver);
    let state = driver.state();
    let bears: Vec<_> = state
        .zone(Zone::Battlefield)
        .iter()
        .map(|id| state.object(*id).unwrap())
        .filter(|object| object.card().name == "Grizzly Bears")
        .collect();
    (
        bears.first().map_or(9, |bear| bear.controller().index()),
        bears.len(),
    )
}

#[rstest]
#[case::back_under_its_owner(FLICKER, "Flick", (1, 1))]
#[case::under_your_control(SWAP, "Flick Swap", (0, 1))]
fn a_flicker_returns_the_exiled_card(
    #[case] script: &str,
    #[case] name: &str,
    #[case] expected: (usize, usize),
) {
    assert_eq!(flicker(script, name), expected);
}

const SNIP: &str = "Name:Snip\nManaCost:B\nTypes:Sorcery\nA:SP$ LoseLife | ValidTgts$ Opponent | LifeAmount$ 1 | SubAbility$ DBExile | SpellDescription$ Target opponent loses 1 life. You choose a nonland card from their hand and exile it.\nSVar:DBExile:DB$ ChangeZone | Origin$ Hand | Destination$ Exile | DefinedPlayer$ Targeted | Chooser$ You | ChangeType$ Card.nonLand | ChangeNum$ 1 | Mandatory$ True\n";

#[test]
fn a_choice_from_the_hand_of_the_targeted_player_takes_only_a_nonland_card_of_theirs() {
    use mtg_testkit::cards::swamp;
    let card = translated(SNIP, "Snip", "Sorcery", "{B}");
    let scenario = Scenario::new(2)
        .hand(0, [card, bear()])
        .hand(1, [forest(), bear()])
        .battlefield(0, [ready(swamp())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_player(1);
    driver.cast_at("Snip", &[target]).unwrap();
    crate::r3p3_runs::settle(&mut driver);
    let state = driver.state();
    let names = |zone: Zone| -> Vec<String> {
        state
            .zone(zone)
            .iter()
            .map(|id| state.object(*id).unwrap().card().name.clone())
            .collect()
    };
    assert_eq!(names(Zone::Exile), ["Grizzly Bears"]);
    assert_eq!(names(Zone::Hand(PlayerId::from_index(1))), ["Forest"]);
    assert_eq!(
        names(Zone::Hand(PlayerId::from_index(0))),
        ["Grizzly Bears"]
    );
}

const PAINFUL: &str = "Name:Painful\nManaCost:B\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Hand | Destination$ Library | LibraryPosition$ 0 | ValidTgts$ Opponent | ChangeType$ Card | ChangeNum$ 1 | Mandatory$ True | SpellDescription$ Look at target opponent's hand and choose a card from it. Put that card on top of their library.\n";
const IMP: &str = "Name:Imp Spell\nManaCost:B\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Hand | Destination$ Library | LibraryPosition$ 0 | ValidTgts$ Opponent | Chooser$ Targeted | ChangeType$ Card | ChangeNum$ 1 | Mandatory$ True | SpellDescription$ Target opponent puts a card from their hand on top of their library.\n";
const FAVOR: &str = "Name:Favor\nManaCost:G\nTypes:Sorcery\nA:SP$ ChangeZone | ValidTgts$ Player | Origin$ Library | Destination$ Battlefield | Tapped$ True | ChangeType$ Land.Basic | Chooser$ Targeted | SpellDescription$ Target player searches their library for a basic land card, puts it onto the battlefield tapped, then shuffles.\n";

/// The seat that is asked to choose cards when `spell` is cast at the opponent (seat 1).
fn who_chooses(script: &str, name: &str) -> (usize, Vec<String>) {
    use mtg_testkit::cards::swamp;
    let card = translated(script, name, "Sorcery", "{B}");
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .hand(1, [bear(), plains()])
        .library(1, [forest(), forest()])
        .battlefield(0, [ready(swamp()), ready(forest())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_player(1);
    driver.cast_at(name, &[target]).unwrap();
    let mut chooser = 9;
    for _ in 0..100 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            DecisionKind::YesNo(_) => Answer::YesNo(mtg_engine::choose::YesNoChoice(true)),
            DecisionKind::ChooseCards(o) => {
                chooser = decision.player.index();
                Answer::ChooseCards(mtg_engine::choose::CardsChoice(
                    o.from.iter().copied().take(o.max as usize).collect(),
                ))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    let state = driver.state();
    let top = |zone: Zone| -> Vec<String> {
        state
            .zone(zone)
            .iter()
            .map(|id| state.object(*id).unwrap().card().name.clone())
            .collect()
    };
    let mut seen = top(Zone::Hand(PlayerId::from_index(1)));
    seen.extend(
        top(Zone::Battlefield)
            .into_iter()
            .filter(|n| n == "Forest")
            .map(|n| format!("battlefield {n}")),
    );
    (chooser, seen)
}

#[rstest]
#[case::the_caster_chooses_a_card_of_the_opponents_hand(PAINFUL, "Painful", 0)]
#[case::the_opponent_chooses_their_own_card(IMP, "Imp Spell", 1)]
fn the_chooser_of_a_card_from_a_targeted_hand_is_the_one_the_card_names(
    #[case] script: &str,
    #[case] name: &str,
    #[case] chooser: usize,
) {
    let (who, hand) = who_chooses(script, name);
    assert_eq!(who, chooser);
    let in_hand = hand
        .iter()
        .filter(|n| !n.starts_with("battlefield"))
        .count();
    assert_eq!(in_hand, 1, "one of the two cards left the hand: {hand:?}");
}

#[test]
fn the_targeted_player_searches_their_own_library_and_the_caster_does_not() {
    let (who, seen) = who_chooses(FAVOR, "Favor");
    assert_eq!(who, 1, "the targeted player picks the land");
    let lands = seen.iter().filter(|n| n.starts_with("battlefield")).count();
    assert_eq!(
        lands, 2,
        "the caster's own Forest and the one found: {seen:?}"
    );
}

const WARDEN: &str = "Name:Warden\nManaCost:2\nTypes:Artifact\nA:AB$ ChangeZone | Cost$ T | Origin$ Battlefield | Destination$ Exile | ValidTgts$ Creature.OppCtrl | Duration$ UntilHostLeavesPlay | SpellDescription$ Exile target creature an opponent controls until Warden leaves the battlefield.\n";
const SMASH: &str = "Name:Smash\nManaCost:R\nTypes:Instant\nA:SP$ Destroy | ValidTgts$ Artifact\n";

#[test]
fn an_exile_until_the_source_leaves_returns_the_creature_when_it_goes() {
    use mtg_testkit::cards::mountain;
    let scenario = Scenario::new(2)
        .battlefield(
            0,
            [
                ready(translated(WARDEN, "Warden", "Artifact", "{2}")),
                ready(mountain()),
            ],
        )
        .hand(0, [translated(SMASH, "Smash", "Instant", "{R}")])
        .battlefield(1, [ready(bear())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Warden").unwrap();
    let target = driver.target_named("Grizzly Bears").unwrap();
    driver.choose_targets(&[target]).unwrap();
    crate::r3p3_runs::settle(&mut driver);
    let count = |driver: &Driver, zone: Zone| {
        let state = driver.state();
        state
            .zone(zone)
            .iter()
            .filter(|id| state.object(**id).unwrap().card().name == "Grizzly Bears")
            .count()
    };
    assert_eq!(count(&driver, Zone::Exile), 1, "held while Warden stays");
    assert_eq!(count(&driver, Zone::Battlefield), 0);
    let warden = driver.target_named("Warden").unwrap();
    driver.cast_at("Smash", &[warden]).unwrap();
    crate::r3p3_runs::settle(&mut driver);
    assert_eq!(count(&driver, Zone::Exile), 0, "back once Warden is gone");
    assert_eq!(count(&driver, Zone::Battlefield), 1);
}

const TWIST: &str = "Name:Twist\nManaCost:U\nTypes:Sorcery\nA:SP$ ChangeZoneAll | ChangeType$ Card | Origin$ Hand,Graveyard | Destination$ Library | Shuffle$ True | UseAllOriginZones$ True | SpellDescription$ Each player shuffles their hand and graveyard into their library.\n";
const HAG: &str = "Name:Hag\nManaCost:U\nTypes:Sorcery\nA:SP$ ChangeZoneAll | Origin$ Graveyard | Destination$ Exile | Defined$ Opponent | ChangeType$ Card | SpellDescription$ Exile each opponent's graveyard.\n";

/// Cards by seat in (hand, graveyard, library, exile) after `script` resolves with two cards in each hand and graveyard.
fn zones_after(script: &str, name: &str) -> Vec<[usize; 4]> {
    use mtg_testkit::cards::island;
    let scenario = Scenario::new(2)
        .hand(0, [translated(script, name, "Sorcery", "{U}"), bear()])
        .graveyard(0, [bear(), bear()])
        .hand(1, [bear(), bear()])
        .graveyard(1, [bear(), bear()])
        .library(0, [forest()])
        .library(1, [forest()])
        .battlefield(0, [ready(island())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast(name).unwrap();
    crate::r3p3_runs::settle(&mut driver);
    let state = driver.state();
    (0..2)
        .map(|seat| {
            let me = PlayerId::from_index(seat);
            [
                state.zone(Zone::Hand(me)).len(),
                state.zone(Zone::Graveyard(me)).len(),
                state.zone(Zone::Library(me)).len(),
                state
                    .zone(Zone::Exile)
                    .iter()
                    .filter(|id| state.object(**id).unwrap().owner() == me)
                    .count(),
            ]
        })
        .collect()
}

#[rstest]
#[case::every_hand_and_graveyard_goes_into_its_owners_library(TWIST, "Twist", [[0, 1, 4, 0], [0, 0, 5, 0]])]
#[case::only_the_opponents_graveyard_is_exiled(HAG, "Hag", [[1, 3, 1, 0], [2, 0, 1, 2]])]
fn a_move_of_all_cards_takes_the_zones_and_players_the_card_names(
    #[case] script: &str,
    #[case] name: &str,
    #[case] expected: [[usize; 4]; 2],
) {
    assert_eq!(zones_after(script, name), expected);
}

const DIG: &str = "Name:Dig Spell\nManaCost:U\nTypes:Sorcery\nA:SP$ Dig | DigNum$ 3 | ChangeNum$ 1 | DestinationZone$ Hand | LibraryPosition$ 0 | RestRandomOrder$ True | SpellDescription$ Look at the top three cards of your library. Put one of them into your hand and the rest on the bottom of your library in a random order.\n";
const BURY: &str = "Name:Bury Spell\nManaCost:U\nTypes:Sorcery\nA:SP$ Dig | DigNum$ 2 | ChangeNum$ Any | DestinationZone$ Library | SpellDescription$ Look at the top two cards of your library. Put any number of them on the bottom of your library.\n";

/// The names in the library of seat 0 from the top, and in the hand, after `script` resolves over a four-card library.
fn dig_run(script: &str, name: &str) -> (Vec<String>, Vec<String>) {
    use mtg_testkit::cards::{free_creature, island};
    let library = ["A", "B", "C", "D"].map(|n| free_creature(n, 1, 1));
    let scenario = Scenario::new(2)
        .hand(0, [translated(script, name, "Sorcery", "{U}")])
        .library(0, library)
        .library(1, [forest()])
        .battlefield(0, [ready(island())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast(name).unwrap();
    crate::r3p3_runs::settle(&mut driver);
    let state = driver.state();
    let names = |zone: Zone| -> Vec<String> {
        state
            .zone(zone)
            .iter()
            .map(|id| state.object(*id).unwrap().card().name.clone())
            .collect()
    };
    let me = PlayerId::from_index(0);
    (names(Zone::Library(me)), names(Zone::Hand(me)))
}

#[test]
fn a_library_position_for_another_destination_leaves_the_rest_at_the_bottom() {
    let (library, hand) = dig_run(DIG, "Dig Spell");
    assert_eq!(hand.len(), 1, "{hand:?}");
    assert_eq!(library.len(), 3, "{library:?}");
    // The zone lists the bottom first.
    assert_eq!(
        library[2], "D",
        "the card not looked at stays on top: {library:?}"
    );
}

#[test]
fn a_dig_to_the_library_without_a_position_goes_to_the_bottom() {
    let (library, hand) = dig_run(BURY, "Bury Spell");
    assert!(hand.is_empty(), "{hand:?}");
    assert_eq!(library.len(), 4);
    assert_eq!(
        library[2..],
        ["D", "C"],
        "the two not looked at stay on top: {library:?}"
    );
}
