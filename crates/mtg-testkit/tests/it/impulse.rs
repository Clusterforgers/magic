//! Goal: the impulse draw plays whole: the cards a spell or ability exiles from the top of a library are face up in exile and their controller may cast them (paying their costs, cost modifiers included, at the timing of the card) or play a land among them (a land play of their own turn, in a main step, with an empty stack) until the duration ends (this turn, the end of their next turn) or the card leaves exile; a library that runs short or empty exiles what it has and gives no permission for the rest, the two-card form offers both cards, a land cannot be cast, and the cards of the corpus (Abbot of Keral Keep, Professional Face-Breaker, Reckless Impulse) do it at two, three and four seats and replay to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::modify::Duration;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::house::HouseRules;
use mtg_engine::decision::{DecisionKind, PriorityOptions};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{creature, forest, instant, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, impulse_spell, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::{name_of, names, pid};

const SEAT: usize = 0;

/// A sorcery-speed card the library holds for the tests, for `{1}{R}`.
fn bear() -> Arc<CardDefinition> {
    creature("Bear", 1, &[Color::Red], 2, 2)
}

/// An instant for `{R}`, which can be cast at any time.
fn trick() -> Arc<CardDefinition> {
    instant("Trick", 0, &[Color::Red])
}

/// `top` over cards nobody can cast, enough of them that the player draws for turns: nothing but the top card shares a name
/// with what a test looks for.
fn library(top: Arc<CardDefinition>) -> Vec<Arc<CardDefinition>> {
    let filler = creature("Filler", 9, &[], 1, 1);
    std::iter::once(top)
        .chain(std::iter::repeat_n(filler, 8))
        .collect()
}

/// The spell of seat 0: exiles one card and lets its controller play it for `duration`.
fn impulse_for(duration: Duration) -> Arc<CardDefinition> {
    impulse_spell("Impulse", CardType::Sorcery, 0, &[Color::Red], 1, duration)
}

/// Seat 0 holds `hand` over four Mountains with `library` on top, top first; the other seats have libraries of their own.
fn table(
    seats: usize,
    hand: Vec<Arc<CardDefinition>>,
    library: Vec<Arc<CardDefinition>>,
) -> Scenario {
    Scenario::new(seats)
        .hand(SEAT, hand)
        .battlefield(SEAT, lands(4, mountain))
        .library(SEAT, library)
}

fn priority(driver: &Driver) -> &PriorityOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options,
        other => panic!("priority is wanted: {}", other.name()),
    }
}

/// The cards seat 0 may cast now.
fn castable(driver: &Driver) -> Vec<String> {
    priority(driver)
        .castable
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect()
}

/// The lands seat 0 may play now.
fn playable(driver: &Driver) -> Vec<String> {
    priority(driver)
        .playable_lands
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect()
}

/// Passes priority while the stack holds something.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Casts the spell and lets it resolve.
fn cast_and_settle(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.cast(name)?;
    settle(driver)
}

