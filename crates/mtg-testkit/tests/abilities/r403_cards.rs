//! Goal: the r40-3 pieces play as printed at tables of two, three and four seats: Thought Lash exiles the library of a
//! player who doesn't pay its cumulative upkeep and takes one card from one who pays; Hibernation's End finds a creature
//! with mana value equal to its age counters when its cumulative upkeep is paid and is sacrificed without a search when
//! it isn't; Grimdancer enters with the two different counters chosen and only those; a kicked Voidpouncer enters with
//! two +1/+1 counters, a trample counter and haste and an unkicked one with none; Denry Klin copies its counters onto a
//! nontoken creature entering under its controller only while it has counters; Anthem of Rakdos doubles the damage of
//! its controller's sources only while their hand is empty; and every log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, pid};
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{CardsChoice, CostsChoice, OptionChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, forest, free_creature, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// How the table answers what the casts and triggers ask.
#[derive(Debug, Clone, Default)]
struct R403Answers {
    /// Pay the optional costs (kicker) and say yes to a question.
    yes: bool,
    /// The option picked from a list (the counters of Grimdancer).
    option: usize,
    /// The cards picked from a choice, by name.
    picks: Vec<&'static str>,
    /// The player seat a target is chosen from, when the target is a player.
    target_seat: Option<usize>,
}

/// Answers until the stack is empty and nothing is pending.
fn r403_settle(driver: &mut Driver, how: &R403Answers) -> Result<(), ScriptError> {
    for _ in 0..120 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::ChooseCosts(_) => Answer::ChooseCosts(CostsChoice::Costs(if how.yes {
                vec![0]
            } else {
                Vec::new()
            })),
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(how.yes)),
            DecisionKind::ChooseOption(_) => Answer::ChooseOption(OptionChoice(how.option)),
            DecisionKind::ChooseCards(options) => {
                let chosen: Vec<_> = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| how.picks.contains(&name_of(driver.state(), *id).as_str()))
                    .take(usize::try_from(options.max).unwrap())
                    .collect();
                Answer::ChooseCards(CardsChoice(chosen))
            }
            DecisionKind::ChooseTargets(_) if how.target_seat.is_some() => {
                let seat = how.target_seat.unwrap();
                Answer::ChooseTargets(TargetChoice::Targets(vec![driver.target_player(seat)]))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn r403_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn r403_counters(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(kind)
}

fn r403_on_battlefield(driver: &Driver, name: &str) -> bool {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| name_of(driver.state(), *id) == name)
}

fn r403_other(name: &str) -> CounterKind {
    CounterKind::Other(name.to_owned())
}

/// Seat 0 holds the permanent on the opening turn; every seat has five cards in its library.
fn r403_upkeep_table(
    seats: usize,
    permanent: &str,
    library: fn() -> std::sync::Arc<mtg_cards::CardDefinition>,
) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card(permanent).unwrap()), ready(forest())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| library()));
    }
    scenario
}

#[rstest]
fn r403_thought_lash_exiles_the_library_of_a_player_who_does_not_pay(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] paid: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r403_upkeep_table(seats, "Thought Lash", forest))?;
    driver.idle_until(1 + u32::try_from(seats).unwrap(), Step::Upkeep)?;
    r403_settle(
        &mut driver,
        &R403Answers {
            yes: paid,
            ..R403Answers::default()
        },
    )?;
    let library = driver.state().zone(Zone::Library(pid(0))).len();
    assert_eq!(
        library,
        if paid { 4 } else { 0 },
        "the payment took one card"
    );
    assert_eq!(r403_on_battlefield(&driver, "Thought Lash"), paid);
    for seat in 1..seats {
        assert_eq!(
            driver.state().zone(Zone::Library(pid(seat))).len(),
            4,
            "each other seat drew once"
        );
    }
    r403_replays(&driver);
    Ok(())
}

#[rstest]
fn r403_hibernations_end_searches_only_when_paid(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] paid: bool,
) -> Result<(), ScriptError> {
    let elf = || card("Llanowar Elves").unwrap();
    let mut driver = Driver::from_scenario(r403_upkeep_table(seats, "Hibernation's End", elf))?;
    driver.idle_until(1 + u32::try_from(seats).unwrap(), Step::Upkeep)?;
    let how = R403Answers {
        yes: paid,
        picks: vec!["Llanowar Elves"],
        ..R403Answers::default()
    };
    r403_settle(&mut driver, &how)?;
    assert_eq!(r403_on_battlefield(&driver, "Llanowar Elves"), paid);
    assert_eq!(r403_on_battlefield(&driver, "Hibernation's End"), paid);
    r403_replays(&driver);
    Ok(())
}

