//! Goal: replacement effects apply as the rules say, at tables of two, three and four seats: the tokens one effect makes, the counters put on a permanent (an effect's, a creature's own as it enters, a planeswalker's loyalty but not what its ability pays), the damage a source deals, the life a player gains and the cards a player draws are changed by the effects of the permanents in play; each effect applies once, in the order the affected player chooses when two or more apply (asked exactly then), and to what the one before it left; an effect of another player's permanent does not apply to mine, an effect that left play applies no more, and one that changes control applies for its new controller; Rest in Peace exiles what would be put into a graveyard, from a destroyed creature to a discard, a countered spell and a token; and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, names, pid};
use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{ReplacementChoice, ReplacementOptions};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::events::ZoneCause;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, characteristics, replay};
use mtg_testkit::cards::{
    bear, burn, forest, free_creature, island, mind_rot, mountain, plains, study, swamp,
};
use mtg_testkit::corpus::card;
use mtg_testkit::counter_cards::growth_spurt;
use mtg_testkit::replace_cards::{mend, raise_soldiers};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

const PLUS: CounterKind = CounterKind::PlusOnePlusOne;

type Pick<'a> = &'a dyn Fn(&Driver, &ReplacementOptions) -> usize;

/// The first effect on offer.
fn first(_: &Driver, _: &ReplacementOptions) -> usize {
    0
}

