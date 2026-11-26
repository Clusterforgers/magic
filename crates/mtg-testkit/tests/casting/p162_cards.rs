//! Goal: the p162 cards play whole, at two to four seats, and each game replays to its state. Sahir grows when a source
//! its controller controls deals exactly 1 damage to them or a spell or ability they control makes them lose exactly 1
//! life, draws on the second resolution in a turn, and its eminence deals 1 damage from the battlefield and the command
//! zone. Grimoire Thief exiles the top three cards of an opponent's library face down for its controller alone to look at,
//! and its sacrifice turns them face up and counters every spell with one of their names. Vulcan Mind-Meld has each of the
//! two players exile the other's top cards face down to look at and play, with mana of any type. Parallel Thoughts
//! exiles seven searched cards in a face-down pile that no one looks at, and a draw may take the pile's top card instead,
//! even from an empty pile. Piracy lets its caster tap the lands of others for mana spent only on spells. Land
//! Equilibrium has an opponent with at least as many lands put a land onto the battlefield and then sacrifice a land,
//! before anyone could tap it. Ertai's Meddling exiles a spell with X delay counters, removes one at each of its
//! controller's upkeeps and puts the card back onto the stack as the spell it was, uncast.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{CardsChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::object::FaceDown;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, free_creature, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

fn replays(driver: &Driver) {
    let state = mtg_engine::replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// A spell of `kind` for {0} with these rules.
fn spell(name: &str, kind: CardType, rules: &str) -> Arc<CardDefinition> {
    let mut def = CardDefinition::new(name).with_type(kind);
    def.mana_cost = Some(ManaCost::parse("{0}").unwrap());
    def.rules = vec![Arc::new(serde_json::from_str::<Ability>(rules).unwrap())];
    Arc::new(def)
}

/// A sorcery for {0} that makes its controller lose `amount` life.
fn bleed(amount: i32) -> Arc<CardDefinition> {
    spell(
        &format!("Bleed {amount}"),
        CardType::Sorcery,
        &format!(r#"{{"Spell":{{"effects":[{{"LoseLife":{{"amount":{amount}}}}}]}}}}"#),
    )
}

/// Ten cards named after their seat and place, so each card of a library is told apart.
fn named_library(scenario: Scenario, seat: usize, count: usize) -> Scenario {
    scenario.library(
        seat,
        (0..count)
            .map(|at| free_creature(&format!("Card {seat}-{at}"), 1, 1))
            .collect::<Vec<_>>(),
    )
}

fn libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| named_library(scenario, seat, 10))
}

/// Answers every question until the stack is empty and seat 0 has priority again: mana is paid automatically, the
/// first legal targets are taken, `pick` answers what it knows and every yes-or-no is `yes`.
fn settle(driver: &mut Driver, yes: bool, pick: &dyn Fn(&Decision) -> Option<Answer>) {
    settle_at(driver, 0, yes, pick);
}

