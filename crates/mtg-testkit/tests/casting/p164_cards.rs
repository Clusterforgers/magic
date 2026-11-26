//! Goal: the p164 cards play whole, at two to four seats, and are `Verified` with the engine's own capabilities: Chorus
//! of the Conclave lets its controller pay any amount of mana as they cast a creature spell, the creature entering with
//! that many more +1/+1 counters, and offers nothing for other spells or other players; Eriette, the Beguiler takes an
//! opponent's permanent her controller's Aura enchants when its mana value is at most the Aura's, for as long as the
//! Aura stays on it; Equinox counters a spell only when the spell itself would destroy its controller's land; Spy Kit
//! gives every nonlegendary creature card's name the game knows; Panglacial Wurm is cast, paid for, while its owner
//! searches their own library and never in another's search; each game replays to its state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{CardsChoice, NumberChoice, Target, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::events::Event;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost, creature, forest, instant, island, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, attached_to, ready};
use rstest::rstest;

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

fn replays(driver: &Driver) {
    let state = mtg_engine::replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, vec![island(); 10])
    })
}

/// Answers every question until the stack is empty and `seat` has priority again: mana is paid automatically, `pick`
/// answers what it knows and the rest is answered passively; the texts of the numbers asked come back.
fn settle(
    driver: &mut Driver,
    seat: usize,
    pick: &dyn Fn(&Decision) -> Option<Answer>,
) -> Vec<String> {
    let mut asked = Vec::new();
    for _ in 0..300 {
        let decision = driver.decision().unwrap().clone();
        if let DecisionKind::ChooseNumber(options) = &decision.kind {
            asked.push(options.text.clone());
        }
        if let Some(answer) = pick(&decision) {
            driver.answer(answer).unwrap();
            continue;
        }
        let answer = match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().resolving().is_none()
                    && decision.player == pid(seat) =>
            {
                return asked;
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    panic!("the game did not settle");
}

fn number(n: u32) -> impl Fn(&Decision) -> Option<Answer> {
    move |decision| match &decision.kind {
        DecisionKind::ChooseNumber(_) => Some(Answer::ChooseNumber(NumberChoice::Number(n))),
        _ => None,
    }
}

fn plus_counters(driver: &Driver, seat: usize, name: &str) -> u32 {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .find(|object| object.card().name == name && object.controller() == pid(seat))
        .and_then(|object| object.permanent())
        .map(|permanent| permanent.counters_of(&CounterKind::PlusOnePlusOne))
        .unwrap()
}

#[rstest]
#[case("Chorus of the Conclave")]
#[case("Eriette, the Beguiler")]
#[case("Equinox")]
#[case("Spy Kit")]
#[case("Panglacial Wurm")]
fn p164_the_cards_are_verified(#[case] name: &str) {
    let support = mtg_testkit::corpus::corpus()
        .unwrap()
        .support(name)
        .unwrap();
    assert_eq!(
        support.support,
        mtg_cards::support::Support::Verified,
        "{name}: {support:?}"
    );
}

#[rstest]
#[case::nothing_paid(0)]
#[case::one_paid(1)]
#[case::all_paid(6)]
fn chorus_of_the_conclave_pays_for_counters(
    #[values(2, 3, 4)] seats: usize,
    #[case] counters: u32,
) {
    let mut board = vec![ready(card("Chorus of the Conclave").unwrap())];
    board.extend((0..8).map(|_| ready(forest())));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board)
        .hand(0, [bear(), instant("Shock", 0, &[])]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    driver.begin_cast("Grizzly Bears").unwrap();
    let asked = settle(&mut driver, 0, &number(counters));
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert_eq!(plus_counters(&driver, 0, "Grizzly Bears"), counters);
    let untapped = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| {
            object.card().name == "Forest"
                && object
                    .permanent()
                    .is_some_and(|permanent| !permanent.tapped())
        })
        .count();
    assert_eq!(
        untapped,
        8 - 2 - counters as usize,
        "each counter cost {{1}}"
    );
    // An instant is not a creature spell: nothing more is offered.
    driver.begin_cast("Shock").unwrap();
    let asked = settle(&mut driver, 0, &number(1));
    assert!(asked.is_empty(), "{asked:?}");
    replays(&driver);
}

#[rstest]
fn chorus_of_the_conclave_offers_nothing_to_an_opponent(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(0, [ready(card("Chorus of the Conclave").unwrap())])
        .battlefield(1, (0..4).map(|_| ready(forest())).collect::<Vec<_>>())
        .hand(1, [bear()]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    driver.begin_cast("Grizzly Bears").unwrap();
    let asked = settle(&mut driver, 1, &number(2));
    assert!(asked.is_empty(), "{asked:?}");
    assert_eq!(plus_counters(&driver, 1, "Grizzly Bears"), 0);
    replays(&driver);
}

/// An instant for {0}: "Destroy target enchantment."
fn shatter_aura() -> Arc<CardDefinition> {
    let json = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Enchantment"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#;
    Arc::new(
        CardDefinition::new("Shatter Aura")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(serde_json::from_str::<Ability>(json).unwrap()),
    )
}

fn controller(driver: &Driver, name: &str) -> PlayerId {
    mtg_engine::characteristics::controller_of(driver.state(), driver.id_of(name).unwrap()).unwrap()
}

#[rstest]
#[case::cheaper(1, true)]
#[case::as_costly(2, true)]
#[case::costlier(3, false)]
fn eriette_takes_what_her_auras_enchant_while_they_stay(
    #[values(2, 3, 4)] seats: usize,
    #[case] value: u32,
    #[case] taken: bool,
) {
    let victim = creature("Victim", value - 1, &[Color::Green], 2, 2);
    let mut board = vec![ready(card("Eriette, the Beguiler").unwrap())];
    board.extend((0..4).map(|_| ready(plains())));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board)
        .battlefield(seats - 1, [ready(victim)])
        .hand(0, [card("Pacifism").unwrap(), shatter_aura()]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    let target = driver.target_named("Victim").unwrap();
    driver.cast_at("Pacifism", &[target]).unwrap();
    settle(&mut driver, 0, &|_| None);
    let owner = pid(seats - 1);
    let expected = if taken { pid(0) } else { owner };
    assert_eq!(controller(&driver, "Victim"), expected);
    // The Aura leaves, and the control it held ends with it.
    let aura = driver.target_named("Pacifism").unwrap();
    driver.cast_at("Shatter Aura", &[aura]).unwrap();
    settle(&mut driver, 0, &|_| None);
    assert_eq!(controller(&driver, "Victim"), owner);
    replays(&driver);
}

#[rstest]
fn eriette_ignores_an_aura_on_her_controllers_own_permanent(#[values(2, 3, 4)] seats: usize) {
    let mut board = vec![
        ready(card("Eriette, the Beguiler").unwrap()),
        ready(creature("Pet", 0, &[Color::Green], 1, 1)),
    ];
    board.extend((0..2).map(|_| ready(plains())));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board)
        .hand(0, [card("Pacifism").unwrap()]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    let target = driver.target_named("Pet").unwrap();
    driver.cast_at("Pacifism", &[target]).unwrap();
    settle(&mut driver, 0, &|_| None);
    assert_eq!(controller(&driver, "Pet"), pid(0));
    replays(&driver);
}

/// A sorcery for {0} with these rules.
fn sorcery(name: &str, json: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(serde_json::from_str::<Ability>(json).unwrap()),
    )
}

/// A land with no ability of its own.
fn plain_land(name: &str) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(name).with_type(CardType::Land))
}

