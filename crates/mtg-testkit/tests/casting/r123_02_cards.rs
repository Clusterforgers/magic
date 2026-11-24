//! Goal: the r123-02 cards play whole, at two to four seats: Unstable Glyphbridge cast keeps one creature with power 2 or
//! less of each player who has one (two of one player are refused) and destroys the rest; Sandswirl Wanderglyph's
//! rules keep an opponent who cast a spell in their turn from attacking you or your planeswalkers, not the other
//! players, and keep an opponent who attacked you or your planeswalker from casting spells; Tazri's granted mana
//! ability taps only a creature with another activated ability (cycling counts); each game replays to its state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost, creature, instant, island, pinger, plains, walker};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// The back face's trigger: "Whenever an opponent casts a spell during their turn, they can't attack you or
/// planeswalkers you control this turn."
const R123_NO_ATTACK: &str = r#"{"Triggered":{"when":{"SpellCast":{"by":"Opponents"}},"if":{"AtTrigger":{"Compare":{"left":{"Players":{"PlayersWhere":{"who":"TriggeringPlayer","test":"YourTurn"}}},"op":"Ge","right":1}}},"effects":[{"Rule":{"rule":{"Cant":{"who":"TriggeringPlayer","action":{"R118Attack":{"you":true,"walkers":true}}}},"duration":"UntilEndOfTurn"}}]}}"#;

/// The back face's static: "Each opponent who attacked you or a planeswalker you control this turn can't cast spells."
const R123_NO_CAST: &str = r#"{"Rule":{"rule":{"Cant":{"who":{"PlayersWhere":{"who":"Opponents","test":{"R123AttackedThisTurn":"Asker"}}},"action":{"Cast":{}}}}}}"#;

fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, vec![island(); 10])
    })
}

/// Passes and answers passively until a decision of `kind` comes, which is returned.
fn until(driver: &mut Driver, kind: &str) -> mtg_engine::decision::Decision {
    for _ in 0..300 {
        let decision = driver.decision().unwrap().clone();
        if decision.kind.name() == kind {
            return decision;
        }
        let answer = match &decision.kind {
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    panic!("no {kind} came");
}

fn names_on_battlefield(driver: &Driver, seat: usize) -> Vec<String> {
    let mut names: Vec<String> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.controller() == PlayerId(u64::try_from(seat).unwrap()))
        .map(|object| object.card().name.clone())
        .filter(|name| name == "Bear" || name == "Giant")
        .collect();
    names.sort();
    names
}

#[rstest]
fn glyphbridge_keeps_one_small_creature_of_each_player(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] last_has_none: bool,
) {
    let giant = || ready(creature("Giant", 0, &[], 3, 3));
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    for seat in 0..seats {
        let mut mine = vec![giant()];
        if !(last_has_none && seat == seats - 1) {
            mine.push(ready(bear()));
        }
        if seat == 0 {
            mine.extend((0..5).map(|_| ready(plains())));
        }
        scenario = scenario.battlefield(seat, mine);
    }
    let scenario = scenario.hand_named(0, ["Unstable Glyphbridge // Sandswirl Wanderglyph"]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    driver
        .cast("Unstable Glyphbridge // Sandswirl Wanderglyph")
        .unwrap();
    let decision = until(&mut driver, "choose-cards");
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        unreachable!()
    };
    let holders = if last_has_none { seats - 1 } else { seats };
    assert_eq!(options.min as usize, holders);
    let bears: Vec<ObjectId> = options.from.clone();
    assert_eq!(bears.len(), holders, "only the creatures with power 2 or less");
    // Two creatures of one player are not a choice for each player.
    let two_of_one = driver
        .answer(Answer::ChooseCards(CardsChoice(vec![bears[0], bears[0]])));
    assert!(two_of_one.is_err());
    driver
        .answer(Answer::ChooseCards(CardsChoice(bears)))
        .unwrap();
    until(&mut driver, "priority");
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    for seat in 0..seats {
        let kept = if last_has_none && seat == seats - 1 {
            Vec::<String>::new()
        } else {
            vec!["Bear".to_owned()]
        };
        assert_eq!(names_on_battlefield(&driver, seat), kept, "seat {seat}");
    }
    replays(&driver);
}