/// The effect of the first of these cards that is on offer.
fn prefer<'a>(order: &'a [&'a str]) -> impl Fn(&Driver, &ReplacementOptions) -> usize + 'a {
    move |driver, options| {
        order
            .iter()
            .find_map(|want| {
                options
                    .candidates
                    .iter()
                    .position(|c| name_of(driver.state(), c.source) == *want)
            })
            .unwrap_or(0)
    }
}

/// Passes priority until the stack is empty, answering each question of the replacement step with `pick`; returns how
/// many times it was asked.
fn settle(driver: &mut Driver, pick: Pick<'_>) -> Result<u32, ScriptError> {
    let mut asked = 0;
    for _ in 0..400 {
        let Some(decision) = driver.engine().decision().cloned() else {
            return Ok(asked);
        };
        match decision.kind {
            DecisionKind::ChooseReplacement(options) => {
                asked += 1;
                let choice = pick(driver, &options);
                driver.answer(Answer::ChooseReplacement(ReplacementChoice(choice)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(asked);
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn count(driver: &Driver, zone: Zone, name: &str) -> usize {
    names(driver.state(), zone)
        .iter()
        .filter(|found| *found == name)
        .count()
}

fn counters_on(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(kind))
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn replays(driver: &Driver, setup: &mtg_engine::GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn permanents(names: &[&str]) -> Vec<mtg_engine::PresetPermanent> {
    names
        .iter()
        .map(|name| ready(card(name).unwrap()))
        .collect()
}

/// What an effect does to a number.
#[derive(Clone, Copy)]
enum Change {
    Add(u32),
    Times(u32),
}

fn changed(start: u32, order: &[Change]) -> u32 {
    order.iter().fold(start, |now, change| match change {
        Change::Add(more) => now + more,
        Change::Times(times) => now * times,
    })
}

/// The change each card of the corpus makes to the +1/+1 counters put on a creature of its controller.
fn counter_change(name: &str) -> Change {
    match name {
        "Hardened Scales" => Change::Add(1),
        _ => Change::Times(2),
    }
}

// Tokens

#[rstest]
#[case::none(&[], 2, 0)]
#[case::one_procession(&["Anointed Procession"], 4, 0)]
#[case::parallel_lives(&["Parallel Lives"], 4, 0)]
#[case::doubling_season(&["Doubling Season"], 4, 0)]
#[case::two_doublers(&["Anointed Procession", "Parallel Lives"], 8, 1)]
#[case::two_of_a_kind(&["Anointed Procession", "Anointed Procession"], 8, 1)]
#[case::three_doublers(&["Anointed Procession", "Parallel Lives", "Doubling Season"], 16, 2)]
#[case::four_doublers(
    &["Anointed Procession", "Anointed Procession", "Parallel Lives", "Doubling Season"],
    32,
    3
)]
fn each_doubler_of_tokens_applies_once_and_the_order_is_asked_when_two_or_more_apply(
    #[values(2, 3, 4)] seats: usize,
    #[case] doublers: &[&str],
    #[case] soldiers: usize,
    #[case] asks: u32,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [raise_soldiers().unwrap()])
            .battlefield(0, permanents(doublers).into_iter().chain([ready(plains())]))
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Raise Soldiers")?;
    assert_eq!(settle(&mut driver, &first)?, asks);
    assert_eq!(count(&driver, Zone::Battlefield, "Soldier"), soldiers);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_doubler_of_another_player_does_not_double_my_tokens(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [raise_soldiers().unwrap()])
            .battlefield(0, [ready(plains())])
            .battlefield(1, permanents(&["Anointed Procession", "Doubling Season"]))
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Raise Soldiers")?;
    assert_eq!(settle(&mut driver, &first)?, 0);
    assert_eq!(count(&driver, Zone::Battlefield, "Soldier"), 2);
    Ok(())
}

#[test]
fn tokens_made_for_several_players_are_doubled_for_each_controller_of_a_doubler()
-> Result<(), ScriptError> {
    // Hero of Bladehold makes two tokens for its controller each time it attacks.
    let scenario = || {
        Scenario::new(2).at(1, 0, Step::Main1).battlefield(
            0,
            [
                ready(card("Hero of Bladehold").unwrap()),
                ready(card("Anointed Procession").unwrap()),
            ],
        )
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Hero of Bladehold", 1)])?;
    settle(&mut driver, &first)?;
    // Two tokens, doubled, enter attacking.
    assert_eq!(count(&driver, Zone::Battlefield, "Soldier"), 4);
    Ok(())
}

// Counters

#[rstest]
#[case::none(&[], &[])]
#[case::scales(&["Hardened Scales"], &["Hardened Scales"])]
#[case::season(&["Doubling Season"], &["Doubling Season"])]
#[case::corpsejack(&["Corpsejack Menace"], &["Corpsejack Menace"])]
#[case::two_scales(&["Hardened Scales", "Hardened Scales"], &[])]
#[case::scales_then_corpsejack(
    &["Hardened Scales", "Corpsejack Menace"],
    &["Hardened Scales", "Corpsejack Menace"]
)]
#[case::corpsejack_then_scales(
    &["Hardened Scales", "Corpsejack Menace"],
    &["Corpsejack Menace", "Hardened Scales"]
)]
#[case::season_then_scales(
    &["Hardened Scales", "Doubling Season"],
    &["Doubling Season", "Hardened Scales"]
)]
#[case::scales_then_season(
    &["Hardened Scales", "Doubling Season"],
    &["Hardened Scales", "Doubling Season"]
)]
#[case::all_three(
    &["Hardened Scales", "Corpsejack Menace", "Doubling Season"],
    &["Corpsejack Menace", "Hardened Scales", "Doubling Season"]
)]
fn counters_put_on_a_creature_are_changed_once_by_each_effect_in_the_order_chosen(
    #[values(2, 3, 4)] seats: usize,
    #[case] effects: &[&str],
    #[case] order: &[&str],
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [growth_spurt().unwrap()])
            .battlefield(
                0,
                permanents(effects)
                    .into_iter()
                    .chain([ready(free_creature("Grizzly Bears", 2, 2)), ready(forest())]),
            )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let bears = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Growth Spurt", &[bears])?;
    let pick = prefer(order);
    let asked = settle(&mut driver, &pick)?;
    // The effects apply in the order they are preferred; those with no preference come after them.
    let mut applied: Vec<Change> = order.iter().map(|name| counter_change(name)).collect();
    let mut listed = order.to_vec();
    for name in effects {
        match listed.iter().position(|seen| seen == name) {
            Some(at) => {
                listed.remove(at);
            }
            None => applied.push(counter_change(name)),
        }
    }
    assert_eq!(
        counters_on(&driver, "Grizzly Bears", &PLUS),
        changed(1, &applied)
    );
    assert_eq!(
        asked,
        u32::try_from(effects.len().saturating_sub(1)).unwrap()
    );
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn the_order_changes_the_counters_in_the_way_the_rules_say() -> Result<(), ScriptError> {
    // One counter, plus one and then doubled is four; doubled and then plus one is three.
    for (order, counters) in [
        (["Hardened Scales", "Corpsejack Menace"], 4),
        (["Corpsejack Menace", "Hardened Scales"], 3),
    ] {
        let mut driver = Driver::from_scenario(
            Scenario::new(2)
                .hand(0, [growth_spurt().unwrap()])
                .battlefield(
                    0,
                    permanents(&["Hardened Scales", "Corpsejack Menace"])
                        .into_iter()
                        .chain([ready(free_creature("Grizzly Bears", 2, 2)), ready(forest())]),
                ),
        )?;
        let bears = driver.target_named("Grizzly Bears")?;
        driver.cast_at("Growth Spurt", &[bears])?;
        settle(&mut driver, &prefer(&order))?;
        assert_eq!(
            counters_on(&driver, "Grizzly Bears", &PLUS),
            counters,
            "{order:?}"
        );
    }
    Ok(())
}

