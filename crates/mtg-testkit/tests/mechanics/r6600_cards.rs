//! Goal: the r66-00 cards play whole at two to four seats: Conduit of Worlds casts a permanent card from the graveyard
//! only while no spell was cast this turn and then forbids more spells (CR 608.2g, 611); All Will Be One triggers once
//! for each permanent and player its controller puts counters on, for that many counters (CR 603.2c); The Chain Veil
//! lets each planeswalker activate a loyalty ability once more (CR 606.3); Biomancer's Familiar makes the next adapt of
//! a creature ignore its +1/+1 counters, once (CR 701.46a), and makes activated abilities cost {2} less to at least one;
//! Selective Obliteration lets each player choose a color in turn order (CR 101.4) and exiles what is neither colorless
//! nor only its controller's color; Atraxa, Grand Unifier puts at most one revealed card into the hand for each card
//! type, no card counting for two (CR 205.2a); Minthara's ward costs {X} for its controller's experience counters.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::{CardsChoice, ChoiceOption, OptionChoice, Target, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{
    creature, forest, free_creature, island, mountain, plains, swamp, wastes,
};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

/// The life of the seat.
fn r6600_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// The card with this name in the seat's graveyard, as a target.
fn r6600_in_graveyard(driver: &Driver, seat: usize, name: &str) -> Option<Target> {
    let state = driver.state();
    let id = state
        .zone(Zone::Graveyard(pid(seat)))
        .iter()
        .copied()
        .find(|id| name_of(state, *id) == name)?;
    Some(Target::Object(state.object(id).unwrap().reference()))
}

/// Resolves the stack: `cast` answers each yes-or-no question, mana is paid automatically, each target question takes
/// the first of `targets` it allows, and the rest is answered passively until seat 0 has priority on an empty stack.
fn r6600_resolve(driver: &mut Driver, cast: bool, targets: &[Target]) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && decision.player == pid(0) =>
            {
                return Ok(());
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(cast)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseTargets(options) if !targets.is_empty() => {
                let legal: Vec<Target> = targets
                    .iter()
                    .copied()
                    .filter(|t| options.legal.contains(t))
                    .take(1)
                    .collect();
                driver.choose_targets(&legal)?;
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("resolving".into()))
}

/// Whether seat 0 may cast the card with this name now.
fn r6600_castable(driver: &Driver, name: &str) -> bool {
    let Ok(decision) = driver.decision() else {
        return false;
    };
    let DecisionKind::Priority(options) = &decision.kind else {
        return false;
    };
    options
        .castable
        .iter()
        .any(|id| name_of(driver.state(), *id) == name)
}

#[rstest]
#[case::cast_when_no_spell_was_cast(false, true, true)]
#[case::declined(false, false, false)]
#[case::not_after_a_spell_this_turn(true, true, false)]
fn r6600_conduit_casts_a_permanent_card_then_forbids_spells(
    #[values(2, 3, 4)] seats: usize,
    #[case] bolted_first: bool,
    #[case] accept: bool,
    #[case] cast: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Conduit of Worlds").unwrap()),
                ready(forest()),
                ready(forest()),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .hand(
            0,
            [
                card("Lightning Bolt").unwrap(),
                card("Lightning Bolt").unwrap(),
            ],
        )
        .graveyard(0, [card("Grizzly Bears").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    let foe = driver.target_player(1);
    if bolted_first {
        driver.cast_at("Lightning Bolt", &[foe])?;
        r6600_resolve(&mut driver, false, &[])?;
    }
    let bears = r6600_in_graveyard(&driver, 0, "Grizzly Bears").unwrap();
    driver.begin_activation("Conduit of Worlds")?;
    r6600_resolve(&mut driver, accept, &[bears])?;
    assert_eq!(driver.id_of("Grizzly Bears").is_ok(), cast);
    assert_eq!(
        r6600_in_graveyard(&driver, 0, "Grizzly Bears").is_some(),
        !cast
    );
    // Once the card was cast, no more spells this turn; otherwise the second Bolt is still castable.
    assert_eq!(r6600_castable(&driver, "Lightning Bolt"), !cast);
    Ok(())
}

/// An artifact whose free ability puts `count` +1/+1 counters on each creature its controller controls and a poison
/// counter on each opponent.
fn r6600_counter_tool() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":["Tap"],"effects":[
            {"PutCounters":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"+1/+1","count":2}},
            {"PutCounters":{"on":"Opponents","counter":"poison"}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Counter Tool")
            .with_type(CardType::Artifact)
            .with_rules(rule),
    )
}

