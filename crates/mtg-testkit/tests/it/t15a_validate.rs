//! Goal: the popular cards of the sweep play the edges a Commander table hits at two, three and four seats: Smothering Tithe asks each drawing opponent and gives its controller the Treasure, Mana Drain adds the countered spell's mana value at its caster's next main phase (and nothing when it is countered itself), Force of Will pays 1 life and a blue card, Mana Vault stays tapped and hurts in the draw step unless {4} is paid in the upkeep, The Great Henge and Otawara cost less as printed, Syr Konrad counts milled cards and creatures dying beside it, and Three Tree City adds one color as often as there are creatures of the chosen type.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::CardType;
use mtg_core::mana::ManaType;
use mtg_engine::GameState;
use mtg_engine::choose::{
    CardsChoice, ChoiceOption, OptionChoice, Target, TargetChoice, YesNoChoice,
};
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::mana::ManaSource;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, forest, island, mountain, plains, study, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

use crate::common::casting::ways;
use crate::common::{names, on_battlefield, pid, spell};

/// Answers with `policy`, else passively, until `done` holds at an idle priority; returns the seats asked yes or no.
fn t15a_v_drive(
    driver: &mut Driver,
    done: impl Fn(&GameState) -> bool,
    mut policy: impl FnMut(&Driver, &Decision) -> Option<Answer>,
) -> Result<Vec<usize>, ScriptError> {
    let mut asked = Vec::new();
    for _ in 0..800 {
        let decision = driver.decision()?.clone();
        let state = driver.state();
        let idle = state.stack().is_empty()
            && state.resolving().is_none()
            && state.pending_triggers().is_empty();
        if matches!(decision.kind, DecisionKind::Priority(_)) && idle && done(state) {
            return Ok(asked);
        }
        if matches!(decision.kind, DecisionKind::YesNo(_)) {
            asked.push(decision.player.index());
        }
        let answer = policy(driver, &decision).unwrap_or_else(|| Driver::passive_answer(&decision));
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("t15a validate".into()))
}

/// Pays every payment automatically and answers yes or no with `yes`.
fn t15a_v_paying(yes: bool) -> impl FnMut(&Driver, &Decision) -> Option<Answer> {
    move |_, decision| match decision.kind {
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        DecisionKind::YesNo(_) => Some(Answer::YesNo(YesNoChoice(yes))),
        _ => None,
    }
}

/// The permanents named `name` that `seat` controls.
fn t15a_v_count(state: &GameState, name: &str, seat: usize) -> usize {
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.card().name == name && o.controller() == pid(seat))
        .count()
}

/// The mana of `kind` in the pool of `seat`.
fn t15a_v_pool(driver: &Driver, seat: usize, kind: ManaType) -> u32 {
    driver.state().player(pid(seat)).unwrap().mana().get(kind)
}

/// Whether the named permanent is tapped.
fn t15a_v_tapped(state: &GameState, name: &str) -> bool {
    on_battlefield(state, name)
        .and_then(|id| state.object(id))
        .and_then(|o| o.permanent())
        .is_some_and(mtg_engine::object::PermanentState::tapped)
}

/// A table of `seats` at turn `turn` of `active`, every seat with a library of Swamps to draw from.
fn t15a_v_table(seats: usize, turn: u32, active: usize, step: Step) -> Scenario {
    let mut scenario = Scenario::new(seats).at(turn, active, step);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..6).map(|_| swamp()));
    }
    scenario
}