#[rstest]
fn counters_on_a_creature_of_another_player_are_not_changed(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [growth_spurt().unwrap()])
            .battlefield(0, [ready(forest())])
            .battlefield(
                1,
                permanents(&["Hardened Scales", "Corpsejack Menace"])
                    .into_iter()
                    .chain([ready(bear())]),
            ),
    )?;
    let target = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Growth Spurt", &[target])?;
    // The effects are another player's, so they apply to that player's creature and not to the spell's controller's;
    // the counter goes on a creature of seat 1, which they do change: asked once, then four or three.
    let asked = settle(
        &mut driver,
        &prefer(&["Hardened Scales", "Corpsejack Menace"]),
    )?;
    assert_eq!(asked, 1);
    assert_eq!(counters_on(&driver, "Grizzly Bears", &PLUS), 4);
    Ok(())
}

#[test]
fn an_effect_that_left_play_changes_nothing_and_one_that_changed_hands_changes_for_its_new_controller()
-> Result<(), ScriptError> {
    // Corpsejack Menace doubles my counters: the Menace dies to a bolt and the next counter is single.
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(
                0,
                [
                    growth_spurt().unwrap(),
                    growth_spurt().unwrap(),
                    burn("Bolt", 4),
                ],
            )
            .battlefield(
                0,
                permanents(&["Corpsejack Menace"]).into_iter().chain([
                    ready(forest()),
                    ready(forest()),
                    ready(mountain()),
                    ready(free_creature("Grizzly Bears", 2, 2)),
                ]),
            ),
    )?;
    let bears = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Growth Spurt", &[bears])?;
    settle(&mut driver, &first)?;
    assert_eq!(counters_on(&driver, "Grizzly Bears", &PLUS), 2);
    let menace = driver.target_named("Corpsejack Menace")?;
    driver.cast_at("Bolt", &[menace])?;
    settle(&mut driver, &first)?;
    assert_eq!(count(&driver, Zone::Battlefield, "Corpsejack Menace"), 0);
    driver.cast_at("Growth Spurt", &[bears])?;
    settle(&mut driver, &first)?;
    assert_eq!(counters_on(&driver, "Grizzly Bears", &PLUS), 3);
    // The opponent's Menace changes my counters after Control Magic gives it to me.
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(
                0,
                [
                    growth_spurt().unwrap(),
                    growth_spurt().unwrap(),
                    card("Control Magic").unwrap(),
                ],
            )
            .battlefield(
                0,
                [
                    ready(forest()),
                    ready(forest()),
                    ready(island()),
                    ready(island()),
                    ready(island()),
                    ready(island()),
                    ready(free_creature("Grizzly Bears", 2, 2)),
                ],
            )
            .battlefield(1, permanents(&["Corpsejack Menace"])),
    )?;
    let bears = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Growth Spurt", &[bears])?;
    settle(&mut driver, &first)?;
    assert_eq!(counters_on(&driver, "Grizzly Bears", &PLUS), 1);
    let menace = driver.target_named("Corpsejack Menace")?;
    driver.cast_at("Control Magic", &[menace])?;
    settle(&mut driver, &first)?;
    driver.cast_at("Growth Spurt", &[bears])?;
    settle(&mut driver, &first)?;
    assert_eq!(counters_on(&driver, "Grizzly Bears", &PLUS), 3);
    Ok(())
}