/// Seat 0's Wanderglyph with the back face's rules, seat 1 to move in its first main phase with a ready Bear, an
/// instant and a planeswalker of seat 0.
fn wanderglyph_table(seats: usize) -> Driver {
    let mut glyph = CardDefinition::new("Wanderglyph")
        .with_type(CardType::Creature)
        .with_mana_cost(cost(0, &[]))
        .with_power_toughness(5, 3);
    for text in [R123_NO_ATTACK, R123_NO_CAST] {
        glyph = glyph.with_rules(serde_json::from_str::<Ability>(text).unwrap());
    }
    let scenario = Scenario::new(seats)
        .at(2, 1, Step::Main1)
        .battlefield(0, [ready(Arc::new(glyph)), ready(walker("Jace", 3))])
        .battlefield(1, [ready(bear()), ready(creature("Giant", 0, &[], 3, 3))])
        .hand(1, [instant("Shock", 0, &[]), instant("Bolt", 0, &[])]);
    Driver::from_scenario(libraries(scenario, seats)).unwrap()
}

/// Whether seat 1's Bear may attack the player at `seat`, or Jace when `seat` is `None`.
fn bear_attacks(mut driver: Driver, seat: Option<usize>) -> bool {
    until(&mut driver, "declare-attackers");
    match seat {
        Some(seat) => driver.attack(&[("Bear", seat)]).is_ok(),
        None => driver.attack_planeswalkers(&[("Bear", "Jace")]).is_ok(),
    }
}

#[rstest]
fn wanderglyph_keeps_a_caster_from_attacking_you(#[values(2, 3, 4)] seats: usize) {
    assert!(bear_attacks(wanderglyph_table(seats), Some(0)), "before a spell");
    let cast = || {
        let mut driver = wanderglyph_table(seats);
        driver.cast("Shock").unwrap();
        while !driver.state().stack().is_empty() {
            driver.pass().unwrap();
        }
        replays(&driver);
        driver
    };
    assert!(!bear_attacks(cast(), Some(0)));
    assert!(!bear_attacks(cast(), None), "nor your planeswalkers");
    if seats > 2 {
        assert!(bear_attacks(cast(), Some(seats - 1)), "the others stay open");
    }
}

#[rstest]
fn wanderglyph_keeps_an_attacker_from_casting(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] at: usize,
) {
    if at == 2 && seats == 2 {
        return;
    }
    let mut driver = wanderglyph_table(seats);
    until(&mut driver, "declare-attackers");
    match at {
        0 => driver.attack(&[("Bear", 0)]).unwrap(),
        1 => driver.attack_planeswalkers(&[("Bear", "Jace")]).unwrap(),
        _ => driver.attack(&[("Bear", seats - 1)]).unwrap(),
    };
    driver.idle_until(2, Step::Main2).unwrap();
    let cast = driver.cast("Bolt");
    assert_eq!(cast.is_ok(), at == 2, "only an attack elsewhere leaves casting open");
    replays(&driver);
}

#[rstest]
#[case::vanilla("Bear", false)]
#[case::activated("Pinger", true)]
#[case::cycling("Archfiend of Ifnir", true)]
fn tazri_taps_a_creature_with_another_activated_ability(
    #[values(2, 4)] seats: usize,
    #[case] name: &str,
    #[case] taps: bool,
) {
    let other = match name {
        "Bear" => bear(),
        "Pinger" => pinger("Pinger", 0),
        _ => card(name).unwrap(),
    };
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Tazri, Stalwart Survivor").unwrap()),
            ready(other),
        ],
    );
    let scenario = libraries(scenario, seats);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    assert_eq!(driver.tap_for_mana(name).is_ok(), taps);
    replays(&driver);
    // Tazri has the granted ability and her mill ability, so she taps too.
    let mut driver = Driver::from_scenario(scenario).unwrap();
    assert!(driver.tap_for_mana("Tazri, Stalwart Survivor").is_ok());
    replays(&driver);
}