#[rstest]
fn r403_grimdancer_enters_with_the_two_counters_chosen(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] option: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Grimdancer").unwrap()])
        .battlefield(0, (0..3).map(|_| ready(swamp())));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Grimdancer")?;
    r403_settle(
        &mut driver,
        &R403Answers {
            option,
            ..R403Answers::default()
        },
    )?;
    let sets = [
        ["menace", "deathtouch"],
        ["menace", "lifelink"],
        ["deathtouch", "lifelink"],
    ];
    for kind in ["menace", "deathtouch", "lifelink"] {
        let expected = u32::from(sets[option].contains(&kind));
        assert_eq!(
            r403_counters(&driver, "Grimdancer", &r403_other(kind)),
            expected,
            "{kind}"
        );
    }
    r403_replays(&driver);
    Ok(())
}

#[rstest]
fn r403_voidpouncer_enters_with_both_kinds_only_when_kicked(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] kicked: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Voidpouncer").unwrap()])
        .battlefield(
            0,
            (0..4)
                .map(|_| ready(mountain()))
                .chain([ready(card("Wastes").unwrap())]),
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Voidpouncer")?;
    r403_settle(
        &mut driver,
        &R403Answers {
            yes: kicked,
            ..R403Answers::default()
        },
    )?;
    let plus = r403_counters(&driver, "Voidpouncer", &CounterKind::PlusOnePlusOne);
    let trample = r403_counters(&driver, "Voidpouncer", &r403_other("trample"));
    assert_eq!((plus, trample), if kicked { (2, 1) } else { (0, 0) });
    let id = driver.id_of("Voidpouncer")?;
    let keywords = mtg_engine::keywords::keywords_of(driver.state(), id);
    assert_eq!(keywords.iter().any(|k| k.name() == "Haste"), kicked);
    r403_replays(&driver);
    Ok(())
}

#[rstest]
fn r403_denry_copies_its_counters_while_it_has_some(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2)] plus: u32,
) -> Result<(), ScriptError> {
    let denry = card("Denry Klin, Editor in Chief").unwrap();
    let mut denry = ready(denry);
    if plus > 0 {
        denry = with_counters(denry, CounterKind::PlusOnePlusOne, plus);
        denry = with_counters(denry, r403_other("vigilance"), 1);
    }
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [free_creature("Recruit", 1, 1)])
        .battlefield(0, [denry]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Recruit")?;
    r403_settle(&mut driver, &R403Answers::default())?;
    assert_eq!(
        r403_counters(&driver, "Recruit", &CounterKind::PlusOnePlusOne),
        plus
    );
    assert_eq!(
        r403_counters(&driver, "Recruit", &r403_other("vigilance")),
        u32::from(plus > 0)
    );
    r403_replays(&driver);
    Ok(())
}