// A creature that enters with counters

#[rstest]
#[case::alone(&[], &[], 3)]
#[case::scales(&["Hardened Scales"], &[], 4)]
#[case::corpsejack(&["Corpsejack Menace"], &[], 6)]
#[case::scales_first(&["Hardened Scales", "Corpsejack Menace"], &["Hardened Scales", "Corpsejack Menace"], 8)]
#[case::corpsejack_first(&["Hardened Scales", "Corpsejack Menace"], &["Corpsejack Menace", "Hardened Scales"], 7)]
#[case::season(&["Doubling Season"], &[], 6)]
fn a_creature_that_enters_with_counters_has_them_changed_by_the_effects_that_apply_to_them(
    #[values(2, 3, 4)] seats: usize,
    #[case] effects: &[&str],
    #[case] order: &[&str],
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [card("Faithful Watchdog").unwrap()])
            .battlefield(
                0,
                permanents(effects)
                    .into_iter()
                    .chain([ready(forest()), ready(plains())]),
            )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Faithful Watchdog")?;
    settle(&mut driver, &prefer(order))?;
    assert_eq!(counters_on(&driver, "Faithful Watchdog", &PLUS), counters);
    let chars = characteristics(driver.state(), driver.id_of("Faithful Watchdog")?).unwrap();
    let size = i32::try_from(counters).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(size), Some(size)));
    replays(&driver, &setup);
    Ok(())
}

// A planeswalker enters with loyalty

#[rstest]
#[case::alone(&[], 3, 5)]
#[case::season(&["Doubling Season"], 6, 8)]
#[case::scales_and_corpsejack_leave_loyalty_alone(&["Hardened Scales", "Corpsejack Menace"], 3, 5)]
fn a_planeswalker_enters_with_loyalty_that_a_counter_doubler_doubles_and_pays_a_plus_cost_undoubled(
    #[values(2, 3, 4)] seats: usize,
    #[case] effects: &[&str],
    #[case] entered: u32,
    #[case] after_plus: u32,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [card("Jace Beleren").unwrap()])
            .battlefield(
                0,
                permanents(effects).into_iter().chain([
                    ready(island()),
                    ready(island()),
                    ready(island()),
                ]),
            )
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Jace Beleren")?;
    settle(&mut driver, &first)?;
    assert_eq!(
        counters_on(&driver, "Jace Beleren", &CounterKind::Loyalty),
        entered
    );
    driver.begin_ability("Jace Beleren", 0)?;
    settle(&mut driver, &first)?;
    assert_eq!(
        counters_on(&driver, "Jace Beleren", &CounterKind::Loyalty),
        after_plus
    );
    Ok(())
}

// Damage