/// `settle` until the stack is empty and `home` has priority.
fn settle_at(
    driver: &mut Driver,
    home: usize,
    yes: bool,
    pick: &dyn Fn(&Decision) -> Option<Answer>,
) {
    for _ in 0..400 {
        let decision = driver.decision().unwrap().clone();
        if let Some(answer) = pick(&decision) {
            driver.answer(answer).unwrap();
            continue;
        }
        let answer = match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().resolving().is_none()
                    && decision.player == pid(home) =>
            {
                return;
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(yes)),
            DecisionKind::ChooseTargets(options) => Answer::ChooseTargets(TargetChoice::Targets(
                options
                    .legal
                    .iter()
                    .copied()
                    .take(options.min.max(1) as usize)
                    .collect(),
            )),
            DecisionKind::ChooseCards(options) => Answer::ChooseCards(CardsChoice(
                options
                    .from
                    .iter()
                    .copied()
                    .take(options.max as usize)
                    .collect(),
            )),
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    panic!("the game did not settle");
}

fn none(_: &Decision) -> Option<Answer> {
    None
}

fn plus_counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .and_then(|permanent| {
            permanent
                .counters()
                .find(|(kind, _)| **kind == mtg_cards::value::CounterKind::PlusOnePlusOne)
                .map(|(_, count)| count)
        })
        .unwrap_or(0)
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn hand_size(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

fn exiled(driver: &Driver) -> Vec<ObjectId> {
    driver.state().zone(Zone::Exile).to_vec()
}

#[rstest]
fn p162_cards_are_verified_with_the_engines_capabilities(
    #[values(
        "Parallel Thoughts",
        "Piracy",
        "Land Equilibrium",
        "Ertai's Meddling",
        "Grimoire Thief",
        "Vulcan Mind-Meld",
        "Sahir, Visitor in Darkness"
    )]
    name: &str,
) {
    let record = mtg_testkit::corpus::corpus()
        .unwrap()
        .support(name)
        .unwrap();
    assert_eq!(
        record.support,
        mtg_cards::support::Support::Verified,
        "{name}: {:?}",
        record.clauses
    );
}

// --- Sahir, Visitor in Darkness ---

