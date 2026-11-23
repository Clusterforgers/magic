//! Goal: at 2 to 4 seats, the r119-01 facts read the game as the cards say ("all cards ... share a card type", "if you
//! put two or more +1/+1 counters on creatures this way", "if you had N or more cards in your starting deck"), Itazura's
//! opponent with the highest secret number takes that much damage and may cast one of the exiled cards for free while
//! its controller keeps one, Kuroki casts a spell from the hand of an opponent who drew four, else grows, and the
//! creature Lwaxana Troi chose triggers twice while she stays tapped; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, NumberChoice, Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers until the stack and the pending triggers are empty: `pick` answers the decisions it knows, the rest are
/// answered passively.
fn r119_resolve(
    driver: &mut Driver,
    pick: impl Fn(&Decision) -> Option<Answer>,
) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let busy =
            !driver.state().stack().is_empty() || !driver.state().pending_triggers().is_empty();
        let decision = driver.decision()?.clone();
        if !matches!(decision.kind, DecisionKind::Priority(_))
            && let Some(answer) = pick(&decision)
        {
            driver.answer(answer)?;
            continue;
        }
        match &decision.kind {
            DecisionKind::Priority(_) if busy => {
                driver.pass()?;
            }
            DecisionKind::Priority(_) => return Ok(()),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the resolution".into()))
}

/// The log replays to the live state.
fn r119_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// A free sorcery named "Test" with the one ability.
fn r119_sorcery(rules: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(rules).unwrap();
    Arc::new(
        CardDefinition::new("Test")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability),
    )
}

/// A free card of the one type.
fn r119_typed(name: &str, kind: CardType) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_mana_cost(cost(0, &[])),
    )
}

/// Casts "Test" from seat 0's hand in its main phase with the scenario's other seats, and resolves it; the life seat 0
/// gained and the cards it drew.
fn r119_cast_test(scenario: Scenario) -> Result<(i32, usize), ScriptError> {
    let mut driver = Driver::from_scenario(scenario.at(2, 0, Step::Main1))?;
    let life = driver.state().player(pid(0)).unwrap().life();
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.cast("Test")?;
    r119_resolve(&mut driver, |_| None)?;
    r119_replays(&driver);
    let gained = driver.state().player(pid(0)).unwrap().life() - life;
    // The cast card left the hand.
    let drawn = driver.state().zone(Zone::Hand(pid(0))).len() + 1 - hand;
    Ok((gained, drawn))
}

/// The table: each seat's library of forests.
fn r119_table(seats: usize, forests: usize) -> Scenario {
    (0..seats).fold(Scenario::new(seats), |scenario, seat| {
        scenario.library(seat, (0..forests).map(|_| forest()))
    })
}

#[rstest]
#[case::two_creatures(&[CardType::Creature, CardType::Creature], true)]
#[case::one_card(&[CardType::Instant], true)]
#[case::an_artifact_and_an_artifact_creature(&[CardType::Artifact, CardType::Creature], false)]
#[case::a_creature_and_an_instant(&[CardType::Creature, CardType::Instant], false)]
#[case::none(&[], false)]
fn r119_cards_share_a_card_type_when_one_type_is_on_each(
    #[values(2, 3, 4)] seats: usize,
    #[case] kinds: &[CardType],
    #[case] share: bool,
) -> Result<(), ScriptError> {
    let test = r119_sorcery(
        r#"{"Spell":{"effects":[{"If":{"condition":{"R119ShareCardType":{"Each":[{"In":"Graveyard"},{"OwnedBy":"You"}]}},"then":[{"GainLife":{"amount":10}}]}}]}}"#,
    );
    let graveyard: Vec<_> = kinds
        .iter()
        .enumerate()
        .map(|(at, kind)| r119_typed(&format!("Card {at}"), *kind))
        .collect();
    let scenario = r119_table(seats, 10)
        .hand(0, [test])
        .graveyard(0, graveyard);
    let (gained, _) = r119_cast_test(scenario)?;
    assert_eq!(gained, if share { 10 } else { 0 });
    Ok(())
}

#[rstest]
fn r119_counters_put_this_way_are_counted(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] creatures: usize,
) -> Result<(), ScriptError> {
    let test = r119_sorcery(
        r#"{"Spell":{"effects":[{"PutCounters":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"+1/+1"}},{"If":{"condition":{"Compare":{"left":{"R119PutThisWay":"+1/+1"},"op":"Ge","right":2}},"then":[{"Draw":{}}]}}]}}"#,
    );
    let bears = (0..creatures).map(|at| ready(free_creature(&format!("Bear {at}"), 2, 2)));
    // An opponent's creature gets no counter and counts for nothing.
    let scenario = r119_table(seats, 10)
        .hand(0, [test])
        .battlefield(0, bears)
        .battlefield(1, [ready(free_creature("Other Bear", 2, 2))]);
    let (_, drawn) = r119_cast_test(scenario)?;
    assert_eq!(drawn, usize::from(creatures >= 2));
    Ok(())
}

#[rstest]
fn r119_the_starting_deck_counts_every_card_of_the_deck(
    #[values(2, 3, 4)] seats: usize,
    #[values(18, 19, 20)] forests: usize,
) -> Result<(), ScriptError> {
    let test = r119_sorcery(
        r#"{"Spell":{"effects":[{"If":{"condition":{"R119StartingDeck":{"at_least":20}},"then":[{"GainLife":{"amount":1}}]}}]}}"#,
    );
    // The deck is the library and the card in hand.
    let scenario = r119_table(seats, forests).hand(0, [test]);
    let (gained, _) = r119_cast_test(scenario)?;
    assert_eq!(gained, i32::from(forests + 1 >= 20));
    Ok(())
}

