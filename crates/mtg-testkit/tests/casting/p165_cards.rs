//! Goal: the p165 cards play whole at two to four seats: Tawnos's Coffin exiles a creature with its Auras and returns it
//! tapped with the counters noted and the Auras attached once the Coffin untaps; Takklemaggot, when its creature dies,
//! returns attached to a creature that creature's controller chooses, or as a non-Aura enchantment that deals 1 damage to
//! that player at each of their upkeeps; Psychic Battle lets the player who reveals the greatest mana value change the
//! targets just chosen, a tie changes nothing and its own change does not trigger it again; Miss Highwater's deal draws a
//! fresh seven with a contract counter, and the player's loss gives her controller copies of their artifacts and
//! creatures; each game replays and each card is Verified with the engine's capabilities.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::card::Subtype;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature_costing, free_creature, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, attached_to, ready, with_counters};
use rstest::rstest;

use crate::common::pid;

fn p165_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn p165_libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, vec![island(); 12])
    })
}

fn p165_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn p165_counters(driver: &Driver, id: ObjectId, kind: &CounterKind) -> u32 {
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(kind))
}

fn p165_zone(driver: &Driver, id: ObjectId) -> Option<Zone> {
    driver
        .state()
        .object(id)
        .map(mtg_engine::object::GameObject::zone)
}

/// The permanents named `name` that `seat` controls.
fn p165_named(driver: &Driver, name: &str, seat: usize) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().name == name && object.controller() == pid(seat))
        .count()
}