fn sahir_table(seats: usize, hand: Vec<Arc<CardDefinition>>) -> Driver {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            vec![
                ready(card("Sahir, Visitor in Darkness").unwrap()),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .hand(0, hand);
    Driver::from_scenario(libraries(scenario, seats)).unwrap()
}

#[rstest]
#[case::one_damage_from_my_spell_to_me(burn("Ping", 1), true, 1)]
#[case::two_damage_is_not_exactly_one(burn("Ping", 2), true, 0)]
#[case::one_damage_to_an_opponent(burn("Ping", 1), false, 0)]
#[case::losing_exactly_one_life(bleed(1), true, 1)]
#[case::losing_two_life(bleed(2), true, 0)]
fn sahir_grows_on_exactly_one(
    #[values(2, 3, 4)] seats: usize,
    #[case] hurt: Arc<CardDefinition>,
    #[case] at_me: bool,
    #[case] grows: u32,
) {
    let name = hurt.name.clone();
    let mut driver = sahir_table(seats, vec![hurt]);
    let hand = hand_size(&driver, 0);
    let target = driver.target_player(usize::from(!at_me));
    driver.begin_cast(&name).unwrap();
    if matches!(
        driver.decision().unwrap().kind,
        DecisionKind::ChooseTargets(_)
    ) {
        driver.choose_targets(&[target]).unwrap();
    }
    settle(&mut driver, true, &none);
    assert_eq!(plus_counters(&driver, "Sahir, Visitor in Darkness"), grows);
    // The first resolution of the turn draws nothing.
    assert_eq!(hand_size(&driver, 0), hand - 1);
    replays(&driver);
}

#[rstest]
fn sahir_draws_on_the_second_resolution_of_a_turn(#[values(2, 3, 4)] seats: usize) {
    let mut driver = sahir_table(seats, vec![burn("Ping", 1), bleed(1)]);
    let me = driver.target_player(0);
    driver.cast_at("Ping", &[me]).unwrap();
    settle(&mut driver, true, &none);
    driver.cast("Bleed 1").unwrap();
    settle(&mut driver, true, &none);
    assert_eq!(plus_counters(&driver, "Sahir, Visitor in Darkness"), 2);
    assert_eq!(hand_size(&driver, 0), 1, "two spells cast, one card drawn");
    assert_eq!(life(&driver, 0), 18);
    replays(&driver);
}

#[rstest]
fn sahir_eminence_hurts_from_the_battlefield_and_feeds_itself(#[values(2, 3, 4)] seats: usize) {
    let mut driver = sahir_table(seats, Vec::new());
    driver
        .idle_until(1 + u32::try_from(seats).unwrap(), Step::Upkeep)
        .unwrap();
    settle(&mut driver, true, &none);
    assert_eq!(life(&driver, 0), 19);
    assert_eq!(plus_counters(&driver, "Sahir, Visitor in Darkness"), 1);
    replays(&driver);
}

#[rstest]
fn sahir_eminence_hurts_from_the_command_zone(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .format(Arc::new(mtg_engine::format::Commander))
        .at(1, 0, Step::Main1)
        .commanders(0, [card("Sahir, Visitor in Darkness").unwrap()]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    let before = life(&driver, 0);
    driver
        .idle_until(1 + u32::try_from(seats).unwrap(), Step::Upkeep)
        .unwrap();
    settle(&mut driver, true, &none);
    assert_eq!(life(&driver, 0), before - 1);
    replays(&driver);
}

// --- Grimoire Thief ---

const TAP: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Tap":{"what":{"Target":0}}}]}}"#;

/// Seat 0 with Grimoire Thief, an Island and a tapping sorcery; the libraries hold `foe_top` on top of seat 1's.
fn thief_table(seats: usize, hand: Vec<Arc<CardDefinition>>) -> Driver {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            vec![
                ready(card("Grimoire Thief").unwrap()),
                ready(island()),
                ready(mountain()),
            ],
        )
        .hand(0, hand);
    let scenario = libraries(scenario, seats);
    // Seat 1's top card shares its name with a burn spell seat 0 holds.
    // The top of a library is its first card.
    let mut foe: Vec<Arc<CardDefinition>> = vec![burn("Shock", 2)];
    foe.extend((0..9).map(|at| free_creature(&format!("Card 1-{at}"), 1, 1)));
    Driver::from_scenario(scenario.library(1, foe)).unwrap()
}

fn tap_the_thief(driver: &mut Driver) {
    let thief = driver.target_named("Grimoire Thief").unwrap();
    driver.cast_at("Tapper", &[thief]).unwrap();
    settle(driver, true, &none);
}

#[rstest]
fn grimoire_thief_exiles_three_face_down_for_its_controller_alone(#[values(2, 3, 4)] seats: usize) {
    let mut driver = thief_table(seats, vec![spell("Tapper", CardType::Sorcery, TAP)]);
    let foe_library = driver.state().zone(Zone::Library(pid(1))).len();
    tap_the_thief(&mut driver);
    let exiled = exiled(&driver);
    assert_eq!(exiled.len(), 3);
    assert_eq!(
        driver.state().zone(Zone::Library(pid(1))).len(),
        foe_library - 3
    );
    for id in exiled {
        let object = driver.state().object(id).unwrap();
        assert!(matches!(object.face_down(), Some(FaceDown::Sealed { .. })));
        assert!(driver.state().may_look_at(pid(0), id));
        for seat in 1..seats {
            assert!(!driver.state().may_look_at(pid(seat), id), "seat {seat}");
        }
    }
    replays(&driver);
}

#[rstest]
fn grimoire_thief_turns_its_cards_up_and_counters_spells_with_their_names(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = thief_table(
        seats,
        vec![spell("Tapper", CardType::Sorcery, TAP), burn("Shock", 2)],
    );
    tap_the_thief(&mut driver);
    let exiled = exiled(&driver);
    let foe = driver.target_player(1);
    let before = life(&driver, 1);
    driver.cast_at("Shock", &[foe]).unwrap();
    driver.begin_activation("Grimoire Thief").unwrap();
    settle(&mut driver, true, &none);
    assert_eq!(life(&driver, 1), before, "the Shock was countered");
    assert!(driver.id_of("Grimoire Thief").is_err(), "sacrificed");
    for id in exiled {
        assert_eq!(driver.state().object(id).unwrap().face_down(), None);
        for seat in 0..seats {
            assert!(driver.state().may_look_at(pid(seat), id));
        }
    }
    let graveyard = driver.state().zone(Zone::Graveyard(pid(0)));
    assert!(
        graveyard
            .iter()
            .any(|id| driver.state().object(*id).unwrap().card().name == "Shock")
    );
    replays(&driver);
}

// --- Vulcan Mind-Meld ---

#[rstest]
fn vulcan_mind_meld_has_each_player_exile_the_others_cards_to_play(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, vec![ready(island()), ready(island())])
        .hand(0, vec![card("Vulcan Mind-Meld").unwrap()]);
    let scenario = libraries(scenario, seats);
    let mut foe: Vec<Arc<CardDefinition>> = vec![burn("Red Bolt", 1), mountain()];
    foe.extend((0..8).map(|at| free_creature(&format!("Card 1-{at}"), 1, 1)));
    let mut driver = Driver::from_scenario(scenario.library(1, foe)).unwrap();
    let mine = *driver.state().zone(Zone::Library(pid(0))).last().unwrap();
    let theirs: Vec<ObjectId> = driver
        .state()
        .zone(Zone::Library(pid(1)))
        .iter()
        .rev()
        .take(2)
        .copied()
        .collect();
    let foe_target = driver.target_player(1);
    driver.begin_cast("Vulcan Mind-Meld").unwrap();
    driver.choose_targets(&[foe_target]).unwrap();
    driver.pay_tapping(&["Island"], 0).unwrap();
    settle(&mut driver, true, &none);
    let state = driver.state();
    assert_eq!(state.zone(Zone::Exile).len(), 3);
    let held = |id: ObjectId, looker: usize| {
        matches!(
            state.object(id).unwrap().face_down(),
            Some(FaceDown::Held { looker: seen, .. }) if seen == pid(looker)
        )
    };
    assert!(held(mine, 1));
    assert!(theirs.iter().all(|id| held(*id, 0)));
    assert!(state.may_look_at(pid(1), mine) && !state.may_look_at(pid(0), mine));
    assert!(theirs.iter().all(|id| state.may_look_at(pid(0), *id)));
    assert!(theirs.iter().all(|id| !state.may_look_at(pid(1), *id)));
    // The Island left pays {R} of the Bolt exiled from seat 1's library, and the Mountain is played from exile.
    let target = driver.target_player(1);
    let before = life(&driver, 1);
    driver.cast_at("Red Bolt", &[target]).unwrap();
    settle(&mut driver, true, &none);
    assert_eq!(life(&driver, 1), before - 1);
    driver.play_land("Mountain").unwrap();
    assert!(driver.id_of("Mountain").is_ok());
    replays(&driver);
}

