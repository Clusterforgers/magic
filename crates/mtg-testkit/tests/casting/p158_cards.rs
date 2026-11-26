//! Goal: the p158 cards play whole, at two to four seats: Xu-Ifit returns a creature card as a Skeleton without
//! abilities, so its enters trigger never fires; Caller of the Untamed makes nothing in a game without a draft; Heirloom
//! Epic draws with creatures tapped for its mana but not artifacts; The Unluckiest Planeswalker's +1 mana makes a
//! Gremlin only when spent on an Aura spell and its −3 discards, draws twice the Auras and moves the Auras on its
//! controller to an opponent; Garth One-Eye copies a card of a name it has not chosen, the copy is cast and a permanent
//! copy becomes a token, and a chosen name is not offered again; each game replays to its state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{ChoiceOption, OptionChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::manual::ManualAction;
use mtg_engine::object::ObjectKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, replay};
use mtg_testkit::cards::{forest, free_creature, island, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, enchanting, ready, sick};
use rstest::rstest;

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, vec![island(); 10])
    })
}

/// Answers every question until the stack is empty and seat 0 has priority again: mana is paid automatically, the
/// first legal targets are taken, `pick` answers the questions of options and every yes-or-no is yes.
fn settle(driver: &mut Driver, pick: &dyn Fn(&Decision) -> Option<Answer>) {
    for _ in 0..300 {
        let decision = driver.decision().unwrap().clone();
        if let Some(answer) = pick(&decision) {
            driver.answer(answer).unwrap();
            continue;
        }
        let answer = match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().resolving().is_none()
                    && decision.player == pid(0) =>
            {
                return;
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
            DecisionKind::ChooseTargets(options) => Answer::ChooseTargets(TargetChoice::Targets(
                options
                    .legal
                    .iter()
                    .copied()
                    .take(options.min.max(1) as usize)
                    .collect(),
            )),
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    panic!("the game did not settle");
}

fn nothing(_: &Decision) -> Option<Answer> {
    None
}

fn names_in(driver: &Driver, zone: Zone) -> Vec<String> {
    let mut names: Vec<String> = driver
        .state()
        .zone(zone)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .map(|object| object.card().name.clone())
        .collect();
    names.sort();
    names
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// A creature with "When this creature enters, you gain 3 life."
fn healer_bear() -> Arc<CardDefinition> {
    let json = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"GainLife":{"amount":3}}]}}"#;
    Arc::new(
        CardDefinition::new("Healer Bear")
            .with_type(CardType::Creature)
            .with_subtype("Bear")
            .with_power_toughness(2, 2)
            .with_rules(serde_json::from_str::<Ability>(json).unwrap()),
    )
}

#[rstest]
fn xu_ifit_returns_a_skeleton_without_abilities(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Xu-Ifit, Osteoharmonist").unwrap())])
        .graveyard(0, [healer_bear()]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    driver.begin_activation("Xu-Ifit, Osteoharmonist").unwrap();
    settle(&mut driver, &nothing);
    let bear = driver.id_of("Healer Bear").unwrap();
    let object = driver.state().object(bear).unwrap();
    assert_eq!(object.zone(), Zone::Battlefield);
    let chars = mtg_engine::characteristics::characteristics(driver.state(), bear).unwrap();
    let printed = &object.card().subtypes;
    for kind in ["Skeleton", "Bear"] {
        let kind = mtg_core::card::Subtype(kind.to_owned());
        assert!(chars.has_subtype(printed, &kind), "{kind:?}");
    }
    assert!(chars.abilities.is_empty(), "it has no abilities");
    assert_eq!(life(&driver, 0), 20, "its enters trigger never fired");
    replays(&driver);
}

#[rstest]
fn caller_of_the_untamed_makes_nothing_without_a_draft(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Caller of the Untamed").unwrap()),
            ready(forest()),
            ready(forest()),
        ],
    );
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    let before = driver.state().zone(Zone::Battlefield).len();
    driver.begin_activation("Caller of the Untamed").unwrap();
    settle(&mut driver, &|decision| match &decision.kind {
        DecisionKind::ChooseNumber(options) => Some(Answer::ChooseNumber(
            mtg_engine::choose::NumberChoice::Number(options.max.unwrap_or(2).min(2)),
        )),
        _ => None,
    });
    assert_eq!(driver.state().zone(Zone::Battlefield).len(), before);
    replays(&driver);
}

