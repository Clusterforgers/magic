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