#[rstest]
fn t15a_v_smothering_tithe_asks_each_drawing_opponent(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] pays: bool,
) -> Result<(), ScriptError> {
    let wheel = spell(
        "Each Draws",
        CardType::Sorcery,
        "[]",
        r#"[{"Draw":{"who":"EachPlayer"}}]"#,
    );
    let mut scenario = t15a_v_table(seats, 1, 0, Step::Main1)
        .hand(0, [wheel])
        .battlefield(0, [ready(card("Smothering Tithe").unwrap())]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(swamp()), ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Each Draws")?;
    let mut asked = t15a_v_drive(&mut driver, |_| true, t15a_v_paying(pays))?;
    asked.sort_unstable();
    assert_eq!(
        asked,
        (1..seats).collect::<Vec<_>>(),
        "each opponent decides once"
    );
    let treasures = if pays { 0 } else { seats - 1 };
    assert_eq!(t15a_v_count(driver.state(), "Treasure", 0), treasures);
    for seat in 1..seats {
        assert_eq!(t15a_v_count(driver.state(), "Treasure", seat), 0);
    }
    Ok(())
}

/// Passes priority until `seat` holds it.
fn t15a_v_priority_to(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    for _ in 0..8 {
        let decision = driver.decision()?;
        if decision.player == pid(seat) && matches!(decision.kind, DecisionKind::Priority(_)) {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck(format!("priority to seat {seat}")))
}

/// The top object of the stack as a target.
fn t15a_v_top(driver: &Driver) -> Target {
    let top = *driver.state().stack().last().unwrap();
    Target::Object(driver.state().object(top).unwrap().reference())
}

#[rstest]
fn t15a_v_mana_drain_adds_at_the_next_main_phase_of_its_caster(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] drained_back: bool,
) -> Result<(), ScriptError> {
    let scenario = t15a_v_table(seats, 1, 1, Step::Main1)
        .hand(0, [card("Mana Drain").unwrap()])
        .battlefield(0, [ready(island()), ready(island())])
        .hand(
            1,
            [
                card("Dregscape Zombie").unwrap(),
                card("Counterspell").unwrap(),
            ],
        )
        .battlefield(
            1,
            (0..2)
                .map(|_| ready(swamp()))
                .chain((0..2).map(|_| ready(island()))),
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping("Dregscape Zombie", &["Swamp", "Swamp"])?;
    t15a_v_priority_to(&mut driver, 0)?;
    let zombie = t15a_v_top(&driver);
    driver.cast_at("Mana Drain", &[zombie])?;
    if drained_back {
        t15a_v_priority_to(&mut driver, 1)?;
        let drain = t15a_v_top(&driver);
        driver.cast_at("Counterspell", &[drain])?;
    }
    t15a_v_drive(&mut driver, |_| true, |_, _| None)?;
    let zombie_on = on_battlefield(driver.state(), "Dregscape Zombie").is_some();
    assert_eq!(
        zombie_on, drained_back,
        "the zombie is countered unless Mana Drain is"
    );
    // The opponent's postcombat main is not the caster's main phase.
    driver.idle_until(1, Step::Main2)?;
    t15a_v_drive(&mut driver, |_| true, |_, _| None)?;
    assert_eq!(t15a_v_pool(&driver, 0, ManaType::Colorless), 0);
    let mine = u32::try_from(seats).unwrap();
    driver.idle_until(mine, Step::Main1)?;
    t15a_v_drive(&mut driver, |_| true, |_, _| None)?;
    let expected = if drained_back { 0 } else { 2 };
    assert_eq!(t15a_v_pool(&driver, 0, ManaType::Colorless), expected);
    Ok(())
}

#[rstest]
fn t15a_v_mana_drain_on_the_casters_own_turn_pays_in_the_postcombat_main(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = t15a_v_table(seats, 1, 0, Step::Main1)
        .hand(
            0,
            [
                card("Mana Drain").unwrap(),
                card("Dregscape Zombie").unwrap(),
            ],
        )
        .battlefield(
            0,
            (0..2)
                .map(|_| ready(island()))
                .chain((0..2).map(|_| ready(swamp()))),
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping("Dregscape Zombie", &["Swamp", "Swamp"])?;
    let zombie = t15a_v_top(&driver);
    driver.cast_at("Mana Drain", &[zombie])?;
    t15a_v_drive(&mut driver, |_| true, |_, _| None)?;
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))).len(), 2);
    driver.idle_until(1, Step::Main2)?;
    t15a_v_drive(&mut driver, |_| true, |_, _| None)?;
    assert_eq!(t15a_v_pool(&driver, 0, ManaType::Colorless), 2);
    Ok(())
}

/// Force of Will as `mtg-cli compile` reads it from the oracle text (it is compiled, not in the corpus).
fn t15a_v_force_of_will() -> std::sync::Arc<mtg_cards::CardDefinition> {
    let free =
        r#"{"FreeCast":{"cost":[{"PayLife":1},{"ExileFromHand":{"filter":[{"Color":"Blue"}]}}]}}"#;
    let counter = r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#;
    std::sync::Arc::new(
        mtg_cards::CardDefinition::new("Force of Will")
            .with_type(CardType::Instant)
            .with_mana_cost(mtg_testkit::cards::cost(
                3,
                &[mtg_core::color::Color::Blue, mtg_core::color::Color::Blue],
            ))
            .with_rules(serde_json::from_str(free).unwrap())
            .with_rules(serde_json::from_str(counter).unwrap()),
    )
}