// --- Parallel Thoughts ---

fn thoughts_table(seats: usize, library: usize, studies: usize) -> Driver {
    let mut hand = vec![card("Parallel Thoughts").unwrap()];
    hand.extend((0..studies).map(|_| {
        spell(
            "Study",
            CardType::Sorcery,
            r#"{"Spell":{"effects":[{"Draw":{}}]}}"#,
        )
    }));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, (0..5).map(|_| ready(island())).collect::<Vec<_>>())
        .hand(0, hand);
    let scenario = libraries(scenario, seats);
    Driver::from_scenario(named_library(scenario, 0, library)).unwrap()
}

#[rstest]
#[case::a_full_library(10, 7)]
#[case::seven_exactly(7, 7)]
#[case::a_short_library(4, 4)]
fn parallel_thoughts_exiles_a_face_down_pile_no_one_sees(
    #[values(2, 3, 4)] seats: usize,
    #[case] library: usize,
    #[case] pile: usize,
) {
    let mut driver = thoughts_table(seats, library, 1);
    driver.cast("Parallel Thoughts").unwrap();
    settle(&mut driver, true, &none);
    let exiled = exiled(&driver);
    assert_eq!(exiled.len(), pile);
    assert_eq!(
        driver.state().zone(Zone::Library(pid(0))).len(),
        library - pile
    );
    for id in exiled {
        assert!(matches!(
            driver.state().object(id).unwrap().face_down(),
            Some(FaceDown::Sealed { .. })
        ));
        for seat in 0..seats {
            assert!(!driver.state().may_look_at(pid(seat), id));
        }
    }
    replays(&driver);
}

