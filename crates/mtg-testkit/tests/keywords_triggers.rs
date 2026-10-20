//! Goal: prowess, battle cry and ward, the keywords that are triggered abilities, play whole at tables of two, three and four seats as their rules say: a creature with prowess grows by 1/+1 for each noncreature spell its controller casts, once for each instance of the keyword, and shrinks back at the end of the turn; battle cry gives every other attacker +1/+0 once it resolves, so the order of Hero of Bladehold's two attack triggers decides whether its Soldiers get it; ward counters a spell or an ability an opponent controls that targets the creature unless that player pays the cost (asked, and not asked when they cannot pay), asks no one for the controller's own spells, and does nothing for a cast that was backed out of; the triggers are told apart in the ordering prompt, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::Modification;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_core::card::CardType;
use mtg_engine::abilities::AbilitySource;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, GameSetup, characteristics, replay};
use mtg_testkit::cards::{burn, forest, free_creature, keyworded, mountain, pinger};
use mtg_testkit::corpus::card;
use mtg_testkit::pump_cards::grant;
use mtg_testkit::trigger_cards::creature_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn ward(cost: &str) -> Keyword {
    Keyword::Ward(Cost::mana(cost.parse().unwrap()))
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// The power and toughness of the permanent called `name`.
fn size(driver: &Driver, name: &str) -> (i32, i32) {
    let id = driver.id_of(name).unwrap();
    let chars = characteristics(driver.state(), id).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

/// The log replays to the live state, and an engine rebuilt from it stands at the same decision.
#[allow(clippy::unnecessary_wraps)] // the tests end with it, as they end with their last script step
fn assert_replays(driver: &Driver, setup: &GameSetup) -> Result<(), ScriptError> {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
    assert_eq!(resumed.decision(), driver.engine().decision());
    Ok(())
}

/// Passes priority while something is on the stack and nobody is asked anything else.
fn resolve_everything(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let mut events = Vec::new();
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        && !driver.state().stack().is_empty()
    {
        events.extend(driver.pass()?);
    }
    Ok(events)
}

/// The triggers a keyword added to the permanent called `name`, among `events`.
fn keyword_triggers(driver: &Driver, events: &[Event], name: &str) -> usize {
    events
        .iter()
        .filter(|event| match event {
            Event::AbilityTriggered(trigger) => {
                name_of(driver.state(), trigger.source.id) == name
                    && matches!(
                        trigger.origin,
                        AbilitySource::PrintedKeyword(_) | AbilitySource::GrantedKeyword(_)
                    )
            }
            _ => false,
        })
        .count()
}

/// Seat `active` has priority in its first main phase. Seat 0 controls the prowess creature, a Mountain for each spell in
/// `hand`, and holds `hand`; the other seats have nothing.
fn prowess_table(
    seats: usize,
    prowler: Arc<CardDefinition>,
    hand: Vec<Arc<CardDefinition>>,
) -> Scenario {
    let lands = vec![ready(mountain()); hand.len()];
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [vec![ready(prowler)], lands].concat())
        .hand(0, hand)
}

fn prowler() -> Arc<CardDefinition> {
    keyworded("Prowler", 1, 2, &[Keyword::Prowess])
}

#[rstest]
fn a_noncreature_spell_makes_prowess_grow_the_creature_until_the_end_of_the_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] spells: usize,
) -> Result<(), ScriptError> {
    let hand = vec![burn("Bolt", 1); spells];
    let setup = prowess_table(seats, prowler(), hand.clone())
        .setup()
        .unwrap();
    let mut driver = Driver::from_scenario(prowess_table(seats, prowler(), hand))?;
    let mut events = Vec::new();
    for _ in 0..spells {
        let foe = driver.target_player(1);
        events.extend(driver.cast_at("Bolt", &[foe])?);
        events.extend(resolve_everything(&mut driver)?);
    }
    assert_eq!(keyword_triggers(&driver, &events, "Prowler"), spells);
    let more = i32::try_from(spells).unwrap();
    assert_eq!(size(&driver, "Prowler"), (1 + more, 2 + more));
    // The growth lasts to the cleanup step of the turn and no longer.
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(size(&driver, "Prowler"), (1, 2));
    assert_replays(&driver, &setup)
}