#[rstest]
#[case::one_creature(1)]
#[case::two_creatures(2)]
fn r6600_all_will_be_one_triggers_for_each_recipient(
    #[values(2, 3, 4)] seats: usize,
    #[case] creatures: usize,
) -> Result<(), ScriptError> {
    let mut board = vec![
        ready(card("All Will Be One").unwrap()),
        ready(r6600_counter_tool()),
    ];
    board.extend((0..creatures).map(|_| ready(card("Grizzly Bears").unwrap())));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board);
    let mut driver = Driver::from_scenario(scenario)?;
    let foe = driver.target_player(1);
    let before = r6600_life(&driver, 1);
    driver.begin_activation("Counter Tool")?;
    r6600_resolve(&mut driver, false, &[foe])?;
    // Two counters on each creature and one poison counter on each opponent, each a trigger of its own.
    let opponents = i32::try_from(seats - 1).unwrap();
    let dealt = 2 * i32::try_from(creatures).unwrap() + opponents;
    assert_eq!(r6600_life(&driver, 1), before - dealt);
    Ok(())
}

#[rstest]
fn r6600_all_will_be_one_ignores_an_opponents_counters(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(0, [ready(card("All Will Be One").unwrap())])
        .battlefield(
            1,
            [
                ready(r6600_counter_tool()),
                ready(card("Grizzly Bears").unwrap()),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    let lives: Vec<i32> = (0..seats).map(|seat| r6600_life(&driver, seat)).collect();
    driver.begin_activation("Counter Tool")?;
    for _ in 0..60 {
        if driver.state().stack().is_empty()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            && driver.decision()?.player == pid(1)
        {
            break;
        }
        driver.answer_passively()?;
    }
    let after: Vec<i32> = (0..seats).map(|seat| r6600_life(&driver, seat)).collect();
    assert_eq!(after, lives);
    Ok(())
}

/// Activates the planeswalker's first loyalty ability and resolves it.
fn r6600_plus(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.begin_ability("Ajani Goldmane", 0)?;
    r6600_resolve(driver, false, &[])
}

#[rstest]
#[case::no_veil(0, 1)]
#[case::one_veil(1, 2)]
fn r6600_the_chain_veil_allows_one_more_loyalty_activation(
    #[values(2, 3, 4)] seats: usize,
    #[case] veils: usize,
    #[case] activations: usize,
) -> Result<(), ScriptError> {
    let mut board = vec![
        ready(card("The Chain Veil").unwrap()),
        ready(card("Ajani Goldmane").unwrap()),
    ];
    board.extend((0..4).map(|_| ready(plains())));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board);
    let mut driver = Driver::from_scenario(scenario)?;
    let before = r6600_life(&driver, 0);
    r6600_plus(&mut driver)?;
    for _ in 0..veils {
        assert!(driver.offered_abilities("Ajani Goldmane")?.is_empty());
        driver.begin_activation("The Chain Veil")?;
        r6600_resolve(&mut driver, false, &[])?;
        r6600_plus(&mut driver)?;
    }
    assert!(driver.offered_abilities("Ajani Goldmane")?.is_empty());
    let gained = 2 * i32::try_from(activations).unwrap();
    assert_eq!(r6600_life(&driver, 0), before + gained);
    Ok(())
}

#[rstest]
#[case::activated_a_loyalty_ability(true, 0)]
#[case::did_not(false, -2)]
fn r6600_the_chain_veil_drains_without_a_loyalty_activation(
    #[values(2, 3, 4)] seats: usize,
    #[case] activate: bool,
    #[case] change: i32,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("The Chain Veil").unwrap()),
            ready(card("Ajani Goldmane").unwrap()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let mut before = r6600_life(&driver, 0);
    if activate {
        r6600_plus(&mut driver)?;
        before += 2;
    }
    for _ in 0..200 {
        let turn = driver.state().turn();
        if turn.step() == Step::Cleanup || turn.number() > 1 {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(r6600_life(&driver, 0), before + change);
    Ok(())
}

/// A 1/1 creature with "{3}: Adapt 2."
fn r6600_adapter() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":[{"Mana":"{3}"}],"effects":[{"Adapt":{"count":2}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Adapter")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rule),
    )
}