/// Casts Force of Will for its alternative cost at the top spell, exiling the first blue card offered.
fn t15a_v_force(driver: &mut Driver) -> Result<(), ScriptError> {
    let bolt = t15a_v_top(driver);
    driver.begin_cast("Force of Will")?;
    for _ in 0..10 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::ChooseOption(_) => {
                let offered = ways(driver)?;
                let at = offered.iter().position(|way| way.contains("life")).unwrap();
                Answer::ChooseOption(OptionChoice(at))
            }
            DecisionKind::ChooseTargets(_) => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![bolt]))
            }
            DecisionKind::ChooseCards(options) => {
                Answer::ChooseCards(CardsChoice(options.from[..1].to_vec()))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::Priority(_) => return Ok(()),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("Force of Will".into()))
}

#[rstest]
fn t15a_v_force_of_will_pays_a_life_and_a_blue_card(
    #[values(2, 3, 4)] seats: usize,
    #[values(20, 2)] life: i32,
) -> Result<(), ScriptError> {
    let scenario = t15a_v_table(seats, 1, 1, Step::Main1)
        .life(0, life)
        .hand(
            0,
            [
                t15a_v_force_of_will(),
                study("Blue Study"),
                burn("Red Bolt", 1),
            ],
        )
        .hand(1, [burn("Bolt", 3)])
        .battlefield(1, [ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario)?;
    let me = driver.target_player(0);
    driver.cast_at("Bolt", &[me])?;
    t15a_v_priority_to(&mut driver, 0)?;
    t15a_v_force(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Exile), ["Blue Study"]);
    driver.expect_life(0, life - 1)?;
    t15a_v_drive(&mut driver, |_| true, |_, _| None)?;
    driver.expect_life(0, life - 1)?;
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Red Bolt"]);
    Ok(())
}