#[test]
fn a_creature_spell_is_not_a_noncreature_spell() -> Result<(), ScriptError> {
    let hand = vec![free_creature("Bear", 2, 2)];
    let table = || prowess_table(2, prowler(), hand.clone());
    let mut driver = Driver::from_scenario(table())?;
    let events = driver.cast("Bear")?;
    assert_eq!(keyword_triggers(&driver, &events, "Prowler"), 0);
    resolve_everything(&mut driver)?;
    assert_eq!(size(&driver, "Prowler"), (1, 2));
    assert_replays(&driver, &table().setup().unwrap())
}

#[test]
fn only_the_spells_of_its_controller_count() -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(3)
            .at(1, 1, Step::Main1)
            .battlefield(0, [ready(prowler())])
            .battlefield(1, [ready(mountain()), ready(prowler())])
            .hand(1, [burn("Bolt", 1)])
    };
    let mut driver = Driver::from_scenario(table())?;
    let foe = driver.target_player(2);
    let events = driver.cast_at("Bolt", &[foe])?;
    resolve_everything(&mut driver)?;
    // The creature of the caster grows; the one of another seat does not.
    assert_eq!(keyword_triggers(&driver, &events, "Prowler"), 1);
    let sizes: Vec<(i32, i32)> = [0, 1]
        .map(|seat| {
            let id = driver
                .state()
                .zone(Zone::Battlefield)
                .iter()
                .copied()
                .find(|id| {
                    name_of(driver.state(), *id) == "Prowler"
                        && driver.state().object(*id).unwrap().controller() == pid(seat)
                });
            let chars = characteristics(driver.state(), id.unwrap()).unwrap();
            (chars.power.unwrap(), chars.toughness.unwrap())
        })
        .to_vec();
    assert_eq!(sizes, [(1, 2), (2, 3)]);
    assert_replays(&driver, &table().setup().unwrap())
}

#[test]
fn each_instance_of_prowess_triggers_for_itself_whether_printed_or_given() -> Result<(), ScriptError>
{
    let table = || {
        prowess_table(
            2,
            prowler(),
            vec![grant("Gift", Keyword::Prowess), burn("Bolt", 1)],
        )
    };
    let mut driver = Driver::from_scenario(table())?;
    let target = driver.target_named("Prowler")?;
    // The Gift is cast before the creature has the second instance, so it makes one trigger.
    let first = driver.cast_at("Gift", &[target])?;
    assert_eq!(keyword_triggers(&driver, &first, "Prowler"), 1);
    resolve_everything(&mut driver)?;
    assert_eq!(size(&driver, "Prowler"), (2, 3));
    let foe = driver.target_player(1);
    let second = driver.cast_at("Bolt", &[foe])?;
    assert_eq!(
        keyword_triggers(&driver, &second, "Prowler"),
        2,
        "printed and given"
    );
    // Two instances of one keyword do the same in either order, so their controller is not asked to order them.
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    resolve_everything(&mut driver)?;
    assert_eq!(size(&driver, "Prowler"), (4, 5));
    assert_replays(&driver, &table().setup().unwrap())
}

/// Seat 0 attacks seat 1 on turn 2 with everything it controls: `attackers` and an Idler that stays home.
fn battle_table(seats: usize, attackers: Vec<Arc<CardDefinition>>) -> Scenario {
    Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            [
                attackers.into_iter().map(ready).collect::<Vec<_>>(),
                vec![ready(free_creature("Idler", 3, 3))],
            ]
            .concat(),
        )
}

fn warlord(name: &str) -> Arc<CardDefinition> {
    keyworded(name, 2, 2, &[Keyword::BattleCry])
}