/// The +1/+1 counters on the Adapter.
fn r6600_adapter_counters(driver: &Driver) -> u32 {
    let id = driver.id_of("Adapter").unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

#[rstest]
#[case::without_the_familiar(false, 1)]
#[case::with_the_familiar_once(true, 3)]
fn r6600_biomancers_familiar_lets_the_next_adapt_ignore_counters(
    #[values(2, 3, 4)] seats: usize,
    #[case] familiar: bool,
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Biomancer's Familiar").unwrap()),
            with_counters(ready(r6600_adapter()), CounterKind::PlusOnePlusOne, 1),
            ready(forest()),
            ready(forest()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let adapter = driver.target_named("Adapter")?;
    if familiar {
        driver.begin_ability("Biomancer's Familiar", 1)?;
        r6600_resolve(&mut driver, false, &[adapter])?;
    }
    // Each adapt costs {1}: {3} reduced by {2}. The second adapt finds the rule used up.
    for _ in 0..2 {
        driver.begin_activation("Adapter")?;
        r6600_resolve(&mut driver, false, &[])?;
    }
    assert_eq!(r6600_adapter_counters(&driver), counters);
    Ok(())
}

#[rstest]
fn r6600_biomancers_familiar_leaves_one_mana(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // "{3}: Adapt 2" costs {1} under the Familiar: one Forest pays it, and nothing is left to pay a second time.
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Biomancer's Familiar").unwrap()),
            ready(r6600_adapter()),
            ready(forest()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Adapter")?;
    r6600_resolve(&mut driver, false, &[])?;
    assert_eq!(r6600_adapter_counters(&driver), 2);
    assert!(driver.offered_abilities("Adapter")?.is_empty());
    Ok(())
}

/// Seat `seat`'s white creature, white and blue creature and colorless creature.
fn r6600_trio(seat: usize) -> Vec<mtg_engine::PresetPermanent> {
    vec![
        ready(creature(&format!("White {seat}"), 0, &[Color::White], 1, 1)),
        ready(creature(
            &format!("Azorius {seat}"),
            0,
            &[Color::White, Color::Blue],
            1,
            1,
        )),
        ready(free_creature(&format!("Gray {seat}"), 1, 1)),
    ]
}

#[rstest]
fn r6600_selective_obliteration_keeps_colorless_and_the_controllers_color(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    for seat in 0..seats {
        let mut board = r6600_trio(seat);
        if seat == 0 {
            board.extend((0..5).map(|_| ready(wastes())));
        }
        scenario = scenario.battlefield(seat, board);
    }
    scenario = scenario.hand(0, [card("Selective Obliteration").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Selective Obliteration")?;
    let mut askers = Vec::new();
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseOption(options) => {
                askers.push(decision.player.index());
                // Seat 0 keeps white, the others choose blue.
                let color = if decision.player == pid(0) {
                    Color::White
                } else {
                    Color::Blue
                };
                let at = options
                    .options
                    .iter()
                    .position(|option| *option == ChoiceOption::Color(color))
                    .unwrap();
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && decision.player == pid(0) =>
            {
                break;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(
        askers,
        (0..seats).collect::<Vec<_>>(),
        "each player in turn order"
    );
    for seat in 0..seats {
        assert_eq!(
            driver.id_of(&format!("White {seat}")).is_ok(),
            seat == 0,
            "White {seat}"
        );
        assert!(
            driver.id_of(&format!("Azorius {seat}")).is_err(),
            "Azorius {seat}"
        );
        assert!(driver.id_of(&format!("Gray {seat}")).is_ok(), "Gray {seat}");
    }
    assert!(driver.id_of("Wastes").is_ok());
    Ok(())
}

/// A card of these types, named `name`.
fn r6600_typed(name: &str, types: &[CardType]) -> Arc<CardDefinition> {
    let card = types
        .iter()
        .fold(CardDefinition::new(name), |card, ty| card.with_type(*ty));
    Arc::new(if types.contains(&CardType::Creature) {
        card.with_power_toughness(1, 1)
    } else {
        card
    })
}

/// The names of the cards in seat 0's hand, sorted.
fn r6600_hand(driver: &Driver) -> Vec<String> {
    let state = driver.state();
    let mut names: Vec<String> = state
        .zone(Zone::Hand(pid(0)))
        .iter()
        .map(|id| name_of(state, *id))
        .collect();
    names.sort();
    names
}

#[rstest]
#[case::the_artifact_creature_as_the_artifact(&["Thopter", "Bear A", "Grove"], &["Bear A", "Grove", "Thopter"])]
#[case::the_artifact_creature_as_the_creature(&["", "Thopter", "Grove"], &["Grove", "Thopter"])]
#[case::one_creature_only(&["", "Bear A", ""], &["Bear A"])]
fn r6600_atraxa_takes_one_card_for_each_card_type(
    #[values(2, 3, 4)] seats: usize,
    #[case] picks: &[&str],
    #[case] hand: &[&str],
) -> Result<(), ScriptError> {
    // The top ten: an artifact creature, two creatures, a land and six spells of no type the picks use; two more below.
    let mut library = vec![
        r6600_typed("Thopter", &[CardType::Artifact, CardType::Creature]),
        r6600_typed("Bear A", &[CardType::Creature]),
        r6600_typed("Bear B", &[CardType::Creature]),
        r6600_typed("Grove", &[CardType::Land]),
    ];
    library.extend((0..6).map(|n| r6600_typed(&format!("Filler {n}"), &[])));
    library.extend((0..2).map(|n| r6600_typed(&format!("Deep Bear {n}"), &[CardType::Creature])));
    let mut board = vec![
        ready(forest()),
        ready(plains()),
        ready(island()),
        ready(swamp()),
    ];
    board.extend((0..3).map(|_| ready(wastes())));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board)
        .hand(0, [card("Atraxa, Grand Unifier").unwrap()])
        .library(0, library);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Atraxa, Grand Unifier")?;
    // The artifact, creature and land questions come in that order; each answer is the picked name, or nothing.
    let mut asked = 0;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseCards(options) => {
                let state = driver.state();
                assert!(
                    options
                        .from
                        .iter()
                        .all(|id| !name_of(state, *id).starts_with("Deep")),
                    "only the top ten are revealed"
                );
                let pick = picks.get(asked).copied().unwrap_or("");
                asked += 1;
                let chosen: Vec<_> = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| name_of(state, *id) == pick)
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && decision.player == pid(0) =>
            {
                break;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(
        asked, 3,
        "artifact, creature and land are asked; the other types have no card"
    );
    assert_eq!(r6600_hand(&driver), hand);
    let library = driver.state().zone(Zone::Library(pid(0))).len();
    assert_eq!(library, 12 - hand.len());
    Ok(())
}

/// A sorcery costing {1} that deals 1 damage to target creature.
fn r6600_ping() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Ping")
            .with_type(CardType::Sorcery)
            .with_mana_cost(mtg_testkit::cards::cost(1, &[]))
            .with_rules(rule),
    )
}

#[rstest]
#[case::no_experience(0, 0, true)]
#[case::two_experience_paid(2, 2, true)]
#[case::two_experience_unpaid(2, 1, false)]
fn r6600_minthara_ward_counts_experience(
    #[values(2, 3, 4)] seats: usize,
    #[case] experience: u32,
    #[case] lands: usize,
    #[case] hit: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(0, [ready(card("Minthara, Merciless Soul").unwrap())])
        .battlefield(1, (0..=lands).map(|_| ready(wastes())).collect::<Vec<_>>())
        .hand(1, [r6600_ping()]);
    if experience > 0 {
        scenario = scenario.counters(0, CounterKind::Other("experience".into()), experience);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let had = driver
        .state()
        .player(pid(0))
        .unwrap()
        .counters_of(&CounterKind::Other("experience".into()));
    assert_eq!(had, experience);
    let minthara = driver.target_named("Minthara, Merciless Soul")?;
    driver.cast_at("Ping", &[minthara])?;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(hit)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && decision.player == pid(1) =>
            {
                break;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let id = driver.id_of("Minthara, Merciless Soul").unwrap();
    let damage = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .damage();
    assert_eq!(damage, u32::from(hit));
    Ok(())
}
