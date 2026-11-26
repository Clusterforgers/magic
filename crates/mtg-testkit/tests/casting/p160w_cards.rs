//! Goal: the p160w cards play whole at two to four seats: Close Encounter deals the power of the creature or warped card
//! chosen as it is cast; Martyrdom gives an ability only its caster may activate, which moves the next 1 damage to the
//! creature; Master Warcraft lets its caster choose another player's attackers, which that player then aims, and their
//! opponents' blocks; Ghostly Flame lets black and red damage through protection from those colors; Guardian Beast keeps
//! untapped noncreature artifacts from destruction, from Auras and from other players' control; each game replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, TargetChoice, YesNoChoice};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{forest, free_creature, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready, tapped};
use rstest::rstest;

use crate::common::casting::choose_way;
use crate::common::pid;

fn p160w_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn p160w_libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, vec![island(); 12])
    })
}

/// Answers until the stack is empty and `seat` has priority: `pick` answers first, mana is paid automatically, the first
/// legal targets are taken and every other player passes.
fn p160w_settle(
    driver: &mut Driver,
    seat: usize,
    pick: &dyn Fn(&Driver, &Decision) -> Option<Answer>,
) {
    for _ in 0..300 {
        let decision = driver.decision().unwrap().clone();
        if let Some(answer) = pick(driver, &decision) {
            driver.answer(answer).unwrap();
            continue;
        }
        let answer = match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().resolving().is_none()
                    && decision.player == pid(seat) =>
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

fn p160w_none(_: &Driver, _: &Decision) -> Option<Answer> {
    None
}

/// The targets of a target question that name the object `id`.
fn p160w_aim(decision: &Decision, id: ObjectId) -> Option<Answer> {
    let DecisionKind::ChooseTargets(options) = &decision.kind else {
        return None;
    };
    Some(Answer::ChooseTargets(TargetChoice::Targets(
        options
            .legal
            .iter()
            .copied()
            .filter(|target| target.object().is_some_and(|object| object.id == id))
            .collect(),
    )))
}

fn p160w_damage(driver: &Driver, id: ObjectId) -> u32 {
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, mtg_engine::object::PermanentState::damage)
}

fn p160w_in_exile(driver: &Driver, name: &str) -> Option<ObjectId> {
    driver
        .state()
        .zone(Zone::Exile)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().card().name == name)
}

/// Passes until `seat` decides at priority.
fn p160w_until_priority(driver: &mut Driver, seat: usize) {
    for _ in 0..40 {
        let decision = driver.decision().unwrap();
        if decision.player == pid(seat) && matches!(decision.kind, DecisionKind::Priority(_)) {
            return;
        }
        driver.pass().unwrap();
    }
    panic!("seat {seat} never got priority");
}