#[rstest]
#[case::none(&[], 3)]
#[case::one_furnace(&["Furnace of Rath"], 6)]
#[case::two_furnaces(&["Furnace of Rath", "Furnace of Rath"], 12)]
fn damage_a_source_deals_is_doubled_once_by_each_furnace(
    #[values(2, 3, 4)] seats: usize,
    #[case] furnaces: &[&str],
    #[case] damage: i32,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, [ready(mountain())])
            .battlefield(1, permanents(furnaces))
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let foe = driver.target_player(1);
    driver.cast_at("Bolt", &[foe])?;
    let asked = settle(&mut driver, &first)?;
    assert_eq!(life(&driver, 1), 20 - damage);
    assert_eq!(asked, u32::from(furnaces.len() > 1));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn combat_damage_is_doubled_and_a_lifelinker_gains_the_doubled_life_doubled_again(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Rhox Faithmender has lifelink and doubles the life its controller gains; Furnace of Rath doubles damage.
    let scenario = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, permanents(&["Rhox Faithmender", "Furnace of Rath"]))
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Rhox Faithmender", 1)])?;
    for _ in 0..40 {
        if driver.state().turn().step() == Step::EndCombat {
            break;
        }
        match &driver.decision()?.kind {
            DecisionKind::ChooseReplacement(_) => {
                driver.answer(Answer::ChooseReplacement(ReplacementChoice(0)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    // One damage becomes two, and the two life it gains become four.
    assert_eq!(life(&driver, 1), 18);
    assert_eq!(life(&driver, 0), 24);
    Ok(())
}

// Life

#[rstest]
#[case::none(&[], 3, 0)]
#[case::one(&["Rhox Faithmender"], 6, 0)]
#[case::two(&["Rhox Faithmender", "Rhox Faithmender"], 12, 1)]
fn life_gained_is_doubled_once_by_each_effect(
    #[values(2, 3, 4)] seats: usize,
    #[case] effects: &[&str],
    #[case] gained: i32,
    #[case] asks: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [mend().unwrap()])
            .battlefield(0, permanents(effects).into_iter().chain([ready(plains())])),
    )?;
    driver.cast("Mend")?;
    assert_eq!(settle(&mut driver, &first)?, asks);
    assert_eq!(life(&driver, 0), 20 + gained);
    Ok(())
}

#[rstest]
#[case::one_more(&["Angel of Vitality"], 4)]
#[case::one_more_from_two_cards(&["Angel of Vitality", "Honor Troll"], 5)]
#[case::doubled(&["Boon Reflection"], 6)]
#[case::nobody_gains(&["Giant Cindermaw"], 0)]
#[case::only_opponents_cannot_gain(&["Knight of Dusk's Shadow"], 3)]
fn life_gained_with_the_corpus_cards_that_change_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] effects: &[&str],
    #[case] gained: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [mend().unwrap()])
            .battlefield(0, permanents(effects).into_iter().chain([ready(plains())])),
    )?;
    driver.cast("Mend")?;
    settle(&mut driver, &first)?;
    assert_eq!(life(&driver, 0), 20 + gained);
    Ok(())
}

#[rstest]
#[case::opponents_cannot_gain("Knight of Dusk's Shadow", 0)]
#[case::nobody_gains("Giant Cindermaw", 0)]
#[case::one_more_is_theirs("Angel of Vitality", 3)]
#[case::doubling_is_theirs("Boon Reflection", 3)]
fn the_cards_of_another_seat_stop_my_life_gain_only_when_they_name_me(
    #[values(2, 3, 4)] seats: usize,
    #[case] other: &str,
    #[case] gained: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [mend().unwrap()])
            .battlefield(0, [ready(plains())])
            .battlefield(1, permanents(&[other])),
    )?;
    driver.cast("Mend")?;
    settle(&mut driver, &first)?;
    assert_eq!(life(&driver, 0), 20 + gained);
    Ok(())
}

// Draws

#[rstest]
fn each_card_drawn_is_two_with_thought_reflection_and_they_are_the_top_cards_in_order(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [study("Study")])
            .battlefield(
                0,
                permanents(&["Thought Reflection"])
                    .into_iter()
                    .chain([ready(island())]),
            )
            .library(
                0,
                (0..10).map(|n| free_creature(&format!("Card {n}"), 1, 1)),
            )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let begun = driver.engine().events().len();
    driver.cast("Study")?;
    settle(&mut driver, &first)?;
    // Draw two twice: four cards, the four on top of the library, each only once.
    let drawn: Vec<ObjectId> = driver.engine().events()[begun..]
        .iter()
        .filter_map(|event| match event {
            Event::CardDrawn { object, .. } => Some(*object),
            _ => None,
        })
        .collect();
    assert_eq!(drawn.len(), 4);
    let mut distinct = drawn.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), 4);
    assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), 6);
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))).len(), 4);
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn drawing_more_cards_than_the_library_holds_fails_the_draws_that_are_left()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [study("Study")])
            .battlefield(
                0,
                permanents(&["Thought Reflection"])
                    .into_iter()
                    .chain([ready(island())]),
            )
            .library(0, [bear()]),
    )?;
    let begun = driver.engine().events().len();
    driver.cast("Study")?;
    settle(&mut driver, &first)?;
    let log = &driver.engine().events()[begun..];
    let drawn = log
        .iter()
        .filter(|e| matches!(e, Event::CardDrawn { .. }))
        .count();
    let failed = log
        .iter()
        .filter(|e| matches!(e, Event::DrewFromEmptyLibrary { .. }))
        .count();
    assert_eq!((drawn, failed), (1, 3));
    Ok(())
}