/// Answers `pick` gives first, then mana automatically, the first legal targets and passes, `steps` times at most or
/// until `stop` holds.
fn p165_run(
    driver: &mut Driver,
    steps: usize,
    pick: &dyn Fn(&Driver, &Decision) -> Option<Answer>,
    stop: &dyn Fn(&Driver) -> bool,
) {
    for _ in 0..steps {
        if stop(driver) || driver.state().result().is_some() {
            return;
        }
        let decision = driver.decision().unwrap().clone();
        if let Some(answer) = pick(driver, &decision) {
            driver.answer(answer).unwrap();
            continue;
        }
        let answer = match &decision.kind {
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
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
}

fn p165_at(turn: u32, step: Step) -> impl Fn(&Driver) -> bool {
    move |driver: &Driver| {
        let now = driver.state().turn();
        (now.number() == turn && now.step() >= step) || now.number() > turn
    }
}

fn p165_none(_: &Driver, _: &Decision) -> Option<Answer> {
    None
}

#[rstest]
#[case::bare(0, false)]
#[case::with_counters(2, false)]
#[case::with_an_aura(0, true)]
#[case::with_both(3, true)]
fn tawnoss_coffin_returns_the_creature_tapped_with_its_counters_and_auras(
    #[values(2, 3, 4)] seats: usize,
    #[case] plus: u32,
    #[case] aura: bool,
) {
    let victim = with_counters(
        ready(free_creature("Bear", 2, 2)),
        CounterKind::PlusOnePlusOne,
        plus,
    );
    let mut theirs = vec![victim];
    if aura {
        theirs.push(attached_to(ready(card("Pacifism").unwrap()), 1, 0));
    }
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Tawnos's Coffin").unwrap()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .battlefield(1, theirs);
    let mut driver = Driver::from_scenario(p165_libraries(scenario, seats)).unwrap();
    let bear = driver.id_of("Bear").unwrap();
    let pacifism = aura.then(|| driver.id_of("Pacifism").unwrap());
    driver.begin_activation("Tawnos's Coffin").unwrap();
    p165_run(&mut driver, 30, &p165_none, &|driver| {
        p165_zone(driver, bear) == Some(Zone::Exile)
            && matches!(
                driver.decision().map(|d| &d.kind),
                Ok(DecisionKind::Priority(_))
            )
            && driver.state().stack().is_empty()
    });
    assert_eq!(p165_zone(&driver, bear), Some(Zone::Exile));
    if let Some(pacifism) = pacifism {
        assert_eq!(
            p165_zone(&driver, pacifism),
            Some(Zone::Exile),
            "the Aura goes too"
        );
    }
    // The Coffin untaps in its controller's next untap step, and the creature comes back.
    let turn = 2 + u32::try_from(seats).unwrap();
    p165_run(&mut driver, 400, &p165_none, &p165_at(turn, Step::Draw));
    assert_eq!(p165_zone(&driver, bear), Some(Zone::Battlefield));
    let back = driver.state().object(bear).unwrap();
    assert!(back.permanent().unwrap().tapped(), "it returns tapped");
    assert_eq!(back.controller(), pid(1), "under its owner's control");
    assert_eq!(
        p165_counters(&driver, bear, &CounterKind::PlusOnePlusOne),
        plus
    );
    if let Some(pacifism) = pacifism {
        assert_eq!(p165_zone(&driver, pacifism), Some(Zone::Battlefield));
        assert_eq!(
            driver.state().host_of(pacifism),
            Some(bear),
            "attached to the creature"
        );
    }
    p165_replays(&driver);
}

#[rstest]
#[case::onto_another_creature(true)]
#[case::as_a_plain_enchantment(false)]
fn takklemaggot_moves_on_or_burns_the_last_controller(
    #[values(2, 3, 4)] seats: usize,
    #[case] other: bool,
) {
    let mut mine = vec![ready(card("Takklemaggot").unwrap())];
    if other {
        mine.insert(0, ready(free_creature("Bear", 2, 2)));
    }
    let maggot_place = usize::from(other);
    let mine: Vec<_> = mine
        .into_iter()
        .enumerate()
        .map(|(at, permanent)| {
            if at == maggot_place {
                attached_to(permanent, 1, 0)
            } else {
                permanent
            }
        })
        .collect();
    // A 0/1 dies to the first -0/-1 counter, at its controller's upkeep.
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine)
        .battlefield(1, [ready(free_creature("Wisp", 0, 1))]);
    let mut driver = Driver::from_scenario(p165_libraries(scenario, seats)).unwrap();
    let maggot = driver.id_of("Takklemaggot").unwrap();
    let wisp = driver.id_of("Wisp").unwrap();
    p165_run(&mut driver, 200, &p165_none, &p165_at(2, Step::Draw));
    assert!(matches!(p165_zone(&driver, wisp), Some(Zone::Graveyard(_))));
    assert_eq!(p165_zone(&driver, maggot), Some(Zone::Battlefield));
    let back = driver.state().object(maggot).unwrap();
    assert_eq!(back.controller(), pid(0), "under its controller's control");
    let back_subtypes = back.card().subtypes.clone();
    let chars = mtg_engine::characteristics::characteristics(driver.state(), maggot).unwrap();
    if other {
        let bear = driver.id_of("Bear").unwrap();
        assert_eq!(driver.state().host_of(maggot), Some(bear));
        assert!(chars.has_subtype(&back_subtypes, &Subtype("Aura".into())));
        return p165_replays(&driver);
    }
    assert_eq!(driver.state().host_of(maggot), None);
    assert!(
        !chars.has_subtype(&back_subtypes, &Subtype("Aura".into())),
        "a non-Aura enchantment"
    );
    // At each of that player's upkeeps it deals them 1 damage.
    let life = p165_life(&driver, 1);
    let next = 2 + u32::try_from(seats).unwrap();
    p165_run(&mut driver, 400, &p165_none, &p165_at(next, Step::Draw));
    assert_eq!(p165_life(&driver, 1), life - 1);
    let others: Vec<i32> = (0..seats)
        .filter(|seat| *seat != 1)
        .map(|seat| p165_life(&driver, seat))
        .collect();
    assert!(
        others.iter().all(|life| *life == 20),
        "nobody else is dealt damage"
    );
    p165_replays(&driver);
}

/// A library whose top card has this mana value (a land for 0).
fn p165_topped(scenario: Scenario, seat: usize, value: u32) -> Scenario {
    let top = if value == 0 {
        island()
    } else {
        creature_costing(
            "Top",
            &vec![mtg_core::mana::ManaSymbol::Generic(1); value as usize],
            1,
            1,
        )
    };
    let mut library = vec![top];
    library.extend(vec![island(); 8]);
    scenario.library(seat, library)
}

#[rstest]
#[case::the_caster_wins(&[1, 5], Some(1))]
#[case::psychic_battle_s_owner_wins(&[6, 2], Some(0))]
#[case::a_tie_changes_nothing(&[4, 4], None)]
#[case::a_tie_for_greatest_with_a_lower(&[3, 3, 1], None)]
#[case::the_third_player_wins(&[0, 1, 7], Some(2))]
fn psychic_battle_lets_the_greatest_reveal_change_the_targets(
    #[case] tops: &[u32],
    #[case] winner: Option<usize>,
) {
    let seats = tops.len().max(2);
    let mut scenario = Scenario::new(seats)
        .at(2, 1, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Psychic Battle").unwrap()),
                ready(free_creature("Bear", 2, 2)),
            ],
        )
        .battlefield(1, [ready(free_creature("Cub", 2, 2)), ready(mountain())])
        .hand(1, [card("Lightning Bolt").unwrap()]);
    for (seat, value) in tops.iter().enumerate() {
        scenario = p165_topped(scenario, seat, *value);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let bear = driver.id_of("Bear").unwrap();
    let cub = driver.id_of("Cub").unwrap();
    let target = driver.target_named("Bear").unwrap();
    driver.cast_at("Lightning Bolt", &[target]).unwrap();
    let asked = std::cell::Cell::new(0);
    let changed_to = driver.target_named("Cub").unwrap();
    p165_run(
        &mut driver,
        60,
        &|_, decision| match &decision.kind {
            DecisionKind::YesNo(_) => {
                assert_eq!(
                    Some(decision.player),
                    winner.map(pid),
                    "only the winner is asked"
                );
                asked.set(asked.get() + 1);
                Some(Answer::YesNo(YesNoChoice(true)))
            }
            DecisionKind::ChooseTargets(options) if !options.can_cancel => {
                assert_eq!(Some(decision.player), winner.map(pid));
                Some(Answer::ChooseTargets(TargetChoice::Targets(vec![
                    changed_to,
                ])))
            }
            _ => None,
        },
        &|driver| {
            driver.state().stack().is_empty() && p165_zone(driver, bear) != Some(Zone::Battlefield)
                || driver.state().stack().is_empty()
                    && p165_zone(driver, cub) != Some(Zone::Battlefield)
        },
    );
    assert_eq!(
        asked.get(),
        usize::from(winner.is_some()),
        "it triggers once: its own change is quiet"
    );
    let (dead, alive) = if winner.is_some() {
        (cub, bear)
    } else {
        (bear, cub)
    };
    assert!(matches!(p165_zone(&driver, dead), Some(Zone::Graveyard(_))));
    assert_eq!(p165_zone(&driver, alive), Some(Zone::Battlefield));
    p165_replays(&driver);
}

#[rstest]
#[case::declined(false)]
#[case::accepted(true)]
fn miss_highwater_offers_a_fresh_seven_for_a_contract(
    #[values(2, 3, 4)] seats: usize,
    #[case] deal: bool,
) {
    let contract = CounterKind::Other("contract".into());
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(card("Miss Highwater").unwrap())])
        .battlefield(
            1,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Cub", 1, 1)),
                ready(card("Sol Ring").unwrap()),
                ready(island()),
            ],
        )
        .hand(1, [island(), island()]);
    let mut driver = Driver::from_scenario(p165_libraries(scenario, seats)).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Miss Highwater", 1)]).unwrap();
    p165_run(
        &mut driver,
        60,
        &|_, decision| match &decision.kind {
            DecisionKind::YesNo(_) => {
                assert_eq!(decision.player, pid(1), "the player dealt damage decides");
                Some(Answer::YesNo(YesNoChoice(deal)))
            }
            _ => None,
        },
        &p165_at(2, Step::EndCombat),
    );
    let hand = driver.state().zone(Zone::Hand(pid(1))).len();
    let counters = driver
        .state()
        .player(pid(1))
        .unwrap()
        .counters_of(&contract);
    assert_eq!(hand, if deal { 7 } else { 2 });
    assert_eq!(counters, u32::from(deal));
    if seats > 2 {
        driver.concede(PlayerId(1)).unwrap();
        p165_run(&mut driver, 30, &p165_none, &|driver| {
            driver.state().stack().is_empty()
        });
        let copies = |name: &str| p165_named(&driver, name, 0);
        let made = usize::from(deal);
        assert_eq!(
            (
                copies("Bear"),
                copies("Cub"),
                copies("Sol Ring"),
                copies("Island")
            ),
            (made, made, made, 0)
        );
    }
    p165_replays(&driver);
}

#[rstest]
#[case("Tawnos's Coffin")]
#[case("Takklemaggot")]
#[case("Psychic Battle")]
#[case("Miss Highwater")]
fn p165_the_cards_are_verified(#[case] name: &str) {
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
