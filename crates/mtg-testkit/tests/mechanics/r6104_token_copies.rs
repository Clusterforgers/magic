//! Goal: the first token creation of a turn may become that many copies of the permanent Moonlit Meditation enchants,
//! of the creature Mirrormind Crown equips (only while it equips one) and, on Esix's controller's own turns, of a
//! creature other than Esix that player chooses; a declined or later creation that turn makes the tokens it would,
//! other players' tokens are not replaced, at two to four seats, and each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::ObjectKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, attached_to, ready};
use rstest::rstest;

use crate::common::{pid, spell};

/// A free instant: each player creates two 1/1 white Soldier creature tokens.
fn r6104_soldiers(name: &str) -> std::sync::Arc<mtg_cards::CardDefinition> {
    spell(
        name,
        CardType::Instant,
        "[]",
        r#"[{"CreateToken":{"who":"EachPlayer","token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}},"count":2}}]"#,
    )
}

/// Passes until the stack is empty, answering each yes-or-no question with the next of `yes` and each choice of a
/// creature with the one named `choose`; returns how many questions were asked.
fn r6104_settle(driver: &mut Driver, yes: &[bool], choose: &str) -> usize {
    let mut asked = 0;
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::YesNo(_) => {
                driver
                    .answer(Answer::YesNo(YesNoChoice(yes[asked])))
                    .unwrap();
                asked += 1;
            }
            DecisionKind::ChooseCards(options) => {
                let want = driver.id_of(choose).unwrap();
                assert!(options.from.contains(&want), "{choose} is offered");
                driver
                    .answer(Answer::ChooseCards(CardsChoice(vec![want])))
                    .unwrap();
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return asked;
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("the stack does not empty");
}

/// The names of the tokens `seat` controls, sorted.
fn r6104_tokens(driver: &Driver, seat: usize) -> Vec<String> {
    let state = driver.state();
    let mut names: Vec<String> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.kind() == ObjectKind::Token && o.controller() == pid(seat))
        .map(|o| o.card().name.clone())
        .collect();
    names.sort();
    names
}

fn r6104_replays(scenario: Scenario, driver: &Driver) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// Seat 0 controls Grizzly Bears with `source` attached to it (or beside it, unattached) and holds two token spells.
fn r6104_scenario(seats: usize, active: usize, source: &str, attached: bool) -> Scenario {
    let bears = ready(card("Grizzly Bears").unwrap());
    let source = ready(card(source).unwrap());
    let source = if attached {
        attached_to(source, 0, 0)
    } else {
        source
    };
    Scenario::new(seats)
        .at(2, active, Step::Main1)
        .battlefield(0, [bears, source])
        .hand(
            active,
            [r6104_soldiers("First Call"), r6104_soldiers("Second Call")],
        )
}

#[rstest]
fn r6104_the_first_creation_may_become_copies_of_the_attached_creature(
    #[values(2, 3, 4)] seats: usize,
    #[values("Moonlit Meditation", "Mirrormind Crown")] source: &str,
    #[values(true, false)] accept: bool,
) {
    let scenario = r6104_scenario(seats, 0, source, true);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.cast("First Call").unwrap();
    assert_eq!(r6104_settle(&mut driver, &[accept], ""), 1, "asked once");
    let first = if accept { "Grizzly Bears" } else { "Soldier" };
    assert_eq!(r6104_tokens(&driver, 0), [first, first]);
    for seat in 1..seats {
        assert_eq!(
            r6104_tokens(&driver, seat),
            ["Soldier", "Soldier"],
            "seat {seat}"
        );
    }
    // A later creation this turn is not replaced, whatever the first answer was (the ruling: no second choice).
    driver.cast("Second Call").unwrap();
    assert_eq!(r6104_settle(&mut driver, &[], ""), 0, "not asked again");
    let mut expected = vec![first, first, "Soldier", "Soldier"];
    expected.sort_unstable();
    assert_eq!(r6104_tokens(&driver, 0), expected);
    r6104_replays(scenario, &driver);
}

#[rstest]
fn r6104_an_unattached_crown_replaces_nothing(#[values(2, 3, 4)] seats: usize) {
    let scenario = r6104_scenario(seats, 0, "Mirrormind Crown", false);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.cast("First Call").unwrap();
    assert_eq!(r6104_settle(&mut driver, &[], ""), 0);
    assert_eq!(r6104_tokens(&driver, 0), ["Soldier", "Soldier"]);
    r6104_replays(scenario, &driver);
}

#[rstest]
fn r6104_esix_copies_a_chosen_creature_on_its_controllers_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] active: usize,
) {
    let scenario = Scenario::new(seats)
        .at(2, active, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Esix, Fractal Bloom").unwrap()),
                ready(card("Grizzly Bears").unwrap()),
            ],
        )
        .hand(
            active,
            [r6104_soldiers("First Call"), r6104_soldiers("Second Call")],
        );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.cast("First Call").unwrap();
    let own_turn = active == 0;
    let asked = r6104_settle(&mut driver, &[true], "Grizzly Bears");
    assert_eq!(asked, usize::from(own_turn));
    let first = if own_turn { "Grizzly Bears" } else { "Soldier" };
    assert_eq!(r6104_tokens(&driver, 0), [first, first]);
    for seat in 1..seats {
        assert_eq!(
            r6104_tokens(&driver, seat),
            ["Soldier", "Soldier"],
            "seat {seat}"
        );
    }
    driver.cast("Second Call").unwrap();
    assert_eq!(
        r6104_settle(&mut driver, &[], ""),
        0,
        "only the first creation"
    );
    r6104_replays(scenario, &driver);
}