#[rstest]
#[case::takes_the_pile_top(true)]
#[case::draws_from_the_library(false)]
fn parallel_thoughts_replaces_a_draw_with_the_pile_top_if_wanted(
    #[values(2, 3, 4)] seats: usize,
    #[case] take: bool,
) {
    let mut driver = thoughts_table(seats, 10, 1);
    driver.cast("Parallel Thoughts").unwrap();
    settle(&mut driver, true, &none);
    let library = driver.state().zone(Zone::Library(pid(0))).len();
    driver.cast("Study").unwrap();
    settle(&mut driver, take, &none);
    let hand = driver.state().zone(Zone::Hand(pid(0)));
    assert_eq!(hand.len(), 1);
    let drawn = driver.state().object(hand[0]).unwrap();
    assert_eq!(
        driver.state().zone(Zone::Exile).len(),
        if take { 6 } else { 7 }
    );
    assert_eq!(
        driver.state().zone(Zone::Library(pid(0))).len(),
        if take { library } else { library - 1 }
    );
    assert!(drawn.card().name.starts_with("Card 0-"));
    replays(&driver);
}

#[rstest]
fn parallel_thoughts_takes_the_pile_then_skips_draws_from_it_empty(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = thoughts_table(seats, 2, 3);
    driver.cast("Parallel Thoughts").unwrap();
    settle(&mut driver, true, &none);
    assert_eq!(
        driver.state().zone(Zone::Library(pid(0))),
        &[] as &[ObjectId]
    );
    assert_eq!(exiled(&driver).len(), 2);
    for _ in 0..3 {
        driver.cast("Study").unwrap();
        settle(&mut driver, true, &none);
    }
    // Both pile cards came to hand and the third draw was replaced by taking from the empty pile.
    assert_eq!(hand_size(&driver, 0), 2);
    assert_eq!(driver.state().zone(Zone::Exile), &[] as &[ObjectId]);
    assert!(
        driver.state().result().is_none(),
        "no one drew from an empty library"
    );
    replays(&driver);
}

// --- Piracy ---

fn piracy_table(seats: usize) -> Driver {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, vec![ready(island()), ready(island())])
        .battlefield(1, vec![ready(mountain()), ready(mountain())])
        .hand(
            0,
            vec![
                card("Piracy").unwrap(),
                burn("Bolt", 2),
                spell(
                    "Pump",
                    CardType::Sorcery,
                    r#"{"Spell":{"effects":[{"Draw":{}}]}}"#,
                ),
            ],
        );
    Driver::from_scenario(libraries(scenario, seats)).unwrap()
}

#[rstest]
fn piracy_taps_the_lands_of_others_for_mana_spent_on_spells(#[values(2, 3, 4)] seats: usize) {
    let mut driver = piracy_table(seats);
    assert!(
        driver.tap_for_mana("Mountain").is_err(),
        "another player's land is no source before Piracy"
    );
    driver.cast("Piracy").unwrap();
    settle(&mut driver, true, &none);
    let events = driver.tap_for_mana("Mountain").unwrap();
    let added = events.iter().find_map(|event| match event {
        mtg_engine::events::Event::ManaAdded { player, only, .. } => Some((*player, only.clone())),
        _ => None,
    });
    let (player, only) = added.expect("mana was added");
    assert_eq!(
        player,
        pid(0),
        "the mana goes to the player who tapped the land"
    );
    assert_eq!(
        only,
        vec![mtg_cards::spend::SpendOn::Spell(
            mtg_cards::spend::SpendFilter::default()
        )]
    );
    let foe = driver.target_player(1);
    let before = life(&driver, 1);
    driver.cast_at("Bolt", &[foe]).unwrap();
    settle(&mut driver, true, &none);
    assert_eq!(life(&driver, 1), before - 2);
    let tapped = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            let object = driver.state().object(**id).unwrap();
            object.controller() == pid(1)
                && object
                    .permanent()
                    .is_some_and(mtg_engine::object::PermanentState::tapped)
        })
        .count();
    assert_eq!(tapped, 1, "seat 1 keeps controlling its tapped Mountain");
    replays(&driver);
}