fn exile(driver: &Driver) -> Vec<String> {
    let mut found = names(driver.state(), Zone::Exile);
    found.sort();
    found
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Seat 0 may have priority in the turn `turn` at its first main step: its own turn, or another seat's after that seat
/// passes.
fn at_main_of(driver: &mut Driver, turn: u32) -> Result<(), ScriptError> {
    driver.idle_until(turn, Step::Main1)?;
    if driver.decision()?.player != pid(SEAT) {
        driver.pass()?;
    }
    assert_eq!(driver.decision()?.player, pid(SEAT));
    Ok(())
}

/// When the exiled card is looked at, after the spell resolved in the first main step of turn 1.
#[derive(Debug, Clone, Copy)]
enum When {
    /// Still in that step, with an empty stack.
    Now,
    /// With a spell of seat 0 on the stack.
    StackBusy,
    /// In the first main step of turn 2, which is seat 1's.
    TheOpponentsTurn,
    /// In the first main step of turn 3, which is seat 0's again.
    MyNextTurn,
    /// In the first main step of turn 5, seat 0's turn after that.
    TheTurnAfter,
}

fn arrive(driver: &mut Driver, when: When) -> Result<(), ScriptError> {
    match when {
        When::Now => {}
        When::StackBusy => {
            driver.cast("Trick Card")?;
            assert_eq!(driver.state().stack().len(), 1);
        }
        When::TheOpponentsTurn => at_main_of(driver, 2)?,
        When::MyNextTurn => at_main_of(driver, 3)?,
        When::TheTurnAfter => at_main_of(driver, 5)?,
    }
    Ok(())
}

/// A card of the hand that can be cast with an empty effect, to put a spell on the stack.
fn filler() -> Arc<CardDefinition> {
    instant("Trick Card", 0, &[Color::Red])
}

#[rstest]
#[case::a_creature_now(Duration::UntilEndOfTurn, bear(), "Bear", When::Now, true)]
#[case::a_creature_with_a_spell_on_the_stack(
    Duration::UntilEndOfTurn,
    bear(),
    "Bear",
    When::StackBusy,
    false
)]
#[case::an_instant_with_a_spell_on_the_stack(
    Duration::UntilEndOfTurn,
    trick(),
    "Trick",
    When::StackBusy,
    true
)]
#[case::a_creature_in_the_opponents_turn_after_this_turn(
    Duration::UntilEndOfTurn,
    bear(),
    "Bear",
    When::TheOpponentsTurn,
    false
)]
#[case::an_instant_in_the_opponents_turn_after_this_turn(
    Duration::UntilEndOfTurn,
    trick(),
    "Trick",
    When::TheOpponentsTurn,
    false
)]
#[case::a_creature_in_my_next_turn_after_this_turn(
    Duration::UntilEndOfTurn,
    bear(),
    "Bear",
    When::MyNextTurn,
    false
)]
#[case::a_creature_in_the_opponents_turn_of_next_turn(
    Duration::UntilEndOfYourNextTurn,
    bear(),
    "Bear",
    When::TheOpponentsTurn,
    false
)]
#[case::an_instant_in_the_opponents_turn_of_next_turn(
    Duration::UntilEndOfYourNextTurn,
    trick(),
    "Trick",
    When::TheOpponentsTurn,
    true
)]
#[case::a_creature_in_my_next_turn(
    Duration::UntilEndOfYourNextTurn,
    bear(),
    "Bear",
    When::MyNextTurn,
    true
)]
#[case::an_instant_in_my_next_turn(
    Duration::UntilEndOfYourNextTurn,
    trick(),
    "Trick",
    When::MyNextTurn,
    true
)]
#[case::a_creature_the_turn_after_next(
    Duration::UntilEndOfYourNextTurn,
    bear(),
    "Bear",
    When::TheTurnAfter,
    false
)]
#[case::an_instant_the_turn_after_next(
    Duration::UntilEndOfYourNextTurn,
    trick(),
    "Trick",
    When::TheTurnAfter,
    false
)]
fn the_exiled_card_is_castable_at_its_timing_until_the_duration_ends(
    #[case] duration: Duration,
    #[case] found: Arc<CardDefinition>,
    #[case] name: &str,
    #[case] when: When,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let scenario = table(2, vec![impulse_for(duration), filler()], library(found));
    let mut driver = Driver::from_scenario(scenario)?;
    cast_and_settle(&mut driver, "Impulse")?;
    assert_eq!(exile(&driver), [name]);
    arrive(&mut driver, when)?;
    assert_eq!(
        castable(&driver).contains(&name.to_owned()),
        offered,
        "{when:?}"
    );
    if offered {
        // The cost is paid as any other cost is, and the card leaves exile for the stack.
        driver.cast(name)?;
        assert_eq!(exile(&driver), Vec::<String>::new());
        assert!(names(driver.state(), Zone::Stack).contains(&name.to_owned()));
    }
    assert_replays(&driver);
    Ok(())
}

/// Seat 0 holds Reckless Impulse (two cards, until the end of the next turn) over four Mountains and `library`, top first.
fn reckless(library: Vec<Arc<CardDefinition>>) -> Scenario {
    table(2, vec![card("Reckless Impulse").unwrap()], library)
}