#[rstest]
#[case::no_other_blue_card(&["Red Bolt"])]
#[case::no_card_at_all(&[])]
fn t15a_v_force_of_will_cannot_exile_itself(
    #[values(2, 3, 4)] seats: usize,
    #[case] others: &[&str],
) -> Result<(), ScriptError> {
    let mut hand = vec![t15a_v_force_of_will()];
    hand.extend(others.iter().map(|name| burn(name, 1)));
    let scenario = t15a_v_table(seats, 1, 1, Step::Main1)
        .hand(0, hand)
        .hand(1, [burn("Bolt", 3)])
        .battlefield(1, [ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario)?;
    let me = driver.target_player(0);
    driver.cast_at("Bolt", &[me])?;
    t15a_v_priority_to(&mut driver, 0)?;
    assert!(
        driver.begin_cast("Force of Will").is_err(),
        "no blue card to exile and no mana"
    );
    Ok(())
}

#[rstest]
fn t15a_v_force_of_will_at_one_life_is_paid_and_loses(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = t15a_v_table(seats, 1, 1, Step::Main1)
        .life(0, 1)
        .hand(0, [t15a_v_force_of_will(), study("Blue Study")])
        .hand(1, [burn("Bolt", 3)])
        .battlefield(1, [ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario)?;
    let opponent = driver.target_player(2);
    driver.cast_at("Bolt", &[opponent])?;
    t15a_v_priority_to(&mut driver, 0)?;
    t15a_v_force(&mut driver)?;
    assert!(
        !driver.state().player(pid(0)).unwrap().is_in_game(),
        "life 0 loses the game"
    );
    Ok(())
}

#[rstest]
fn t15a_v_mana_vault_stays_tapped_and_hurts_unless_paid(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] pays: bool,
) -> Result<(), ScriptError> {
    let scenario = t15a_v_table(seats, 1, seats - 1, Step::Main2).battlefield(
        0,
        [tapped(card("Mana Vault").unwrap())]
            .into_iter()
            .chain((0..4).map(|_| tapped(swamp()))),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let at_main =
        |state: &GameState| state.turn().number() == 2 && state.turn().step() == Step::Main1;
    let asked = t15a_v_drive(&mut driver, at_main, t15a_v_paying(pays))?;
    assert_eq!(asked, [0], "the controller is asked once, in the upkeep");
    assert_eq!(t15a_v_tapped(driver.state(), "Mana Vault"), !pays);
    driver.expect_life(0, if pays { 20 } else { 19 })?;
    for seat in 1..seats {
        driver.expect_life(seat, 20)?;
    }
    Ok(())
}

#[rstest]
#[case::no_creature(&[], "{7}{G}{G}")]
#[case::power_two(&["Dregscape Zombie"], "{5}{G}{G}")]
#[case::greatest_not_sum(&["Dregscape Zombie", "Krenko, Tin Street Kingpin"], "{5}{G}{G}")]
#[case::power_five(&["Syr Konrad, the Grim", "Dregscape Zombie"], "{2}{G}{G}")]
fn t15a_v_the_great_henge_costs_less_by_the_greatest_power(
    #[values(2, 3, 4)] seats: usize,
    #[case] mine: &[&str],
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let creatures = mine.iter().map(|name| ready(card(name).unwrap()));
    let scenario = t15a_v_table(seats, 1, 0, Step::Main1)
        .hand(0, [card("The Great Henge").unwrap()])
        .battlefield(0, creatures.chain((0..9).map(|_| ready(forest()))))
        .battlefield(seats - 1, [ready(card("Niv-Mizzet, Parun").unwrap())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("The Great Henge")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), expected);
    Ok(())
}

#[rstest]
#[case::no_legend(0, "{3}{U}")]
#[case::one_legend(1, "{2}{U}")]
#[case::three_legends(3, "{U}")]
fn t15a_v_otawara_channel_costs_less_per_legendary_creature(
    #[values(2, 3, 4)] seats: usize,
    #[case] legends: usize,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let names_of = [
        "Krenko, Tin Street Kingpin",
        "Syr Konrad, the Grim",
        "Niv-Mizzet, Parun",
    ];
    let mine = names_of[..legends]
        .iter()
        .map(|name| ready(card(name).unwrap()));
    let scenario = t15a_v_table(seats, 1, 0, Step::Main1)
        .hand(0, [card("Otawara, Soaring City").unwrap()])
        .battlefield(0, mine.chain((0..4).map(|_| ready(island()))))
        .battlefield(seats - 1, [ready(card("Dregscape Zombie").unwrap())]);
    let mut driver = Driver::from_scenario(scenario)?;
    let zombie = driver.target_named("Dregscape Zombie")?;
    driver.begin_activation("Otawara, Soaring City")?;
    driver.choose_targets(&[zombie])?;
    assert_eq!(driver.pay_options()?.cost.to_string(), expected);
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    t15a_v_drive(&mut driver, |_| true, |_, _| None)?;
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(seats - 1))),
        ["Dregscape Zombie"]
    );
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Otawara, Soaring City"]
    );
    Ok(())
}

#[rstest]
fn t15a_v_syr_konrad_counts_each_creature_card_milled(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] creatures: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Syr Konrad, the Grim").unwrap()),
            ready(swamp()),
            ready(swamp()),
        ],
    );
    for seat in 0..seats {
        let top = if seat < creatures {
            card("Dregscape Zombie").unwrap()
        } else {
            swamp()
        };
        scenario = scenario.library(seat, [top, swamp(), swamp()]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_ability("Syr Konrad, the Grim", 1)?;
    t15a_v_drive(&mut driver, |_| true, t15a_v_paying(true))?;
    let milled: usize = (0..seats)
        .map(|seat| names(driver.state(), Zone::Graveyard(pid(seat))).len())
        .sum();
    assert_eq!(milled, seats, "each player milled a card");
    let hurt = i32::try_from(creatures.min(seats)).unwrap();
    driver.expect_life(0, 20)?;
    for seat in 1..seats {
        driver.expect_life(seat, 20 - hurt)?;
    }
    Ok(())
}

#[rstest]
fn t15a_v_syr_konrad_sees_the_creatures_dying_beside_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2)] others: usize,
) -> Result<(), ScriptError> {
    let zombies = (0..others).map(|_| ready(card("Dregscape Zombie").unwrap()));
    let scenario = t15a_v_table(seats, 1, 0, Step::Main1)
        .hand(0, [card("Wrath of God").unwrap()])
        .battlefield(
            0,
            [ready(card("Syr Konrad, the Grim").unwrap())]
                .into_iter()
                .chain((0..4).map(|_| ready(plains()))),
        )
        .battlefield(seats - 1, zombies);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Wrath of God")?;
    t15a_v_drive(&mut driver, |_| true, |_, _| None)?;
    let hurt = i32::try_from(others).unwrap();
    for seat in 1..seats {
        driver.expect_life(seat, 20 - hurt)?;
    }
    Ok(())
}

