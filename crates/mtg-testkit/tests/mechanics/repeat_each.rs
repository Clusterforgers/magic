//! Goal: a repeat plays whole through Incite Rebellion, Treacherous Terrain, Stormbreath Dragon, Wound Reflection, Archfiend of Despair and Duskmantle Seer at two, three and four seats: each pass is for one player (none, one and many of them hold what the pass counts), a pass reads the numbers of its own player and no other, what the body binds is bound anew in each pass, and the game replays from its log to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{burn, creature, free_creature, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Passes priority and answers every question passively until nothing is on the stack and priority is asked again.
fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let idle = matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty();
        if idle {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// The creatures of the seat on the battlefield.
fn creatures(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.controller() == pid(seat) && object.card().name.starts_with("Bear"))
        .count()
}

/// The game replays from its log to the state it ended in.
fn replays(driver: &Driver) {
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// `count` 2/2 Bears for the seat, with names that start with "Bear".
fn bears(seat: usize, count: usize) -> Vec<mtg_engine::PresetPermanent> {
    (0..count)
        .map(|n| ready(free_creature(&format!("Bear {seat}-{n}"), 2, 2)))
        .collect()
}

/// Incite Rebellion with the creatures each seat starts with: each player takes damage equal to the number of creatures
/// they control and so does each of those creatures, so a lone creature lives and two or more die.
#[rstest]
#[case::nobody_has_one(&[0, 0])]
#[case::only_the_caster(&[1, 0])]
#[case::only_the_other(&[0, 3])]
#[case::one_each(&[1, 1])]
#[case::many_each(&[3, 2, 4])]
#[case::a_lone_creature_in_a_crowd(&[2, 1, 0, 5])]
fn incite_rebellion_hurts_each_player_and_their_creatures_by_their_own_count(
    #[case] counts: &[usize],
) -> Result<(), ScriptError> {
    let seats = counts.len();
    let mut scenario = Scenario::new(seats)
        .hand(0, [card("Incite Rebellion").unwrap()])
        .battlefield(
            0,
            (0..6).map(|_| ready(mountain())).chain(bears(0, counts[0])),
        );
    for (seat, count) in counts.iter().enumerate().skip(1) {
        scenario = scenario.battlefield(seat, bears(seat, *count));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let before: Vec<i32> = (0..seats).map(|seat| life(&driver, seat)).collect();
    driver.cast("Incite Rebellion")?;
    resolve(&mut driver)?;
    for (seat, count) in counts.iter().enumerate() {
        let hurt = i32::try_from(*count).unwrap();
        assert_eq!(
            life(&driver, seat),
            before[seat] - hurt,
            "life of seat {seat}"
        );
        let survivors = usize::from(*count == 1);
        assert_eq!(
            creatures(&driver, seat),
            survivors,
            "creatures of seat {seat}"
        );
    }
    replays(&driver);
    Ok(())
}

/// Treacherous Terrain hurts each opponent by the lands that opponent controls and the caster not at all.
#[rstest]
#[case::no_lands(&[0, 0])]
#[case::one_opponent_with_lands(&[0, 3])]
#[case::an_opponent_with_none_among_others(&[0, 0, 2, 5])]
#[case::lands_on_every_seat(&[4, 1, 2])]
fn treacherous_terrain_hurts_each_opponent_by_their_own_lands(
    #[case] lands: &[usize],
) -> Result<(), ScriptError> {
    let seats = lands.len();
    let mut scenario = Scenario::new(seats)
        .hand(0, [card("Treacherous Terrain").unwrap()])
        .battlefield(
            0,
            [ready(mountain()), ready(mtg_testkit::cards::forest())]
                .into_iter()
                .chain((0..6).map(|_| ready(mountain()))),
        );
    for (seat, count) in lands.iter().enumerate().skip(1) {
        scenario = scenario.battlefield(seat, (0..*count).map(|_| ready(swamp())));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let before: Vec<i32> = (0..seats).map(|seat| life(&driver, seat)).collect();
    driver.cast("Treacherous Terrain")?;
    resolve(&mut driver)?;
    assert_eq!(life(&driver, 0), before[0], "the caster is not hurt");
    for (seat, count) in lands.iter().enumerate().skip(1) {
        let hurt = i32::try_from(*count).unwrap();
        assert_eq!(
            life(&driver, seat),
            before[seat] - hurt,
            "life of seat {seat}"
        );
    }
    replays(&driver);
    Ok(())
}

/// Stormbreath Dragon becomes monstrous and hurts each opponent by the cards in that opponent's hand.
#[rstest]
#[case::empty_hands(&[0, 0])]
#[case::one_opponent_holds_cards(&[3, 2])]
#[case::many_hands(&[1, 0, 4, 7])]
fn stormbreath_dragon_hurts_each_opponent_by_the_cards_in_their_hand(
    #[case] hands: &[usize],
) -> Result<(), ScriptError> {
    let seats = hands.len();
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        std::iter::once(ready(card("Stormbreath Dragon").unwrap()))
            .chain((0..7).map(|_| ready(mountain()))),
    );
    for (seat, count) in hands.iter().enumerate().skip(1) {
        scenario = scenario.hand(
            seat,
            (0..*count).map(|n| free_creature(&format!("Card {seat}-{n}"), 1, 1)),
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let before: Vec<i32> = (0..seats).map(|seat| life(&driver, seat)).collect();
    driver.begin_activation("Stormbreath Dragon")?;
    driver.pay_tapping(&["Mountain"; 7], 0)?;
    resolve(&mut driver)?;
    assert_eq!(life(&driver, 0), before[0]);
    for (seat, count) in hands.iter().enumerate().skip(1) {
        let hurt = i32::try_from(*count).unwrap();
        assert_eq!(
            life(&driver, seat),
            before[seat] - hurt,
            "life of seat {seat}"
        );
    }
    replays(&driver);
    Ok(())
}

/// Wound Reflection and Archfiend of Despair double the life each opponent lost this turn at the end step: seat 1 is burnt
/// for `burnt` and every other opponent is not.
#[rstest]
fn at_the_end_step_each_opponent_loses_the_life_they_lost_this_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values("Wound Reflection", "Archfiend of Despair")] source: &str,
    #[values(0, 1, 3)] burnt: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Main2)
            .hand(0, [burn("Spark", burnt.max(1))])
            .battlefield(0, [ready(card(source).unwrap()), ready(mountain())]),
    )?;
    let before: Vec<i32> = (0..seats).map(|seat| life(&driver, seat)).collect();
    if burnt > 0 {
        driver.begin_cast("Spark")?;
        let target = driver.target_player(1);
        driver.choose_targets(&[target])?;
        driver.pay_tapping(&["Mountain"], 0)?;
        resolve(&mut driver)?;
    }
    assert_eq!(
        life(&driver, 1),
        before[1] - if burnt > 0 { burnt } else { 0 }
    );
    driver.pass_until(Step::End)?;
    resolve(&mut driver)?;
    assert_eq!(life(&driver, 0), before[0], "the controller loses nothing");
    assert_eq!(
        life(&driver, 1),
        before[1] - 2 * burnt,
        "seat 1 lost it twice over"
    );
    for (seat, start) in before.iter().enumerate().skip(2) {
        assert_eq!(life(&driver, seat), *start, "seat {seat} lost nothing");
    }
    replays(&driver);
    Ok(())
}

/// Duskmantle Seer: at its controller's upkeep each player reveals the top card of their library, loses life equal to its
/// mana value and puts it into their hand; a name the body binds (the card revealed) is bound anew in each pass, so each
/// player loses the value of their own card.
#[rstest]
fn each_player_reveals_loses_life_for_and_takes_their_own_top_card(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let value = |seat: usize| u32::try_from(seat).unwrap() + 1;
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::End)
        .battlefield(0, [ready(card("Duskmantle Seer").unwrap())]);
    for seat in 0..seats {
        // The first card is drawn in the seat's own turn before the upkeep of the Seer comes round; the second is the one
        // revealed.
        let filler = free_creature(&format!("Filler {seat}"), 1, 1);
        let top = creature(&format!("Top {seat}"), value(seat), &[], 1, 1);
        scenario = scenario.library(
            seat,
            if seat == 0 {
                vec![top]
            } else {
                vec![filler, top]
            },
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let before: Vec<i32> = (0..seats).map(|seat| life(&driver, seat)).collect();
    driver.idle_until(2 + u32::try_from(seats).unwrap(), Step::Upkeep)?;
    resolve(&mut driver)?;
    for (seat, start) in before.iter().enumerate() {
        assert_eq!(
            life(&driver, seat),
            start - i32::try_from(value(seat)).unwrap(),
            "life of seat {seat}"
        );
        let in_hand = driver
            .state()
            .zone(Zone::Hand(pid(seat)))
            .iter()
            .filter_map(|id| driver.state().object(*id))
            .any(|object| object.card().name == format!("Top {seat}"));
        assert!(in_hand, "seat {seat} holds the card it revealed");
    }
    let reveals = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::Revealed(_)))
        .count();
    assert_eq!(reveals, seats, "one reveal for each pass");
    replays(&driver);
    Ok(())
}

/// The cards the repeat, the dice and the coins unlock are all Verified: the engine plays every primitive they use.
#[rstest]
#[case("Incite Rebellion")]
#[case("Treacherous Terrain")]
#[case("Stormbreath Dragon")]
#[case("Wound Reflection")]
#[case("Archfiend of Despair")]
#[case("Duskmantle Seer")]
#[case("Mutalith Vortex Beast")]
#[case("Boompile")]
#[case("Goblin Archaeologist")]
#[case("Earth-Cult Elemental")]
fn the_cards_of_the_repeat_dice_and_coins_are_verified(#[case] name: &str) {
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