/// What the opponent's spell aims at.
#[derive(Debug, Clone, Copy)]
enum Aim {
    /// The land Equinox enchants.
    Home,
    /// A land of the caster's own.
    Own,
    /// Seat 0.
    Player,
    /// Nothing.
    None,
}

#[rstest]
#[case::destroys_the_enchanted_land(r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#, Aim::Home, true)]
#[case::destroys_a_land_of_its_caster(r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#, Aim::Own, false)]
#[case::destroys_every_land(
    r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"Type":"Land"}]}}}]}}"#,
    Aim::None,
    true
)]
#[case::destroys_in_a_batch(r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"Together":{"effects":[{"Destroy":{"what":{"Target":0}}}]}}]}}"#, Aim::Home, true)]
#[case::destroys_only_by_a_choice(r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"May":{"effects":[{"Destroy":{"what":{"Target":0}}}]}}]}}"#, Aim::Home, false)]
#[case::deals_damage(r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":2}}]}}"#, Aim::Player, false)]
fn equinox_counters_only_a_spell_that_would_destroy_its_controllers_land(
    #[values(2, 3, 4)] seats: usize,
    #[case] rules: &str,
    #[case] aim: Aim,
    #[case] countered: bool,
) {
    let scenario = Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(
            0,
            [
                ready(plain_land("Homeland")),
                attached_to(ready(card("Equinox").unwrap()), 0, 0),
            ],
        )
        .battlefield(1, [ready(plain_land("Far Land"))])
        .hand(1, [sorcery("Ruin", rules)]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    match aim {
        Aim::Home => {
            let target = driver.target_named("Homeland").unwrap();
            driver.cast_at("Ruin", &[target]).unwrap();
        }
        Aim::Own => {
            let target = driver.target_named("Far Land").unwrap();
            driver.cast_at("Ruin", &[target]).unwrap();
        }
        Aim::Player => {
            let target = driver.target_player(0);
            driver.cast_at("Ruin", &[target]).unwrap();
        }
        Aim::None => {
            driver.cast("Ruin").unwrap();
        }
    }
    let spell = *driver.state().stack().last().unwrap();
    // Seat 1 passes; seat 0 taps the enchanted land at the spell.
    while driver.decision().unwrap().player != pid(0) {
        driver.pass().unwrap();
    }
    driver.begin_activation("Homeland").unwrap();
    let at = Target::Object(driver.state().object(spell).unwrap().reference());
    driver.choose_targets(&[at]).unwrap();
    settle(&mut driver, 1, &|decision| match decision.kind {
        DecisionKind::YesNo(_) => Some(Answer::YesNo(YesNoChoice(true))),
        _ => None,
    });
    let resolved = driver
        .engine()
        .events()
        .iter()
        .any(|event| matches!(event, Event::ResolutionBegan { object } if *object == spell));
    assert_eq!(!resolved, countered);
    replays(&driver);
}

/// A legendary 2/2 for {2}.
fn legend(name: &str) -> Arc<CardDefinition> {
    let mut card = (*creature(name, 2, &[], 2, 2)).clone();
    card.supertypes.insert(mtg_core::card::Supertype::Legendary);
    Arc::new(card)
}

#[rstest]
#[case::a_nonlegendary_creature_card("Grizzly Bears", true)]
#[case::a_legendary_creature_card("Old Hero", false)]
#[case::a_noncreature_card("Shock", false)]
#[case::no_card_of_the_game("Nobody", false)]
fn spy_kit_gives_every_nonlegendary_creature_cards_name(
    #[values(2, 3, 4)] seats: usize,
    #[case] named: &str,
    #[case] destroyed: bool,
) {
    let json = format!(
        r#"{{"Spell":{{"effects":[{{"Destroy":{{"what":{{"Each":[{{"Type":"Creature"}},{{"Named":"{named}"}}]}}}}}}]}}}}"#
    );
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(creature("Agent", 0, &[], 1, 1)),
                attached_to(ready(card("Spy Kit").unwrap()), 0, 0),
                ready(creature("Bystander", 0, &[], 1, 1)),
            ],
        )
        .battlefield(seats - 1, [ready(legend("Old Hero"))])
        .hand(
            0,
            [sorcery("Purge", &json), bear(), instant("Shock", 0, &[])],
        );
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    let agent = driver.id_of("Agent").unwrap();
    let chars = mtg_engine::characteristics::characteristics(driver.state(), agent).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(2), Some(2)));
    driver.cast("Purge").unwrap();
    settle(&mut driver, 0, &|_| None);
    let on_battlefield = |name: &str| {
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .any(|id| driver.state().object(*id).unwrap().card().name == name)
    };
    assert_eq!(!on_battlefield("Agent"), destroyed);
    assert!(on_battlefield("Bystander"));
    replays(&driver);
}