// Rest in Peace

fn graveyard_entries(driver: &Driver, from: usize) -> usize {
    driver.engine().events()[from..]
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::CreatureDied { .. }
                    | Event::CardDiscarded { .. }
                    | Event::ZoneChanged {
                        to: Zone::Graveyard(_),
                        ..
                    }
            )
        })
        .count()
}

#[rstest]
fn rest_in_peace_exiles_a_creature_that_dies_a_spell_that_resolves_and_one_that_is_countered(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [burn("Bolt", 4), card("Doom Blade").unwrap()])
            .battlefield(
                0,
                [
                    ready(swamp()),
                    ready(swamp()),
                    ready(mountain()),
                    ready(card("Rest in Peace").unwrap()),
                ],
            )
            .battlefield(
                1,
                [
                    ready(free_creature("Grizzly Bears", 2, 2)),
                    ready(free_creature("Hill Giant", 3, 3)),
                ],
            )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let begun = driver.engine().events().len();
    let giant = driver.target_named("Hill Giant")?;
    driver.cast_at("Bolt", &[giant])?;
    settle(&mut driver, &first)?;
    let bears = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Doom Blade", &[bears])?;
    settle(&mut driver, &first)?;
    assert_eq!(
        graveyard_entries(&driver, begun),
        0,
        "{:?}",
        driver.engine().events()[begun..].to_vec()
    );
    for seat in 0..seats {
        assert_eq!(
            names(driver.state(), Zone::Graveyard(pid(seat))),
            Vec::<String>::new()
        );
    }
    let exiled = names(driver.state(), Zone::Exile);
    for card in ["Hill Giant", "Grizzly Bears", "Bolt", "Doom Blade"] {
        assert!(exiled.contains(&card.to_owned()), "{card} in {exiled:?}");
    }
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn a_token_that_dies_under_rest_in_peace_goes_to_exile_and_never_to_a_graveyard()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [raise_soldiers().unwrap(), burn("Bolt", 3)])
            .battlefield(
                0,
                [
                    ready(plains()),
                    ready(mountain()),
                    ready(card("Rest in Peace").unwrap()),
                ],
            ),
    )?;
    driver.cast("Raise Soldiers")?;
    settle(&mut driver, &first)?;
    let begun = driver.engine().events().len();
    let soldier = driver.target_named("Soldier")?;
    driver.cast_at("Bolt", &[soldier])?;
    settle(&mut driver, &first)?;
    assert_eq!(count(&driver, Zone::Battlefield, "Soldier"), 1);
    assert_eq!(graveyard_entries(&driver, begun), 0);
    Ok(())
}