#[rstest]
fn r119_itazura_gives_the_highest_bidder_damage_and_a_free_cast(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let exiled = ["Alpha", "Beta", "Gamma"];
    let library: Vec<_> = exiled
        .iter()
        .map(|name| free_creature(name, 1, 1))
        .chain((0..10).map(|_| forest()))
        .collect();
    let scenario = r119_table(seats, 10)
        .library(0, library)
        .at(1, seats - 1, Step::Main1)
        .battlefield(0, [ready(card("Itazura, Lingering Wick").unwrap())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(2, Step::Upkeep)?;
    let last = pid(seats - 1);
    let life = driver.state().player(last).unwrap().life();
    // Each opponent bids its seat number; the last seat bids most, casts the first card it may and Itazura's
    // controller keeps the first card left.
    r119_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::ChooseNumber(_) => Some(Answer::ChooseNumber(NumberChoice::Number(
            u32::try_from(decision.player.index()).unwrap(),
        ))),
        DecisionKind::ChooseCards(options) => Some(Answer::ChooseCards(CardsChoice(
            options.from.iter().copied().take(1).collect(),
        ))),
        DecisionKind::YesNo(_) => Some(Answer::YesNo(YesNoChoice(true))),
        _ => None,
    })?;
    r119_replays(&driver);
    let state = driver.state();
    let bid = i32::try_from(seats - 1).unwrap();
    assert_eq!(state.player(last).unwrap().life(), life - bid);
    let named = |id: &mtg_core::ids::ObjectId| {
        state
            .object(*id)
            .is_some_and(|o| exiled.contains(&o.card().name.as_str()))
    };
    let cast: Vec<_> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| named(id))
        .collect();
    assert_eq!(cast.len(), 1, "the highest bidder cast one card");
    assert_eq!(state.object(*cast[0]).unwrap().controller(), last);
    let kept = state
        .zone(Zone::Hand(pid(0)))
        .iter()
        .filter(|id| named(id))
        .count();
    assert_eq!(kept, 1, "Itazura's controller keeps one card");
    assert_eq!(state.zone(Zone::Exile).len(), 1);
    Ok(())
}

#[rstest]
fn r119_kuroki_casts_from_the_hand_of_an_opponent_who_drew(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] draws: bool,
) -> Result<(), ScriptError> {
    let theirs: Vec<_> = (0..6).map(|_| free_creature("Loot", 3, 3)).collect();
    let scenario = r119_table(seats, 10)
        .library(1, theirs)
        .at(2, 0, Step::Main2)
        .battlefield(0, [ready(card("Kuroki, Thief of Talents").unwrap())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::End)?;
    r119_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::ChooseTargets(_) => Some(Answer::ChooseTargets(TargetChoice::Targets(vec![
            Target::Player(pid(1)),
        ]))),
        DecisionKind::YesNo(_) => Some(Answer::YesNo(YesNoChoice(draws))),
        DecisionKind::ChooseCards(options) => Some(Answer::ChooseCards(CardsChoice(
            options.from.iter().copied().take(1).collect(),
        ))),
        _ => None,
    })?;
    r119_replays(&driver);
    let state = driver.state();
    let kuroki = driver.id_of("Kuroki, Thief of Talents")?;
    let counters = state
        .object(kuroki)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne);
    let stolen = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            state
                .object(**id)
                .is_some_and(|o| o.owner() == pid(1) && o.controller() == pid(0))
        })
        .count();
    if draws {
        assert_eq!(state.zone(Zone::Hand(pid(1))).len(), 3);
        assert_eq!((stolen, counters), (1, 0));
    } else {
        assert_eq!((stolen, counters), (0, 2));
    }
    Ok(())
}

/// A free sorcery of this name with the one ability.
fn r119_named_sorcery(name: &str, rules: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(rules).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability),
    )
}

#[rstest]
fn r119_lwaxana_makes_the_chosen_creature_trigger_twice(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] activated: bool,
) -> Result<(), ScriptError> {
    let echo: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"SpellCast":{"by":"You"}},"effects":[{"GainLife":{"amount":1}}]}}"#,
    )
    .unwrap();
    let echo = Arc::new(
        CardDefinition::new("Echo")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(cost(0, &[]))
            .with_rules(echo),
    );
    let scenario = r119_table(seats, 10)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Lwaxana Troi, Charming Guest").unwrap()),
                ready(echo),
                ready(forest()),
                ready(forest()),
            ],
        )
        .hand(
            0,
            [r119_named_sorcery("Nothing", r#"{"Spell":{"effects":[]}}"#)],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    if activated {
        let echo = driver.id_of("Echo")?;
        driver.begin_ability("Lwaxana Troi, Charming Guest", 1)?;
        r119_resolve(&mut driver, |decision| match &decision.kind {
            DecisionKind::ChooseTargets(_) => {
                Some(Answer::ChooseTargets(TargetChoice::Targets(vec![
                    Target::Object(echo),
                ])))
            }
            DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
            _ => None,
        })?;
    }
    let life = driver.state().player(pid(0)).unwrap().life();
    driver.cast("Nothing")?;
    r119_resolve(&mut driver, |_| None)?;
    r119_replays(&driver);
    let gained = driver.state().player(pid(0)).unwrap().life() - life;
    assert_eq!(gained, if activated { 2 } else { 1 });
    Ok(())
}