#[rstest]
#[case::one_battle_cry(&["Warlord"], &[("Warlord", 2), ("Grunt", 2), ("Cub", 2)])]
#[case::two_battle_cries(&["Warlord", "Herald"], &[("Warlord", 3), ("Herald", 3), ("Grunt", 3), ("Cub", 3)])]
fn battle_cry_gives_each_other_attacker_plus_one_power_until_the_end_of_the_turn(
    #[values(2, 3, 4)] seats: usize,
    #[case] cries: &[&str],
    #[case] attacking: &[(&str, i32)],
) -> Result<(), ScriptError> {
    let creatures = || {
        let mut all: Vec<Arc<CardDefinition>> = cries.iter().map(|name| warlord(name)).collect();
        all.extend([free_creature("Grunt", 1, 1), free_creature("Cub", 1, 1)]);
        all
    };
    let setup = battle_table(seats, creatures()).setup().unwrap();
    let mut driver = Driver::from_scenario(battle_table(seats, creatures()))?;
    let names: Vec<(&str, usize)> = attacking.iter().map(|(name, _)| (*name, 1)).collect();
    let mut events = driver.attack(&names)?;
    // Two battle cries are two triggers of different sources: their controller orders them.
    if matches!(driver.decision()?.kind, DecisionKind::OrderTriggers(_)) {
        assert!(cries.len() > 1);
        events.extend(driver.order_triggers(&[0, 1])?);
    }
    events.extend(resolve_everything(&mut driver)?);
    for cry in cries {
        assert_eq!(keyword_triggers(&driver, &events, cry), 1, "{cry}");
    }
    for (name, power) in attacking {
        assert_eq!(
            size(&driver, name).0,
            *power,
            "{name} after the battle cries"
        );
    }
    assert_eq!(size(&driver, "Idler"), (3, 3), "it did not attack");
    let attacking_power: i32 = attacking.iter().map(|(_, power)| power).sum();
    driver.pass_until(Step::Main2)?;
    assert_eq!(life(&driver, 1), 20 - attacking_power);
    driver.idle_until(3, Step::Upkeep)?;
    assert_eq!(
        size(&driver, "Grunt"),
        (1, 1),
        "the pump ended with the turn"
    );
    assert_replays(&driver, &setup)
}

#[rstest]
#[case::tokens_first(&[0, 1], 7)]
#[case::battle_cry_first(&[1, 0], 5)]
fn the_order_of_heros_attack_triggers_decides_whether_the_soldiers_get_battle_cry(
    #[case] order: &[usize],
    #[case] damage: i32,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(card("Hero of Bladehold").unwrap())])
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.attack(&[("Hero of Bladehold", 1)])?;
    // Two triggers of one source and different abilities: the controller is asked, and told which is which.
    let words: Vec<String> = driver
        .trigger_options()?
        .triggers
        .iter()
        .map(|entry| entry.when.clone())
        .collect();
    assert_eq!(words, ["Battle cry: when it attacks", "When it attacks"]);
    driver.order_triggers(order)?;
    resolve_everything(&mut driver)?;
    driver.pass_until(Step::Main2)?;
    // Hero's three, and two Soldiers that deal 2 each when the battle cry came after them and 1 each when it did not.
    assert_eq!(life(&driver, 1), 20 - damage);
    assert_replays(&driver, &table().setup().unwrap())
}

/// Seat 1 is the active player in its first main phase with `hand` and `lands` Forests and a Mountain, and seat 0
/// controls a Warded 2/1 with ward {2}.
fn ward_table(seats: usize, lands: usize, hand: Vec<Arc<CardDefinition>>) -> Scenario {
    let mut table = Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(0, [ready(keyworded("Warded", 2, 1, &[ward("{2}")]))])
        .battlefield(
            1,
            [
                vec![ready(mountain())],
                vec![ready(forest()); lands],
                vec![ready(pinger("Pinger", 0))],
            ]
            .concat(),
        )
        .hand(1, hand);
    for seat in 2..seats {
        table = table.battlefield(seat, [ready(free_creature("Bystander", 1, 1))]);
    }
    table
}

fn yes_no(yes: bool) -> Answer {
    Answer::YesNo(YesNoChoice(yes))
}