#[rstest]
#[case::a_creature_on_the_battlefield(false)]
#[case::a_warped_card_in_exile(true)]
fn close_encounter_deals_the_power_of_what_was_chosen(
    #[values(2, 3, 4)] seats: usize,
    #[case] warped: bool,
) {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(free_creature("Bear", 3, 3)),
                ready(swamp()),
                ready(forest()),
                ready(forest()),
            ],
        )
        .battlefield(1, [ready(free_creature("Wall", 0, 7))])
        .graveyard(0, [free_creature("Buried", 5, 5)])
        .hand(
            0,
            [
                card("Timeline Culler").unwrap(),
                card("Close Encounter").unwrap(),
            ],
        );
    let mut driver = Driver::from_scenario(p160w_libraries(scenario, seats)).unwrap();
    if warped {
        driver.begin_cast("Timeline Culler").unwrap();
        for _ in 0..10 {
            if !driver.state().stack().is_empty() {
                break;
            }
            match &driver.decision().unwrap().kind {
                DecisionKind::ChooseOption(_) => choose_way(&mut driver, "Warp").unwrap(),
                _ => {
                    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
                }
            }
        }
        p160w_settle(&mut driver, 0, &p160w_none);
        driver
            .idle_until(2 + u32::try_from(seats).unwrap(), Step::Main1)
            .unwrap();
        assert!(
            p160w_in_exile(&driver, "Timeline Culler").is_some(),
            "warped away"
        );
    }
    let wall = driver.id_of("Wall").unwrap();
    let bear = driver.id_of("Bear").unwrap();
    let chosen = if warped {
        p160w_in_exile(&driver, "Timeline Culler").unwrap()
    } else {
        bear
    };
    driver.begin_cast("Close Encounter").unwrap();
    let offered = std::cell::RefCell::new(Vec::new());
    p160w_settle(&mut driver, 0, &|_, decision| match &decision.kind {
        DecisionKind::ChooseCards(options) => {
            offered.borrow_mut().clone_from(&options.from);
            Some(Answer::ChooseCards(CardsChoice(vec![chosen])))
        }
        _ => p160w_aim(decision, wall),
    });
    let mut expected = vec![bear];
    if warped {
        expected.push(chosen);
    }
    let mut seen = offered.into_inner();
    seen.sort_unstable();
    expected.sort_unstable();
    // A single candidate is taken without a question.
    if expected.len() > 1 {
        assert_eq!(
            seen, expected,
            "only the creature and the warped card are offered"
        );
    } else {
        assert!(seen.is_empty() || seen == expected);
    }
    assert_eq!(p160w_damage(&driver, wall), if warped { 2 } else { 3 });
    p160w_replays(&driver);
}

#[rstest]
fn martyrdom_moves_the_next_damage_and_only_its_caster_activates_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] shields: usize,
) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(free_creature("Martyr", 1, 5)),
                ready(plains()),
                ready(plains()),
                ready(plains()),
                ready(mountain()),
            ],
        )
        .hand(
            0,
            [card("Martyrdom").unwrap(), card("Lightning Bolt").unwrap()],
        );
    let mut driver = Driver::from_scenario(p160w_libraries(scenario, seats)).unwrap();
    let martyr = driver.id_of("Martyr").unwrap();
    driver.begin_cast("Martyrdom").unwrap();
    p160w_settle(&mut driver, 0, &|_, decision| p160w_aim(decision, martyr));
    for _ in 0..shields {
        driver.begin_activation("Martyr").unwrap();
        p160w_settle(&mut driver, 0, &|driver, decision| match &decision.kind {
            DecisionKind::ChooseTargets(_) => {
                Some(Answer::ChooseTargets(TargetChoice::Targets(vec![
                    driver.target_player(0),
                ])))
            }
            _ => None,
        });
    }
    driver.begin_cast("Lightning Bolt").unwrap();
    p160w_settle(&mut driver, 0, &|driver, decision| match &decision.kind {
        DecisionKind::ChooseTargets(_) => Some(Answer::ChooseTargets(TargetChoice::Targets(vec![
            driver.target_player(0),
        ]))),
        _ => None,
    });
    let moved = u32::try_from(shields).unwrap();
    assert_eq!(
        p160w_damage(&driver, martyr),
        moved,
        "each shield moves 1 damage"
    );
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life(),
        20 - (3 - i32::try_from(shields).unwrap())
    );
    // Another player is never offered the granted ability.
    driver.pass().unwrap();
    p160w_until_priority(&mut driver, 1);
    assert_eq!(
        driver.offered_abilities("Martyr").unwrap(),
        Vec::<usize>::new()
    );
    p160w_replays(&driver);
}