#[rstest]
#[ignore = "t15o: the {2}, {T} ability of Three Tree City is not offered among the mana abilities"]
fn t15a_v_three_tree_city_adds_one_color_per_creature_of_the_type(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 3)] zombies: usize,
) -> Result<(), ScriptError> {
    let creatures = (0..zombies).map(|_| ready(card("Dregscape Zombie").unwrap()));
    let scenario = t15a_v_table(seats, 1, 0, Step::Main1)
        .hand(0, [card("Three Tree City").unwrap()])
        .battlefield(
            0,
            creatures.chain([
                ready(card("Krenko, Tin Street Kingpin").unwrap()),
                ready(forest()),
                ready(forest()),
            ]),
        )
        .battlefield(seats - 1, [ready(card("Dregscape Zombie").unwrap())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.play_land("Three Tree City")?;
    let pick = |_: &Driver, decision: &Decision, label: &str| match &decision.kind {
        DecisionKind::ChooseOption(options) => options
            .options
            .iter()
            .position(|o| matches!(o, ChoiceOption::Label(l) if l.contains(label)))
            .map(|at| Answer::ChooseOption(OptionChoice(at)))
            .or_else(|| panic!("no {label} among {:?}", options.options)),
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        _ => None,
    };
    t15a_v_drive(
        &mut driver,
        |_| true,
        |d, decision| pick(d, decision, "Zombie"),
    )?;
    let city = on_battlefield(driver.state(), "Three Tree City").unwrap();
    // The {2} of the cost is paid from the pool: the Forests are tapped first.
    driver.tap_for_mana("Forest")?;
    driver.tap_for_mana("Forest")?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority after mana");
    };
    let colored: Vec<ManaSource> = options
        .mana_abilities
        .iter()
        .filter(|m| m.source.object == city && m.produces != ManaType::Colorless)
        .map(|m| m.source)
        .collect();
    if zombies == 0 {
        assert_eq!(colored, [], "no mana to add, nothing offered");
        return Ok(());
    }
    driver.answer(Answer::Priority(PriorityAction::ActivateMana(colored[0])))?;
    for _ in 0..6 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) {
            break;
        }
        let answer =
            pick(&driver, &decision, "Black").unwrap_or_else(|| Driver::passive_answer(&decision));
        driver.answer(answer)?;
    }
    assert_eq!(
        t15a_v_pool(&driver, 0, ManaType::Black),
        u32::try_from(zombies).unwrap()
    );
    for other in [
        ManaType::White,
        ManaType::Blue,
        ManaType::Red,
        ManaType::Green,
        ManaType::Colorless,
    ] {
        assert_eq!(t15a_v_pool(&driver, 0, other), 0, "{other:?}");
    }
    Ok(())
}

/// The game of five seats, seed 3179, that found it: Kalitas's controller loses as an opponent's creature dies.
#[rstest]
fn t15a_v_a_player_who_loses_as_kalitas_replaces_a_death_gets_no_zombie(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let blast = spell(
        "Blast",
        CardType::Instant,
        r#"[{"what":"AnyTarget"}]"#,
        r#"[{"DealDamage":{"to":{"Target":0},"amount":3}},{"DealDamage":{"to":"Opponents","amount":3}}]"#,
    );
    let scenario = t15a_v_table(seats, 1, 0, Step::Main1)
        .hand(0, [blast])
        .battlefield(0, [ready(card("Dregscape Zombie").unwrap())])
        .battlefield(1, [ready(card("Kalitas, Traitor of Ghet").unwrap())])
        .life(1, 3);
    let mut driver = Driver::from_scenario(scenario)?;
    let zombie = driver.target_named("Dregscape Zombie")?;
    driver.cast_at("Blast", &[zombie])?;
    t15a_v_drive(&mut driver, |_| true, |_, _| None)?;
    assert!(!driver.state().player(pid(1)).unwrap().is_in_game());
    assert_eq!(
        on_battlefield(driver.state(), "Kalitas, Traitor of Ghet"),
        None
    );
    assert!(
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .all(|id| driver
                .state()
                .object(*id)
                .is_some_and(|o| o.controller() != pid(1))),
        "nothing is made for the player who left"
    );
    Ok(())
}