/// The question the table waits on: who answers it and what is asked.
fn asked(driver: &Driver) -> (usize, String) {
    let decision = driver.decision().unwrap();
    let text = match &decision.kind {
        DecisionKind::YesNo(options) => options.text.clone(),
        other => other.name().to_owned(),
    };
    (decision.player.index(), text)
}

fn alive(driver: &Driver, name: &str) -> bool {
    driver.id_of(name).is_ok()
}

fn countered(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::ResolutionEnded {
                    outcome: ResolutionOutcome::Countered,
                    ..
                }
            )
        })
        .count()
}

/// How the opponent answers the ward: pays, declines, or has too little mana to be asked.
#[derive(Debug, Clone, Copy)]
enum Reply {
    Pays,
    Declines,
    Cannot,
}

#[rstest]
fn ward_counters_a_spell_unless_its_controller_pays(
    #[values(2, 3, 4)] seats: usize,
    #[values(Reply::Pays, Reply::Declines, Reply::Cannot)] reply: Reply,
) -> Result<(), ScriptError> {
    let hand = vec![burn("Bolt", 3)];
    let lands = if matches!(reply, Reply::Cannot) { 1 } else { 2 };
    let setup = ward_table(seats, lands, hand.clone()).setup().unwrap();
    let mut driver = Driver::from_scenario(ward_table(seats, lands, hand))?;
    let warded = driver.target_named("Warded")?;
    let mut events = driver.cast_at("Bolt", &[warded])?;
    assert_eq!(keyword_triggers(&driver, &events, "Warded"), 1);
    // The ward trigger is above the Bolt, and every seat may answer both.
    assert_eq!(driver.state().stack().len(), 2);
    events.extend(resolve_everything(&mut driver)?);
    match reply {
        Reply::Cannot => {
            assert!(
                driver.state().stack().is_empty(),
                "nobody is asked to pay what they cannot"
            );
        }
        Reply::Pays | Reply::Declines => {
            // Only the controller of the Bolt is asked, though it is not their ward and others are opponents too.
            assert_eq!(asked(&driver), (1, "Pay {2}?".to_owned()));
            events.extend(driver.answer(yes_no(matches!(reply, Reply::Pays)))?);
            if matches!(reply, Reply::Pays) {
                assert_eq!(asked(&driver).1, "pay-mana");
                events.extend(driver.answer(Answer::PayMana(PayChoice::Auto))?);
            }
            events.extend(resolve_everything(&mut driver)?);
        }
    }
    let paid = matches!(reply, Reply::Pays);
    assert_eq!(
        alive(&driver, "Warded"),
        !paid,
        "the Bolt resolves only when the ward was paid"
    );
    assert_eq!(countered(&events), usize::from(!paid), "{reply:?}");
    let in_graveyard = driver.state().zone(Zone::Graveyard(pid(1))).len();
    assert_eq!(
        in_graveyard, 1,
        "the Bolt went to its owner's graveyard either way"
    );
    assert_replays(&driver, &setup)
}

#[rstest]
fn ward_counters_an_activated_ability_unless_its_controller_pays(
    #[values(2, 4)] seats: usize,
    #[values(true, false)] pays: bool,
) -> Result<(), ScriptError> {
    let setup = ward_table(seats, 2, Vec::new()).setup().unwrap();
    let mut driver = Driver::from_scenario(ward_table(seats, 2, Vec::new()))?;
    let warded = driver.target_named("Warded")?;
    driver.begin_activation("Pinger")?;
    let mut events = driver.choose_targets(&[warded])?;
    assert_eq!(keyword_triggers(&driver, &events, "Warded"), 1);
    events.extend(resolve_everything(&mut driver)?);
    assert_eq!(asked(&driver), (1, "Pay {2}?".to_owned()));
    events.extend(driver.answer(yes_no(pays))?);
    if pays {
        events.extend(driver.answer(Answer::PayMana(PayChoice::Auto))?);
    }
    events.extend(resolve_everything(&mut driver)?);
    // The ability on the stack was countered: it ceased to exist and dealt nothing, or it resolved and killed the 2/1.
    assert_eq!(countered(&events), usize::from(!pays));
    assert_eq!(alive(&driver, "Warded"), !pays);
    assert_eq!(driver.state().stack().len(), 0);
    assert_replays(&driver, &setup)
}