fn filler_cards(count: usize) -> Vec<Arc<CardDefinition>> {
    vec![creature("Filler", 9, &[], 1, 1); count]
}

#[rstest]
#[case::a_nonland_and_a_land(vec![bear(), forest()], &["Bear", "Forest"], &["Bear"], &["Forest"])]
#[case::two_lands(vec![forest(), forest()], &["Forest", "Forest"], &[], &["Forest", "Forest"])]
#[case::two_spells(vec![bear(), trick()], &["Bear", "Trick"], &["Bear", "Trick"], &[])]
#[case::a_library_of_one(vec![bear()], &["Bear"], &["Bear"], &[])]
#[case::an_empty_library(Vec::new(), &[], &[], &[])]
fn the_two_card_form_exiles_what_the_library_has_and_offers_each_card_as_it_may_be_played(
    #[case] top: Vec<Arc<CardDefinition>>,
    #[case] exiled: &[&str],
    #[case] cast: &[&str],
    #[case] land: &[&str],
) -> Result<(), ScriptError> {
    let library: Vec<_> = top.into_iter().chain(filler_cards(6)).collect();
    let library = if exiled.len() < 2 {
        library.into_iter().take(exiled.len()).collect()
    } else {
        library
    };
    let mut driver = Driver::from_scenario(reckless(library))?;
    cast_and_settle(&mut driver, "Reckless Impulse")?;
    assert_eq!(exile(&driver), {
        let mut sorted = exiled.to_vec();
        sorted.sort_unstable();
        sorted
    });
    let mut offered = castable(&driver);
    offered.sort();
    assert_eq!(offered, cast);
    assert_eq!(playable(&driver), land);
    let rules = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::RuleCreated { .. }))
        .count();
    assert_eq!(
        rules,
        usize::from(!exiled.is_empty()),
        "nothing exiled, nothing allowed"
    );
    assert_replays(&driver);
    Ok(())
}

#[test]
fn casting_one_of_two_exiled_cards_leaves_the_other_playable() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(reckless(
        [bear(), forest()]
            .into_iter()
            .chain(filler_cards(6))
            .collect(),
    ))?;
    cast_and_settle(&mut driver, "Reckless Impulse")?;
    cast_and_settle(&mut driver, "Bear")?;
    assert_eq!(exile(&driver), ["Forest"]);
    assert_eq!(playable(&driver), ["Forest"]);
    driver.play_land("Forest")?;
    assert_eq!(exile(&driver), Vec::<String>::new());
    assert_eq!(castable(&driver), Vec::<String>::new());
    Ok(())
}

#[rstest]
#[case::no_cost_modifier(false, bear(), "Bear", "{1}{R}")]
#[case::an_instant_without_a_modifier(false, trick(), "Trick", "{R}")]
#[case::a_tax_on_a_noncreature_spell(true, trick(), "Trick", "{1}{R}")]
#[case::a_tax_that_leaves_a_creature_alone(true, bear(), "Bear", "{1}{R}")]
fn the_exiled_card_costs_what_it_costs_with_the_modifiers_of_the_table(
    #[case] taxed: bool,
    #[case] found: Arc<CardDefinition>,
    #[case] name: &str,
    #[case] total: &str,
) -> Result<(), ScriptError> {
    let mut scenario = table(
        2,
        vec![impulse_for(Duration::UntilEndOfTurn)],
        library(found),
    );
    if taxed {
        scenario = scenario.battlefield(1, [ready(card("Thalia, Guardian of Thraben").unwrap())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    cast_and_settle(&mut driver, "Impulse")?;
    driver.begin_cast(name)?;
    driver.expect_decision(0, "pay-mana")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), total);
    Ok(())
}

#[rstest]
fn abbot_of_keral_keep_lets_its_controller_alone_cast_the_card_it_exiles(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        vec![card("Abbot of Keral Keep").unwrap()],
        library(bear()),
    ))?;
    cast_and_settle(&mut driver, "Abbot of Keral Keep")?;
    assert_eq!(exile(&driver), ["Bear"]);
    assert!(castable(&driver).contains(&"Bear".to_owned()));
    cast_and_settle(&mut driver, "Bear")?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Bear".to_owned()));
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn the_seats_after_the_controller_are_not_offered_the_exiled_card(
    #[values(2, 3, 4)] seats: usize,
    #[values(Duration::UntilEndOfTurn, Duration::UntilEndOfYourNextTurn)] duration: Duration,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        table(seats, vec![impulse_for(duration)], library(trick()))
            .battlefield(1, [ready(mountain()), ready(mountain())]),
    )?;
    cast_and_settle(&mut driver, "Impulse")?;
    assert!(castable(&driver).contains(&"Trick".to_owned()));
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(1));
    assert!(
        !castable(&driver).contains(&"Trick".to_owned()),
        "the permission is its controller's, though seat 1 has the mana"
    );
    Ok(())
}