#[rstest]
fn r403_anthem_doubles_damage_only_with_an_empty_hand(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] hellbent: bool,
) -> Result<(), ScriptError> {
    let mut hand = vec![burn("Shock", 2)];
    if !hellbent {
        hand.push(island());
    }
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(
            0,
            [
                ready(card("Anthem of Rakdos").unwrap()),
                ready(mountain()),
                ready(plains()),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    let target = seats - 1;
    driver.begin_cast("Shock")?;
    r403_settle(
        &mut driver,
        &R403Answers {
            target_seat: Some(target),
            ..R403Answers::default()
        },
    )?;
    let life = driver.state().player(pid(target)).unwrap().life();
    assert_eq!(life, if hellbent { 16 } else { 18 });
    r403_replays(&driver);
    Ok(())
}

/// A noncreature permanent of seat `seat` with the card types given, named `name`.
fn r403_typed(
    name: &str,
    types: &[mtg_core::card::CardType],
) -> std::sync::Arc<mtg_cards::CardDefinition> {
    let definition = types.iter().fold(
        mtg_cards::CardDefinition::new(name),
        |definition, card_type| definition.with_type(*card_type),
    );
    std::sync::Arc::new(definition)
}

/// The exchange spells refuse a second target that shares none of their types with the first (Role Reversal: a Kindred
/// artifact and a Kindred enchantment share a card type but no permanent type; Legerdemain: a creature and a
/// noncreature artifact share neither of its types) and exchange two creatures.
#[rstest]
fn r403_exchanges_need_a_shared_type_of_the_spell(
    #[values(2, 3, 4)] seats: usize,
    #[values("Role Reversal", "Legerdemain")] spell: &str,
) -> Result<(), ScriptError> {
    use mtg_core::card::CardType;
    let (mine, theirs): (&str, &str) = if spell == "Role Reversal" {
        ("Totem", "Idol")
    } else {
        ("Mine", "Relic")
    };
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, [card(spell).unwrap()])
            .battlefield(
                0,
                (0..2)
                    .map(|_| ready(island()))
                    .chain((0..2).map(|_| ready(mountain())))
                    .chain([
                        ready(free_creature("Mine", 1, 1)),
                        ready(r403_typed(
                            "Totem",
                            &[CardType::Kindred, CardType::Artifact],
                        )),
                    ]),
            )
            .battlefield(
                1,
                [
                    ready(free_creature("Theirs", 3, 3)),
                    ready(r403_typed(
                        "Idol",
                        &[CardType::Kindred, CardType::Enchantment],
                    )),
                    ready(r403_typed("Relic", &[CardType::Artifact])),
                ],
            ),
    )?;
    driver.begin_cast(spell)?;
    let first = driver.target_named(mine)?;
    driver.choose_targets(&[first])?;
    let refused = driver.target_named(theirs)?;
    assert!(
        driver.choose_targets(&[refused]).is_err(),
        "{mine} and {theirs} share no type of {spell}"
    );
    if spell == "Role Reversal" {
        driver.answer(Answer::ChooseTargets(TargetChoice::Cancel))?;
        driver.begin_cast(spell)?;
        let mine = driver.target_named("Mine")?;
        driver.choose_targets(&[mine])?;
    }
    let theirs = driver.target_named("Theirs")?;
    driver.choose_targets(&[theirs])?;
    r403_settle(&mut driver, &R403Answers::default())?;
    let controller = |name: &str| {
        mtg_engine::characteristics::controller_of(driver.state(), driver.id_of(name).unwrap())
    };
    assert_eq!(controller("Mine"), Some(pid(1)));
    assert_eq!(controller("Theirs"), Some(pid(0)));
    r403_replays(&driver);
    Ok(())
}

/// Seat 0 casts a burn of `amount` at a 2/2 of the last seat: excess noncombat damage beyond 2 amasses that many Orcs
/// for Fall of Cair Andros and makes one Treasure for Magmatic Galleon; 2 damage is no excess and does neither.
#[rstest]
fn r403_excess_noncombat_damage_triggers(
    #[values(2, 3, 4)] seats: usize,
    #[values(2, 5)] amount: i32,
    #[values("Fall of Cair Andros", "Magmatic Galleon")] permanent: &str,
) -> Result<(), ScriptError> {
    let foe = seats - 1;
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [burn("Blast", amount)])
        .battlefield(0, [ready(card(permanent).unwrap()), ready(mountain())])
        .battlefield(foe, [ready(free_creature("Bear", 2, 2))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Blast")?;
    let bear = driver.target_named("Bear")?;
    driver.choose_targets(&[bear])?;
    r403_settle(&mut driver, &R403Answers::default())?;
    let excess = u32::try_from(amount - 2).unwrap();
    if permanent == "Fall of Cair Andros" {
        let army = driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .copied()
            .find(|id| {
                driver
                    .state()
                    .object(*id)
                    .is_some_and(|o| o.card().name.contains("Army"))
            });
        let counters = army.map_or(0, |id| {
            driver
                .state()
                .object(id)
                .unwrap()
                .permanent()
                .unwrap()
                .counters_of(&CounterKind::PlusOnePlusOne)
        });
        assert_eq!(counters, excess);
    } else {
        let treasures = driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter(|id| name_of(driver.state(), **id) == "Treasure")
            .count();
        assert_eq!(treasures, usize::from(excess > 0));
    }
    r403_replays(&driver);
    Ok(())
}

/// Rith makes a Dragon at its controller's end step only when an opponent's creature was dealt excess damage this turn.
#[rstest]
fn r403_rith_counts_excess_damage_this_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values(2, 3)] amount: i32,
) -> Result<(), ScriptError> {
    let foe = seats - 1;
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [burn("Blast", amount)])
        .battlefield(
            0,
            [
                ready(card("Rith, Liberated Primeval").unwrap()),
                ready(mountain()),
            ],
        )
        .battlefield(foe, [ready(free_creature("Bear", 2, 2))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Blast")?;
    let bear = driver.target_named("Bear")?;
    driver.choose_targets(&[bear])?;
    r403_settle(&mut driver, &R403Answers::default())?;
    driver.idle_until(1, Step::End)?;
    r403_settle(&mut driver, &R403Answers::default())?;
    let dragons = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(driver.state(), **id) == "Dragon")
        .count();
    assert_eq!(dragons, usize::from(amount > 2));
    r403_replays(&driver);
    Ok(())
}