#[rstest]
fn master_warcraft_chooses_the_attackers_of_another_player_who_aims_them(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] attackers: usize,
) {
    let scenario = Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(
            0,
            [
                ready(free_creature("Guard", 1, 1)),
                ready(mountain()),
                ready(mountain()),
                ready(plains()),
                ready(plains()),
            ],
        )
        .battlefield(
            1,
            [
                ready(free_creature("Ogre", 3, 3)),
                ready(free_creature("Goblin", 1, 1)),
            ],
        )
        .hand(0, [card("Master Warcraft").unwrap()]);
    let scenario = (2..seats).fold(scenario, |scenario, seat| {
        scenario.battlefield(seat, [ready(free_creature("Sentry", 1, 1))])
    });
    let mut driver = Driver::from_scenario(p160w_libraries(scenario, seats)).unwrap();
    p160w_until_priority(&mut driver, 0);
    driver.begin_cast("Master Warcraft").unwrap();
    p160w_settle(&mut driver, 0, &p160w_none);
    let chosen: Vec<ObjectId> = ["Ogre", "Goblin"]
        .iter()
        .take(attackers)
        .map(|name| driver.id_of(name).unwrap())
        .collect();
    for _ in 0..40 {
        if matches!(
            driver.decision().unwrap().kind,
            DecisionKind::DeclareAttackers(_)
        ) {
            break;
        }
        driver.pass().unwrap();
    }
    let decision = driver.decision().unwrap().clone();
    let DecisionKind::DeclareAttackers(options) = &decision.kind else {
        panic!("attackers are asked");
    };
    assert_eq!(decision.player, pid(0), "the caster chooses the attackers");
    assert_eq!(options.attackers.len(), 2);
    let first = |object: ObjectId| {
        options
            .attackers
            .iter()
            .find(|option| option.object == object)
            .unwrap()
            .defenders[0]
    };
    let picks = chosen.iter().map(|id| (*id, first(*id))).collect();
    let events = driver.answer(Answer::DeclareAttackers(picks)).unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::R5081AttackersChosen { .. }))
    );
    if attackers > 0 {
        let decision = driver.decision().unwrap().clone();
        let DecisionKind::DeclareAttackers(options) = &decision.kind else {
            panic!("the active player aims the attackers");
        };
        assert_eq!(decision.player, pid(1));
        let offered: Vec<ObjectId> = options.attackers.iter().map(|o| o.object).collect();
        assert_eq!(offered, chosen);
        assert_eq!(options.required, chosen);
        // Leaving a chosen creature out is refused.
        assert!(driver.answer(Answer::DeclareAttackers(Vec::new())).is_err());
        let last = pid(if seats == 2 { 0 } else { seats - 1 });
        let aims = options
            .attackers
            .iter()
            .map(|option| (option.object, AttackTarget::Player(last)))
            .collect();
        driver.answer(Answer::DeclareAttackers(aims)).unwrap();
        let declared = driver.state().combat().attackers();
        assert_eq!(declared.len(), attackers);
        // The defending player's blocks are the caster's to declare.
        for _ in 0..40 {
            if matches!(
                driver.decision().unwrap().kind,
                DecisionKind::DeclareBlockers(_)
            ) {
                break;
            }
            driver.pass().unwrap();
        }
        assert_eq!(driver.decision().unwrap().player, pid(0));
    } else {
        assert_eq!(driver.state().combat().attackers().len(), 0);
    }
    p160w_replays(&driver);
}

/// A creature with protection from the color.
fn p160w_warded(color: Color) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Warded")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 4)
            .with_rules(Ability::Keyword(Keyword::Protection(Selector(vec![
                Pred::Color(color),
            ])))),
    )
}

#[rstest]
#[case::red_through_protection_from_red(true, Color::Red, 1)]
#[case::red_stopped_without_the_flame(false, Color::Red, 0)]
#[case::protection_from_blue_is_no_matter(true, Color::Blue, 1)]
#[case::protection_from_blue_without_the_flame(false, Color::Blue, 1)]
fn ghostly_flame_makes_red_damage_colorless(
    #[values(2, 3, 4)] seats: usize,
    #[case] flame: bool,
    #[case] warded: Color,
    #[case] dealt: u32,
) {
    let mut ours = vec![ready(mountain()), ready(mountain())];
    if flame {
        ours.push(ready(card("Ghostly Flame").unwrap()));
    }
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, ours)
        .battlefield(1, [ready(p160w_warded(warded))])
        .hand(0, [card("Boiling Earth").unwrap()]);
    let mut driver = Driver::from_scenario(p160w_libraries(scenario, seats)).unwrap();
    let target = driver.id_of("Warded").unwrap();
    driver.begin_cast("Boiling Earth").unwrap();
    p160w_settle(&mut driver, 0, &|_, decision| match &decision.kind {
        DecisionKind::ChooseOption(_) => Some(Driver::passive_answer(decision)),
        _ => None,
    });
    assert_eq!(p160w_damage(&driver, target), dealt);
    p160w_replays(&driver);
}