#[test]
fn professional_face_breaker_plays_the_card_it_exiles_for_a_treasure_it_made_in_combat()
-> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .battlefield(
            0,
            lands(3, mountain)
                .into_iter()
                .chain([ready(card("Professional Face-Breaker").unwrap())]),
        )
        .library(0, library(bear()));
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(
        driver.offered_abilities("Professional Face-Breaker")?,
        Vec::<usize>::new(),
        "no Treasure to sacrifice yet"
    );
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Professional Face-Breaker", 1)])?;
    for _ in 0..80 {
        if driver.state().turn().step() == Step::Main2
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(driver.state().turn().step(), Step::Main2);
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Treasure".to_owned()));
    driver.begin_activation("Professional Face-Breaker")?;
    settle(&mut driver)?;
    assert!(!names(driver.state(), Zone::Battlefield).contains(&"Treasure".to_owned()));
    assert_eq!(exile(&driver), ["Bear"]);
    cast_and_settle(&mut driver, "Bear")?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Bear".to_owned()));
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::a_land_now(1, None, When::Now, true)]
#[case::the_land_play_used_by_a_land_of_the_hand(1, Some("Mountain"), When::Now, false)]
#[case::a_second_land_play(2, Some("Mountain"), When::Now, true)]
#[case::with_a_spell_on_the_stack(2, None, When::StackBusy, false)]
#[case::in_the_opponents_turn(1, None, When::TheOpponentsTurn, false)]
#[case::in_my_next_turn(1, None, When::MyNextTurn, true)]
#[case::in_the_turn_after(1, None, When::TheTurnAfter, false)]
fn a_land_among_the_exiled_cards_is_played_with_a_land_play_on_my_turn_with_an_empty_stack(
    #[case] land_plays: u32,
    #[case] played_first: Option<&str>,
    #[case] when: When,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let duration = if matches!(when, When::Now | When::StackBusy) {
        Duration::UntilEndOfTurn
    } else {
        Duration::UntilEndOfYourNextTurn
    };
    let house_rules = HouseRules {
        land_plays_per_turn: Some(land_plays),
        ..HouseRules::default()
    };
    let scenario = table(
        2,
        vec![impulse_for(duration), filler(), mountain()],
        library(forest()),
    )
    .house_rules(house_rules);
    let mut driver = Driver::from_scenario(scenario)?;
    if let Some(land) = played_first {
        driver.play_land(land)?;
    }
    cast_and_settle(&mut driver, "Impulse")?;
    assert_eq!(exile(&driver), ["Forest"]);
    arrive(&mut driver, when)?;
    assert_eq!(
        playable(&driver).contains(&"Forest".to_owned()),
        offered,
        "{when:?}"
    );
    assert!(
        !castable(&driver).contains(&"Forest".to_owned()),
        "a land is played, not cast"
    );
    if offered {
        let events = driver.play_land("Forest")?;
        assert!(
            events
                .iter()
                .any(|event| matches!(event, Event::LandPlayed { .. }))
        );
        assert_eq!(exile(&driver), Vec::<String>::new());
        assert!(names(driver.state(), Zone::Battlefield).contains(&"Forest".to_owned()));
        assert!(!playable(&driver).contains(&"Forest".to_owned()));
    }
    assert_replays(&driver);
    Ok(())
}