#[rstest]
#[case::creatures_pay_it_all(0, 4, true)]
#[case::a_land_and_creatures(1, 3, true)]
#[case::one_short(1, 2, false)]
fn heirloom_epic_draws_with_creatures_tapped(
    #[values(2, 3, 4)] seats: usize,
    #[case] lands: usize,
    #[case] bears: usize,
    #[case] payable: bool,
) {
    let mut board = vec![ready(card("Heirloom Epic").unwrap())];
    board.extend((0..lands).map(|_| ready(forest())));
    board.extend((1..=bears).map(|n| sick(free_creature(&format!("Bear {n}"), 2, 2))));
    // An artifact never helps.
    board.push(ready(Arc::new(
        CardDefinition::new("Relic").with_type(CardType::Artifact),
    )));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    let offered = !driver
        .offered_abilities("Heirloom Epic")
        .unwrap()
        .is_empty();
    assert_eq!(offered, payable);
    if !payable {
        return;
    }
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.begin_activation("Heirloom Epic").unwrap();
    settle(&mut driver, &nothing);
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand + 1);
    replays(&driver);
}

/// Picks red among the colors offered.
fn red(decision: &Decision) -> Option<Answer> {
    let DecisionKind::ChooseOption(options) = &decision.kind else {
        return None;
    };
    let at = options
        .options
        .iter()
        .position(|option| *option == ChoiceOption::Color(Color::Red))?;
    Some(Answer::ChooseOption(OptionChoice(at)))
}

fn unluckiest(seats: usize, hand: Vec<Arc<CardDefinition>>) -> Driver {
    let walker = mtg_testkit::with_counters(
        ready(card("The Unluckiest Planeswalker").unwrap()),
        mtg_cards::value::CounterKind::Loyalty,
        5,
    );
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [walker])
        .hand(0, hand);
    Driver::from_scenario(libraries(scenario, seats)).unwrap()
}

fn gremlins(driver: &Driver) -> usize {
    names_in(driver, Zone::Battlefield)
        .iter()
        .filter(|name| *name == "Gremlin")
        .count()
}

#[rstest]
#[case::an_aura_spell("Curse of Opulence", 1)]
#[case::another_spell("Shock", 0)]
fn the_unluckiest_mana_makes_a_gremlin_for_an_aura(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] made: usize,
) {
    let shock = mtg_testkit::cards::burn("Shock", 2);
    let mut driver = unluckiest(seats, vec![card("Curse of Opulence").unwrap(), shock]);
    driver
        .begin_ability("The Unluckiest Planeswalker", 1)
        .unwrap();
    settle(&mut driver, &red);
    assert_eq!(gremlins(&driver), 0);
    driver.begin_cast(spell).unwrap();
    settle(&mut driver, &red);
    assert_eq!(gremlins(&driver), made);
    replays(&driver);
}

#[rstest]
fn the_unluckiest_minus_three_moves_the_auras_on_you(#[values(2, 3, 4)] seats: usize) {
    let curse = enchanting(ready(card("Curse of Opulence").unwrap()), 0);
    let walker = mtg_testkit::with_counters(
        ready(card("The Unluckiest Planeswalker").unwrap()),
        mtg_cards::value::CounterKind::Loyalty,
        5,
    );
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [walker, curse])
        .hand(0, [mountain(), swamp()]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    driver
        .begin_ability("The Unluckiest Planeswalker", 2)
        .unwrap();
    settle(&mut driver, &nothing);
    let hand = names_in(&driver, Zone::Hand(pid(0)));
    assert_eq!(
        hand,
        vec!["Island".to_owned(); 2],
        "one Aura you control, two cards"
    );
    let curse = driver.id_of("Curse of Opulence").unwrap();
    let host = driver.state().player_host_of(curse);
    assert!(host.is_some_and(|player| player != pid(0)), "{host:?}");
    replays(&driver);
}