#[rstest]
fn piracy_ends_with_the_turn(#[values(2, 3, 4)] seats: usize) {
    let mut driver = piracy_table(seats);
    driver.cast("Piracy").unwrap();
    settle(&mut driver, true, &none);
    driver
        .idle_until(1 + u32::try_from(seats).unwrap(), Step::Main1)
        .unwrap();
    assert!(driver.tap_for_mana("Mountain").is_err());
    replays(&driver);
}

// --- Land Equilibrium ---

/// Seat 1's turn: seat 0 has Land Equilibrium and `mine` Islands, seat 1 `theirs` Mountains and a Mountain in hand.
fn equilibrium_table(seats: usize, mine: usize, theirs: usize) -> Driver {
    let mut board = vec![ready(card("Land Equilibrium").unwrap())];
    board.extend((0..mine).map(|_| ready(island())));
    let scenario = Scenario::new(seats)
        .at(2, 1, Step::Main1)
        .battlefield(0, board)
        .battlefield(
            1,
            (0..theirs).map(|_| ready(mountain())).collect::<Vec<_>>(),
        )
        .hand(1, vec![mountain()]);
    Driver::from_scenario(libraries(scenario, seats)).unwrap()
}

fn lands_of(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            let object = driver.state().object(**id).unwrap();
            object.controller() == pid(seat) && object.card().name == "Mountain"
        })
        .count()
}

#[rstest]
#[case::as_many_lands(2, 2, true)]
#[case::more_lands(2, 3, true)]
#[case::fewer_lands(2, 1, false)]
fn land_equilibrium_makes_an_opponent_with_as_many_lands_sacrifice_one(
    #[values(2, 3, 4)] seats: usize,
    #[case] mine: usize,
    #[case] theirs: usize,
    #[case] sacrifices: bool,
) {
    let mut driver = equilibrium_table(seats, mine, theirs);
    let asked = std::cell::Cell::new(false);
    driver.play_land("Mountain").unwrap();
    settle_at(&mut driver, 1, true, &|decision| {
        if decision.player == pid(1)
            && let DecisionKind::ChooseCards(options) = &decision.kind
        {
            asked.set(true);
            return Some(Answer::ChooseCards(CardsChoice(
                options
                    .from
                    .iter()
                    .copied()
                    .take(options.min as usize)
                    .collect(),
            )));
        }
        None
    });
    assert_eq!(asked.get(), sacrifices && theirs > 0);
    let expected = if sacrifices { theirs } else { theirs + 1 };
    assert_eq!(lands_of(&driver, 1), expected);
    let graveyard = driver.state().zone(Zone::Graveyard(pid(1))).len();
    assert_eq!(graveyard, usize::from(sacrifices));
    replays(&driver);
}

#[rstest]
fn land_equilibrium_lets_the_new_land_be_the_one_sacrificed_untapped(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = equilibrium_table(seats, 1, 1);
    let played = *driver.state().zone(Zone::Hand(pid(1))).first().unwrap();
    driver.play_land("Mountain").unwrap();
    settle_at(&mut driver, 1, true, &|decision| match &decision.kind {
        DecisionKind::ChooseCards(options) if decision.player == pid(1) => {
            let new = options
                .from
                .iter()
                .copied()
                .find(|id| *id == played)
                .expect("the land that entered may be sacrificed");
            Some(Answer::ChooseCards(CardsChoice(vec![new])))
        }
        _ => None,
    });
    let card = driver.state().object(played).unwrap();
    assert_eq!(card.zone(), Zone::Graveyard(pid(1)));
    assert_eq!(lands_of(&driver, 1), 1);
    replays(&driver);
}