#[test]
fn a_discarded_card_is_exiled_and_when_rest_in_peace_is_gone_the_next_one_is_not()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [mind_rot("Mind Rot"), card("Doom Blade").unwrap()])
            .hand(1, [bear(), bear(), forest()])
            .battlefield(
                0,
                [
                    ready(swamp()),
                    ready(swamp()),
                    ready(swamp()),
                    ready(swamp()),
                ],
            )
            .battlefield(1, [ready(card("Rest in Peace").unwrap())]),
    )?;
    let begun = driver.engine().events().len();
    let foe = driver.target_player(1);
    driver.cast_at("Mind Rot", &[foe])?;
    // Whatever the discard asks, the first cards are given up.
    for _ in 0..30 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        match &driver.decision()?.kind {
            DecisionKind::ChooseReplacement(_) => {
                driver.answer(Answer::ChooseReplacement(ReplacementChoice(0)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(graveyard_entries(&driver, begun), 0);
    // Two cards discarded, and the spell that made them.
    assert_eq!(names(driver.state(), Zone::Exile).len(), 3);
    Ok(())
}

#[rstest]
fn rest_in_peace_entering_exiles_the_graveyards_and_the_cards_that_follow(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [card("Rest in Peace").unwrap()])
            .battlefield(0, [ready(plains()), ready(plains())])
            .graveyard(0, [bear()])
            .graveyard(1, [bear(), bear()])
    };
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Rest in Peace")?;
    settle(&mut driver, &first)?;
    for seat in 0..seats {
        assert_eq!(
            names(driver.state(), Zone::Graveyard(pid(seat))),
            Vec::<String>::new(),
            "seat {seat}"
        );
    }
    assert_eq!(names(driver.state(), Zone::Exile).len(), 3);
    Ok(())
}

#[test]
fn the_exile_moves_are_ordinary_zone_changes_with_nothing_put_into_a_graveyard()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [burn("Bolt", 4)])
            .battlefield(
                0,
                [ready(mountain()), ready(card("Rest in Peace").unwrap())],
            )
            .battlefield(1, [ready(free_creature("Hill Giant", 3, 3))]),
    )?;
    let begun = driver.engine().events().len();
    let giant = driver.target_named("Hill Giant")?;
    driver.cast_at("Bolt", &[giant])?;
    settle(&mut driver, &first)?;
    let moves: Vec<(String, Zone, Zone, ZoneCause)> = driver.engine().events()[begun..]
        .iter()
        .filter_map(|event| match event {
            Event::ZoneChanged {
                object,
                from,
                to,
                cause,
            } => Some((name_of(driver.state(), *object), *from, *to, *cause)),
            _ => None,
        })
        .collect();
    assert!(
        moves.iter().any(|(name, from, to, _)| name == "Hill Giant"
            && *from == Zone::Battlefield
            && *to == Zone::Exile),
        "{moves:?}"
    );
    assert!(
        moves.iter().any(|(name, from, to, _)| name == "Bolt"
            && *from == Zone::Stack
            && *to == Zone::Exile),
        "{moves:?}"
    );
    Ok(())
}

#[test]
fn with_counters_the_replay_of_the_log_equals_the_live_game() -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [growth_spurt().unwrap()])
            .battlefield(
                0,
                permanents(&["Hardened Scales"])
                    .into_iter()
                    .chain([ready(forest()), ready(with_counters_creature())]),
            )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let target = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Growth Spurt", &[target])?;
    settle(&mut driver, &first)?;
    assert_eq!(counters_on(&driver, "Grizzly Bears", &PLUS), 2);
    replays(&driver, &setup);
    Ok(())
}

fn with_counters_creature() -> std::sync::Arc<mtg_cards::CardDefinition> {
    free_creature("Grizzly Bears", 2, 2)
}

#[test]
fn a_rebuilt_engine_asks_the_same_question_of_a_held_resolution() -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .hand(0, [raise_soldiers().unwrap()])
            .battlefield(
                0,
                permanents(&["Anointed Procession", "Parallel Lives"])
                    .into_iter()
                    .chain([ready(plains())]),
            )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Raise Soldiers")?;
    for _ in 0..10 {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseReplacement(_)) {
            break;
        }
        driver.pass()?;
    }
    let asked = driver.decision()?.clone();
    assert!(matches!(asked.kind, DecisionKind::ChooseReplacement(_)));
    let rebuilt = mtg_engine::Engine::resume(setup, driver.engine().events().to_vec()).unwrap();
    assert_eq!(rebuilt.decision(), Some(&asked));
    Ok(())
}

#[allow(dead_code)]
fn keep_unused_helpers_used() {
    let _ = (with_counters, Step::Main1);
}