/// The six cards Garth names, as the game resolves them: a free sorcery that gains 1 life, and a free 5/5 Shivan Dragon.
fn garth_cards() -> Vec<(String, Arc<CardDefinition>)> {
    let gain = r#"{"Spell":{"effects":[{"GainLife":{"amount":1}}]}}"#;
    [
        "Disenchant",
        "Braingeyser",
        "Terror",
        "Regrowth",
        "Black Lotus",
    ]
    .into_iter()
    .map(|name| {
        let card = CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_rules(serde_json::from_str::<Ability>(gain).unwrap());
        (name.to_owned(), Arc::new(card))
    })
    .chain(std::iter::once((
        "Shivan Dragon".to_owned(),
        free_creature("Shivan Dragon", 5, 5),
    )))
    .collect()
}

fn garth(seats: usize) -> (Scenario, Driver) {
    let scenario = libraries(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(card("Garth One-Eye").unwrap())]),
        seats,
    );
    let setup = scenario
        .clone()
        .setup()
        .unwrap()
        .with_conjurable(garth_cards());
    (scenario, Driver::new(Engine::new(setup).unwrap()).unwrap())
}

/// Picks the name among the options, and records the names offered.
fn naming<'a>(
    name: &'a str,
    offered: &'a std::cell::RefCell<Vec<String>>,
) -> impl Fn(&Decision) -> Option<Answer> + 'a {
    move |decision| {
        let DecisionKind::ChooseOption(options) = &decision.kind else {
            return None;
        };
        let labels: Vec<String> = options
            .options
            .iter()
            .filter_map(|option| match option {
                ChoiceOption::Label(label) => Some(label.clone()),
                _ => None,
            })
            .collect();
        let at = labels.iter().position(|label| label == name)?;
        *offered.borrow_mut() = labels;
        Some(Answer::ChooseOption(OptionChoice(at)))
    }
}

#[rstest]
fn garth_copies_a_name_not_chosen_before(#[values(2, 3, 4)] seats: usize) {
    let (scenario, mut driver) = garth(seats);
    let offered = std::cell::RefCell::new(Vec::new());
    driver.begin_activation("Garth One-Eye").unwrap();
    settle(&mut driver, &naming("Shivan Dragon", &offered));
    assert_eq!(offered.borrow().len(), 6);
    let dragon = driver.id_of("Shivan Dragon").unwrap();
    let object = driver.state().object(dragon).unwrap();
    assert_eq!(object.zone(), Zone::Battlefield);
    assert_eq!(
        object.kind(),
        ObjectKind::Token,
        "a resolved copy becomes a token"
    );
    let garth = driver.id_of("Garth One-Eye").unwrap();
    driver
        .manual(ManualAction::Untap { object: garth })
        .unwrap();
    driver.begin_activation("Garth One-Eye").unwrap();
    settle(&mut driver, &naming("Terror", &offered));
    assert_eq!(offered.borrow().len(), 5);
    assert!(!offered.borrow().iter().any(|name| name == "Shivan Dragon"));
    assert_eq!(life(&driver, 0), 21, "the copy of the sorcery was cast");
    assert!(
        !names_in(&driver, Zone::Exile)
            .iter()
            .any(|name| name == "Terror"),
        "a copy that was cast leaves no card"
    );
    let replayed = replay(
        &scenario.setup().unwrap().with_conjurable(garth_cards()),
        driver.engine().events(),
    )
    .unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}