#[rstest]
fn ghostly_flame_still_leaves_targeting_to_the_color(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [ready(mountain()), ready(card("Ghostly Flame").unwrap())],
        )
        .battlefield(1, [ready(p160w_warded(Color::Red))])
        .hand(0, [card("Lightning Bolt").unwrap()]);
    let mut driver = Driver::from_scenario(p160w_libraries(scenario, seats)).unwrap();
    let warded = driver.id_of("Warded").unwrap();
    driver.begin_cast("Lightning Bolt").unwrap();
    let DecisionKind::ChooseTargets(options) = &driver.decision().unwrap().kind else {
        panic!("targets are asked");
    };
    assert!(
        options
            .legal
            .iter()
            .all(|target| target.object().is_none_or(|object| object.id != warded))
    );
}

/// What a spell does to the noncreature artifact Guardian Beast's controller has.
#[derive(Debug, Clone, Copy)]
enum P160wHit {
    Destroy,
    Steal,
    Enchant,
}

#[rstest]
#[case::destroyed_while_untapped(P160wHit::Destroy, false)]
#[case::destroyed_while_tapped(P160wHit::Destroy, true)]
#[case::stolen_while_untapped(P160wHit::Steal, false)]
#[case::stolen_while_tapped(P160wHit::Steal, true)]
#[case::enchanted_while_untapped(P160wHit::Enchant, false)]
#[case::enchanted_while_tapped(P160wHit::Enchant, true)]
fn guardian_beast_guards_noncreature_artifacts_while_untapped(
    #[values(2, 3, 4)] seats: usize,
    #[case] hit: P160wHit,
    #[case] beast_tapped: bool,
) {
    let beast = card("Guardian Beast").unwrap();
    let relic = Arc::new(CardDefinition::new("Relic").with_type(CardType::Artifact));
    let (spell, lands) = match hit {
        P160wHit::Destroy => ("Crumble", vec![forest()]),
        P160wHit::Steal => ("Invoke the Winds", vec![island(); 5]),
        P160wHit::Enchant => ("Akki War Paint", vec![mountain()]),
    };
    let scenario = Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(
            0,
            [
                if beast_tapped {
                    tapped(beast)
                } else {
                    ready(beast)
                },
                ready(relic),
            ],
        )
        .battlefield(1, lands.into_iter().map(ready).collect::<Vec<_>>())
        .hand(1, [card(spell).unwrap()]);
    let mut driver = Driver::from_scenario(p160w_libraries(scenario, seats)).unwrap();
    let relic = driver.id_of("Relic").unwrap();
    let offered = driver.begin_cast(spell).is_ok() && {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseTargets(options) => options
                .legal
                .iter()
                .any(|target| target.object().is_some_and(|object| object.id == relic)),
            _ => false,
        }
    };
    if offered {
        p160w_settle(&mut driver, 1, &|_, decision| p160w_aim(decision, relic));
    }
    let state = driver.state();
    let here = state
        .object(relic)
        .is_some_and(|o| o.zone() == Zone::Battlefield);
    let guarded = !beast_tapped;
    match hit {
        P160wHit::Destroy => assert_eq!(here, guarded, "indestructible while untapped"),
        P160wHit::Steal => {
            assert!(here);
            let controller = mtg_engine::characteristics::controller_of(state, relic).unwrap();
            assert_eq!(controller == pid(0), guarded, "kept while untapped");
        }
        P160wHit::Enchant => {
            assert_eq!(offered, !guarded, "an Aura can't target it while untapped");
        }
    }
    p160w_replays(&driver);
}