const GROWTH: &str = r#"{"Spell":{"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}"#;

fn in_zone(driver: &Driver, zone: Zone, name: &str) -> usize {
    driver
        .state()
        .zone(zone)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == name)
        .count()
}

#[rstest]
#[case::cast_while_searching(7, true, true)]
#[case::left_in_the_library(7, false, false)]
#[case::too_little_mana(6, true, false)]
fn panglacial_wurm_is_cast_while_its_owner_searches(
    #[values(2, 3, 4)] seats: usize,
    #[case] forests: usize,
    #[case] wants: bool,
    #[case] cast: bool,
) {
    let mut library = vec![card("Panglacial Wurm").unwrap()];
    library.extend((0..5).map(|_| island()));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, (0..forests).map(|_| ready(forest())).collect::<Vec<_>>())
        .hand(0, [sorcery("Growth", GROWTH)]);
    let scenario = (1..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, vec![island(); 10])
    });
    let mut driver = Driver::from_scenario(scenario.library(0, library)).unwrap();
    driver.cast("Growth").unwrap();
    let asked = std::cell::Cell::new(0);
    settle(&mut driver, 0, &|decision| match &decision.kind {
        DecisionKind::ChooseCards(options) if options.text.contains("while you search") => {
            asked.set(asked.get() + 1);
            let wurm = options
                .from
                .first()
                .copied()
                .filter(|_| wants && asked.get() == 1);
            Some(Answer::ChooseCards(CardsChoice(wurm.into_iter().collect())))
        }
        DecisionKind::ChooseCards(options) => Some(Answer::ChooseCards(CardsChoice(
            options.from.iter().copied().take(1).collect(),
        ))),
        _ => None,
    });
    assert_eq!(
        asked.get() > 0,
        forests >= 7,
        "asked only when the Wurm can be cast"
    );
    assert_eq!(
        in_zone(&driver, Zone::Battlefield, "Panglacial Wurm"),
        usize::from(cast)
    );
    assert_eq!(
        in_zone(&driver, Zone::Library(pid(0)), "Panglacial Wurm"),
        usize::from(!cast)
    );
    // The search still found a land: the Island it put onto the battlefield.
    assert_eq!(in_zone(&driver, Zone::Battlefield, "Island"), 1);
    replays(&driver);
}

#[rstest]
fn panglacial_wurm_waits_out_another_players_search(#[values(2, 3, 4)] seats: usize) {
    let mut library = vec![card("Panglacial Wurm").unwrap()];
    library.extend((0..5).map(|_| island()));
    let scenario = Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(0, (0..7).map(|_| ready(forest())).collect::<Vec<_>>())
        .hand(1, [sorcery("Growth", GROWTH)]);
    let scenario = (1..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, vec![island(); 10])
    });
    let mut driver = Driver::from_scenario(scenario.library(0, library)).unwrap();
    driver.cast("Growth").unwrap();
    settle(&mut driver, 1, &|decision| {
        assert!(
            !matches!(&decision.kind, DecisionKind::ChooseCards(options) if options.text.contains("while you search")),
            "only a search of the Wurm's own library offers it"
        );
        None
    });
    assert_eq!(
        in_zone(&driver, Zone::Library(pid(0)), "Panglacial Wurm"),
        1
    );
    replays(&driver);
}