#[rstest]
fn land_equilibrium_leaves_its_controllers_lands_alone(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, vec![ready(card("Land Equilibrium").unwrap())])
        .hand(0, vec![mountain()]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    driver.play_land("Mountain").unwrap();
    settle(&mut driver, true, &none);
    assert_eq!(lands_of(&driver, 0), 1);
    replays(&driver);
}

// --- Ertai's Meddling ---

fn meddling_table(seats: usize, islands: usize) -> Driver {
    let mut board: Vec<_> = (0..islands).map(|_| ready(island())).collect();
    board.push(ready(mountain()));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board)
        .hand(0, vec![card("Ertai's Meddling").unwrap(), burn("Bolt", 3)]);
    Driver::from_scenario(libraries(scenario, seats)).unwrap()
}

/// Casts Bolt at seat 1, then Ertai's Meddling for `x` at the Bolt; the Meddling resolves.
fn meddle(driver: &mut Driver, x: u32) -> ObjectId {
    let foe = driver.target_player(1);
    driver.begin_cast("Bolt").unwrap();
    driver.choose_targets(&[foe]).unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    let bolt = *driver.state().stack().last().unwrap();
    driver.begin_cast("Ertai's Meddling").unwrap();
    for _ in 0..10 {
        match &driver.decision().unwrap().kind {
            DecisionKind::ChooseNumber(options) => {
                assert!(options.min >= 1, "X can't be 0: {options:?}");
                driver
                    .answer(Answer::ChooseNumber(
                        mtg_engine::choose::NumberChoice::Number(x),
                    ))
                    .unwrap();
            }
            DecisionKind::ChooseTargets(_) => {
                let spell = driver.state().object(bolt).unwrap().reference();
                driver
                    .choose_targets(&[mtg_engine::choose::Target::Object(spell)])
                    .unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
                break;
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
    settle(driver, true, &none);
    bolt
}

fn delay_counters(driver: &Driver, card: ObjectId) -> u32 {
    driver
        .state()
        .object(card)
        .unwrap()
        .card_counters_of(&mtg_cards::value::CounterKind::Other("delay".into()))
}

#[rstest]
#[case::one_upkeep(1)]
#[case::two_upkeeps(2)]
fn ertais_meddling_delays_a_spell_by_x_of_its_controllers_upkeeps(
    #[values(2, 3, 4)] seats: usize,
    #[case] x: u32,
) {
    let mut driver = meddling_table(seats, 1 + usize::try_from(x).unwrap());
    let before = life(&driver, 1);
    let bolt = meddle(&mut driver, x);
    assert_eq!(
        life(&driver, 1),
        before,
        "the Bolt is exiled before it resolves"
    );
    assert_eq!(driver.state().object(bolt).unwrap().zone(), Zone::Exile);
    assert_eq!(delay_counters(&driver, bolt), x);
    let round = u32::try_from(seats).unwrap();
    for upkeep in 1..=x {
        driver.idle_until(1 + upkeep * round, Step::Upkeep).unwrap();
        settle(&mut driver, true, &none);
        if upkeep < x {
            assert_eq!(delay_counters(&driver, bolt), x - upkeep);
            assert_eq!(life(&driver, 1), before);
        }
    }
    // The card went back onto the stack as the Bolt it was, at the same target, and resolved without being cast.
    assert_eq!(life(&driver, 1), before - 3);
    assert_eq!(
        driver.state().object(bolt).unwrap().zone(),
        Zone::Graveyard(pid(0))
    );
    let casts = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, mtg_engine::events::Event::SpellCast { object, .. } if *object == bolt))
        .count();
    assert_eq!(casts, 1, "putting it back is no cast");
    replays(&driver);
}