#[test]
fn the_controller_of_the_warded_creature_owes_nothing_for_its_own_spells() -> Result<(), ScriptError>
{
    let table = || {
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .battlefield(
                0,
                [
                    ready(keyworded("Warded", 2, 1, &[ward("{2}")])),
                    ready(mountain()),
                ],
            )
            .hand(0, [burn("Bolt", 3)])
    };
    let mut driver = Driver::from_scenario(table())?;
    let warded = driver.target_named("Warded")?;
    let events = driver.cast_at("Bolt", &[warded])?;
    assert_eq!(keyword_triggers(&driver, &events, "Warded"), 0);
    resolve_everything(&mut driver)?;
    assert!(!alive(&driver, "Warded"));
    assert_replays(&driver, &table().setup().unwrap())
}

#[test]
fn a_cast_that_is_backed_out_of_targets_nothing() -> Result<(), ScriptError> {
    let hand = vec![burn("Bolt", 3)];
    let setup = ward_table(2, 2, hand.clone()).setup().unwrap();
    let mut driver = Driver::from_scenario(ward_table(2, 2, hand))?;
    let before = driver.state().state_hash();
    let warded = driver.target_named("Warded")?;
    driver.begin_cast("Bolt")?;
    driver.choose_targets(&[warded])?;
    let events = driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    assert_eq!(keyword_triggers(&driver, &events, "Warded"), 0);
    assert_eq!(driver.state().stack().len(), 0);
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(1))).len(),
        1,
        "the Bolt is still in the hand"
    );
    assert_eq!(
        driver.state().state_hash(),
        before,
        "backing out changes nothing"
    );
    assert_replays(&driver, &setup)
}

#[rstest]
fn a_ward_a_static_ability_gives_triggers_as_a_printed_one_does(
    #[values(2, 3)] seats: usize,
    #[values(true, false)] pays: bool,
) -> Result<(), ScriptError> {
    let lord = || {
        creature_with(
            "Warden",
            vec![Ability::Static {
                affected: Ref::Each(Selector(vec![Pred::Type(CardType::Creature), Pred::Other])),
                mods: vec![Modification::AddKeyword(ward("{2}"))],
                condition: None,
                defining: false,
                from: ZoneKind::Battlefield,
            }],
        )
    };
    let table = || {
        let mut table = Scenario::new(seats)
            .at(1, 1, Step::Main1)
            .battlefield(0, [ready(lord()), ready(free_creature("Bear", 2, 1))])
            .battlefield(1, [ready(mountain()), ready(forest()), ready(forest())])
            .hand(1, [burn("Bolt", 3)]);
        for seat in 2..seats {
            table = table.battlefield(seat, [ready(free_creature("Bystander", 1, 1))]);
        }
        table
    };
    let setup = table().setup().unwrap();
    let mut driver = Driver::from_scenario(table())?;
    let bear = driver.target_named("Bear")?;
    let mut events = driver.cast_at("Bolt", &[bear])?;
    // The Warden does not give itself the ward, and the trigger is named by the static ability that gave it.
    assert_eq!(keyword_triggers(&driver, &events, "Bear"), 1);
    assert_eq!(keyword_triggers(&driver, &events, "Warden"), 0);
    events.extend(resolve_everything(&mut driver)?);
    assert_eq!(asked(&driver), (1, "Pay {2}?".to_owned()));
    events.extend(driver.answer(yes_no(pays))?);
    if pays {
        events.extend(driver.answer(Answer::PayMana(PayChoice::Auto))?);
    }
    resolve_everything(&mut driver)?;
    assert_eq!(alive(&driver, "Bear"), !pays);
    assert_replays(&driver, &setup)
}
