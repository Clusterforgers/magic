//! Goal: at 2 to 4 seats, Chief Montgomery Scott puts a creature or Spacecraft card from his controller's hand onto the
//! battlefield with as many +1/+1 counters (a creature) or charge counters (a Spacecraft) as differently named lands they
//! control, and nothing when they decline; Sulu crews a Vehicle and stations a Spacecraft as though his power were
//! tripled, while he saddles a Mount with his power alone; Võx gives undaunted to the next artifact creature spell and
//! the next Spacecraft spell of the turn, each once; Selenia's death curses an opponent, who alone then loses twice the
//! life; Mirror Universe Intrusion copies one target creature with menace, or every creature of a target opponent when
//! kicked; The Muckslinger Gang grows the other creatures not in combat and the creature cards outside the battlefield;
//! Captain Benjamin Sisko casts the exiled spells whose mana value the Officers reach; Gwyndala equips her Heirloom and
//! turns her Equipment tokens into copies; Infinite Diversity casts one copy for each creature type of the non-Borg
//! creatures; The Weaver King takes a creature card each opponent lost from the library this turn; each log replays to
//! the live state. Kira Nerys makes a creature entering during an attack enter tapped and attacking what its controller
//! chooses.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::Duration;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::characteristics::{characteristics, controller_of};
use mtg_engine::choose::{
    CardsChoice, ChoiceOption, CostsChoice, OptionChoice, Target, TargetChoice, YesNoChoice,
};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice};
use mtg_engine::effects::Modification;
use mtg_engine::keywords::keywords_of;
use mtg_engine::object::ObjectKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{
    bear, burn, cost, forest, free_creature, island, keyworded, mountain, plains, swamp,
};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers until the stack and the pending triggers are empty: `pick` answers the decisions it knows, the rest are
/// answered passively.
fn r122_resolve(
    driver: &mut Driver,
    pick: impl Fn(&Decision) -> Option<Answer>,
) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let busy = !driver.state().stack().is_empty()
            || !driver.state().pending_triggers().is_empty()
            || driver.state().resolving().is_some();
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
fn r122_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// The object of the named card: the one on the battlefield first, else the first in any other zone.
fn r122_id(driver: &Driver, name: &str) -> Result<mtg_core::ids::ObjectId, ScriptError> {
    let state = driver.state();
    let named: Vec<_> = state
        .objects()
        .filter(|object| object.card().name == name)
        .collect();
    named
        .iter()
        .find(|object| object.zone() == Zone::Battlefield)
        .or_else(|| named.first())
        .map(|object| object.id())
        .ok_or_else(|| ScriptError::NoSuchCard {
            name: name.to_owned(),
            place: "any zone",
        })
}

/// The counters of a kind on the named permanent.
fn r122_counters(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = r122_id(&driver, name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .map_or(0, |permanent| permanent.counters_of(kind))
}

/// Whether the named card is on the battlefield.
fn r122_on_battlefield(driver: &Driver, name: &str) -> bool {
    let id = r122_id(&driver, name).unwrap();
    driver.state().zone(Zone::Battlefield).contains(&id)
}

// Chief Montgomery Scott

#[rstest]
#[case::a_creature_with_five_names("Grizzly Bears", 5, true)]
#[case::a_creature_with_two_names("Grizzly Bears", 2, true)]
#[case::a_spacecraft_with_five_names("Galvanizing Sawship", 5, true)]
#[case::a_spacecraft_with_one_name("Galvanizing Sawship", 1, true)]
#[case::declined("Grizzly Bears", 5, false)]
fn r122_scotty_puts_a_card_in_with_counters_by_its_type(
    #[values(2, 3, 4)] seats: usize,
    #[case] put: &str,
    #[case] names: usize,
    #[case] accept: bool,
) -> Result<(), ScriptError> {
    let kinds = [island, forest, plains, swamp, mountain];
    // Six lands: `names` of them differently named, the rest more Islands.
    let lands: Vec<_> = (0..6)
        .map(|at| ready(kinds[if at < names { at } else { 0 }]()))
        .collect();
    let scenario = Scenario::new(seats)
        .hand(0, [bear(), card("Galvanizing Sawship").unwrap()])
        .battlefield(
            0,
            std::iter::once(ready(card("Chief Montgomery Scott").unwrap())).chain(lands),
        )
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let chosen = r122_id(&driver, put)?;
    driver.begin_activation("Chief Montgomery Scott")?;
    r122_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        DecisionKind::YesNo(_) => Some(Answer::YesNo(YesNoChoice(accept))),
        DecisionKind::ChooseCards(options) if options.from.contains(&chosen) => {
            Some(Answer::ChooseCards(CardsChoice(vec![chosen])))
        }
        _ => None,
    })?;
    let x = u32::try_from(names).unwrap();
    assert_eq!(r122_on_battlefield(&driver, put), accept);
    let creature = put == "Grizzly Bears";
    let (plus, charge) = match (accept, creature) {
        (false, _) => (0, 0),
        (true, true) => (x, 0),
        (true, false) => (0, x),
    };
    assert_eq!(
        r122_counters(&driver, put, &CounterKind::PlusOnePlusOne),
        plus
    );
    assert_eq!(r122_counters(&driver, put, &CounterKind::Charge), charge);
    // The other card stays in hand.
    let other = if creature {
        "Galvanizing Sawship"
    } else {
        "Grizzly Bears"
    };
    assert!(!r122_on_battlefield(&driver, other));
    r122_replays(&driver);
    Ok(())
}

// Sulu, Heroic Helmsman

/// A free card with one keyword: a 4/4 artifact Vehicle with crew `power`, or a 4/4 creature Mount with saddle `power`.
fn r122_ride(vehicle: bool, power: u32) -> Arc<CardDefinition> {
    let (keyword, name) = if vehicle {
        (format!(r#"{{"Keyword":{{"Crew":{power}}}}}"#), "Skiff")
    } else {
        (format!(r#"{{"Keyword":{{"Saddle":{power}}}}}"#), "Steed")
    };
    let ability: Ability = serde_json::from_str(&keyword).unwrap();
    let card = CardDefinition::new(name)
        .with_power_toughness(4, 4)
        .with_rules(ability);
    Arc::new(if vehicle {
        card.with_subtype("Vehicle").with_type(CardType::Artifact)
    } else {
        card.with_subtype("Mount").with_type(CardType::Creature)
    })
}

#[rstest]
#[case::crew_six(true, 6, true)]
#[case::crew_seven(true, 7, false)]
#[case::saddle_two(false, 2, true)]
#[case::saddle_three(false, 3, false)]
fn r122_sulu_crews_as_though_tripled_and_saddles_with_his_power(
    #[values(2, 3, 4)] seats: usize,
    #[case] vehicle: bool,
    #[case] power: u32,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(card("Sulu, Heroic Helmsman").unwrap()),
                ready(r122_ride(vehicle, power)),
            ],
        )
        .at(2, 0, Step::Main1);
    let driver = Driver::from_scenario(scenario)?;
    let name = if vehicle { "Skiff" } else { "Steed" };
    assert_eq!(!driver.offered_abilities(name)?.is_empty(), offered);
    Ok(())
}

#[rstest]
#[case::sulu("Sulu, Heroic Helmsman", 6)]
#[case::a_bear_of_the_same_power("Bear", 2)]
fn r122_sulu_stations_as_though_his_power_were_tripled(
    #[values(2, 3, 4)] seats: usize,
    #[case] tapped: &str,
    #[case] charge: u32,
) -> Result<(), ScriptError> {
    let crewman = if tapped == "Bear" {
        free_creature("Bear", 2, 2)
    } else {
        card(tapped).unwrap()
    };
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [ready(card("Galvanizing Sawship").unwrap()), ready(crewman)],
        )
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let id = r122_id(&driver, tapped)?;
    driver.begin_activation("Galvanizing Sawship")?;
    r122_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        DecisionKind::ChooseCards(options) if options.from.contains(&id) => {
            Some(Answer::ChooseCards(CardsChoice(vec![id])))
        }
        _ => None,
    })?;
    assert_eq!(
        r122_counters(&driver, "Galvanizing Sawship", &CounterKind::Charge),
        charge
    );
    r122_replays(&driver);
    Ok(())
}

// Võx, Child of the Collective

/// A colorless card of these types costing `{generic}`, a 2/2 when it is a creature.
fn r122_spell(name: &str, types: &[CardType], generic: u32) -> Arc<CardDefinition> {
    let card = types.iter().fold(
        CardDefinition::new(name).with_mana_cost(cost(generic, &[])),
        |card, kind| card.with_type(*kind),
    );
    let card = if types.contains(&CardType::Creature) {
        card.with_power_toughness(2, 2)
    } else {
        card
    };
    Arc::new(card)
}

/// The untapped lands seat 0 controls.
fn r122_untapped_lands(driver: &Driver) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            let object = state.object(**id).unwrap();
            object.card().types.contains(&CardType::Land)
                && controller_of(state, **id) == Some(pid(0))
                && !object.permanent().unwrap().tapped()
        })
        .count()
}

/// Casts the named spell paying automatically and lets it resolve; the mana it took.
fn r122_cast_paid(driver: &mut Driver, name: &str) -> Result<usize, ScriptError> {
    let before = r122_untapped_lands(driver);
    driver.cast(name)?;
    r122_resolve(driver, |_| None)?;
    Ok(before - r122_untapped_lands(driver))
}

#[rstest]
fn r122_vox_gives_undaunted_to_the_next_spell_of_each_kind(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] cast: bool,
) -> Result<(), ScriptError> {
    let artifact_creature = [CardType::Artifact, CardType::Creature];
    let vox = card("Võx, Child of the Collective").unwrap();
    let spells = [
        r122_spell("Ogre", &[CardType::Creature], 4),
        r122_spell("Golem", &artifact_creature, 4),
        r122_spell("Second Golem", &artifact_creature, 4),
        card("Galvanizing Sawship").unwrap(),
    ];
    let scenario = Scenario::new(seats).library(0, (0..5).map(|_| mountain()));
    // It enters as it is cast in the main phase, or it waits on the battlefield for the first main phase.
    let mut permanents: Vec<_> = (0..18)
        .map(|_| ready(mountain()))
        .chain((0..5).map(|_| ready(island())))
        .collect();
    let scenario = if cast {
        scenario
            .hand(0, spells.into_iter().chain([vox]))
            .at(2, 0, Step::Main1)
    } else {
        permanents.push(ready(vox));
        scenario.hand(0, spells).at(2, 0, Step::Upkeep)
    };
    let scenario = scenario.battlefield(0, permanents);
    let mut driver = Driver::from_scenario(scenario)?;
    if cast {
        assert_eq!(
            r122_cast_paid(&mut driver, "Võx, Child of the Collective")?,
            5
        );
    } else {
        driver.pass_until(Step::Main1)?;
        r122_resolve(&mut driver, |_| None)?;
    }
    let opponents = seats - 1;
    // A creature that is no artifact pays its whole cost and leaves the grant for the artifact creature.
    assert_eq!(r122_cast_paid(&mut driver, "Ogre")?, 4);
    assert_eq!(r122_cast_paid(&mut driver, "Golem")?, 4 - opponents);
    assert_eq!(r122_cast_paid(&mut driver, "Second Golem")?, 4);
    assert_eq!(
        r122_cast_paid(&mut driver, "Galvanizing Sawship")?,
        6 - opponents
    );
    r122_replays(&driver);
    Ok(())
}

// Selenia, the Cursed Heart

#[rstest]
fn r122_selenia_curses_an_opponent_who_alone_loses_twice_the_life(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let victim = seats - 1;
    let scenario = Scenario::new(seats)
        .hand(0, [burn("Shock", 2), burn("Second Shock", 2)])
        .battlefield(
            0,
            [
                ready(card("Selenia, the Cursed Heart").unwrap()),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let selenia = r122_id(&driver, "Selenia, the Cursed Heart")?;
    let shrink = Modification::ModifyPowerToughness {
        power: -10,
        toughness: -10,
    };
    driver.add_effect(driver.on(selenia), shrink, Duration::UntilEndOfTurn)?;
    r122_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::ChooseTargets(_) => Some(Answer::ChooseTargets(TargetChoice::Targets(vec![
            Target::Player(pid(victim)),
        ]))),
        _ => None,
    })?;
    let curse = r122_id(&driver, "Selenia's Curse")?;
    let state = driver.state();
    assert!(state.zone(Zone::Battlefield).contains(&curse));
    assert_eq!(controller_of(state, curse), Some(pid(0)));
    assert_eq!(
        state
            .object(curse)
            .unwrap()
            .permanent()
            .unwrap()
            .attached_player(),
        Some(pid(victim))
    );
    let life = |driver: &Driver, seat: usize| driver.state().player(pid(seat)).unwrap().life();
    let before = life(&driver, victim);
    driver.cast_at("Shock", &[Target::Player(pid(victim))])?;
    r122_resolve(&mut driver, |_| None)?;
    assert_eq!(before - life(&driver, victim), 4);
    if seats > 2 {
        let before = life(&driver, 1);
        driver.cast_at("Second Shock", &[Target::Player(pid(1))])?;
        r122_resolve(&mut driver, |_| None)?;
        assert_eq!(before - life(&driver, 1), 2);
    }
    r122_replays(&driver);
    Ok(())
}

// Mirror Universe Intrusion

/// The names of the creature tokens seat 0 controls that have menace, sorted.
fn r122_menace_tokens(driver: &Driver) -> Vec<String> {
    let state = driver.state();
    let mut names: Vec<String> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            let object = state.object(**id).unwrap();
            object.kind() == ObjectKind::Token
                && controller_of(state, **id) == Some(pid(0))
                && keywords_of(state, **id).contains(&&Keyword::Menace)
        })
        .map(|id| {
            let object = state.object(*id).unwrap();
            characteristics(state, *id)
                .unwrap()
                .name_for(&object.card().name)
        })
        .collect();
    names.sort();
    names
}

#[rstest]
fn r122_mirror_universe_intrusion_copies_one_creature_or_an_opponents_all(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] kicked: bool,
) -> Result<(), ScriptError> {
    let victim = seats - 1;
    let mut scenario = Scenario::new(seats)
        .hand(0, [card("Mirror Universe Intrusion").unwrap()])
        .battlefield(
            0,
            (0..5)
                .map(|_| ready(island()))
                .chain((0..4).map(|_| ready(swamp()))),
        )
        .battlefield(
            victim,
            [
                ready(free_creature("Ogre", 3, 3)),
                ready(free_creature("Elf", 1, 1)),
            ],
        )
        .at(2, 0, Step::Main1);
    if seats > 2 {
        scenario = scenario.battlefield(1, [ready(free_creature("Troll", 4, 4))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let ogre = driver.target_named("Ogre")?;
    driver.begin_cast("Mirror Universe Intrusion")?;
    r122_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::ChooseCosts(_) => Some(Answer::ChooseCosts(CostsChoice::Costs(if kicked {
            vec![0]
        } else {
            vec![]
        }))),
        DecisionKind::ChooseTargets(_) => {
            let target = if kicked {
                Target::Player(pid(victim))
            } else {
                ogre.clone()
            };
            Some(Answer::ChooseTargets(TargetChoice::Targets(vec![target])))
        }
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        _ => None,
    })?;
    let expected: &[&str] = if kicked { &["Elf", "Ogre"] } else { &["Ogre"] };
    assert_eq!(r122_menace_tokens(&driver), expected);
    r122_replays(&driver);
    Ok(())
}

// The Muckslinger Gang

/// The power and toughness of the named object.
fn r122_size(driver: &Driver, name: &str) -> (i32, i32) {
    let found = characteristics(driver.state(), r122_id(&driver, name).unwrap()).unwrap();
    (found.power.unwrap(), found.toughness.unwrap())
}

#[rstest]
fn r122_muckslinger_gang_grows_the_others_out_of_combat_and_the_cards_elsewhere(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(card("The Muckslinger Gang").unwrap()),
                ready(free_creature("Ogre", 3, 3)),
                ready(free_creature("Elf", 1, 1)),
            ],
        )
        .battlefield(1, [ready(free_creature("Troll", 4, 4))])
        .hand(0, [bear()])
        .hand(1, [free_creature("Rival", 2, 2)])
        .graveyard(0, [free_creature("Ghoul", 2, 2)])
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(r122_size(&driver, "Ogre"), (6, 6));
    assert_eq!(r122_size(&driver, "Elf"), (4, 4));
    assert_eq!(r122_size(&driver, "The Muckslinger Gang"), (3, 3));
    assert_eq!(r122_size(&driver, "Troll"), (4, 4));
    assert_eq!(r122_size(&driver, "Grizzly Bears"), (5, 5));
    assert_eq!(r122_size(&driver, "Ghoul"), (5, 5));
    assert_eq!(r122_size(&driver, "Rival"), (2, 2));
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Ogre", 1)])?;
    assert_eq!(r122_size(&driver, "Ogre"), (3, 3));
    assert_eq!(r122_size(&driver, "Elf"), (4, 4));
    r122_replays(&driver);
    Ok(())
}

// Captain Benjamin Sisko

/// A free creature with one creature type.
fn r122_typed(name: &str, subtype: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(subtype)
            .with_power_toughness(1, 1)
            .with_mana_cost(cost(0, &[])),
    )
}

/// The zone of the named card.
fn r122_zone(driver: &Driver, name: &str) -> Zone {
    driver
        .state()
        .object(r122_id(&driver, name).unwrap())
        .unwrap()
        .zone()
}

#[rstest]
#[case::no_officer(0)]
#[case::two_officers(2)]
#[case::three_officers(3)]
fn r122_sisko_casts_the_exiled_spells_up_to_the_number_of_officers(
    #[values(2, 3, 4)] seats: usize,
    #[case] officers: usize,
) -> Result<(), ScriptError> {
    let names = ["One", "Two", "Three", "Four", "Five"];
    let library: Vec<_> = names
        .iter()
        .zip(1u32..)
        .map(|(name, value)| r122_spell(name, &[CardType::Creature], value))
        .collect();
    let lands = [
        plains(),
        island(),
        swamp(),
        mountain(),
        forest(),
        forest(),
        forest(),
    ];
    let scenario = Scenario::new(seats)
        .library(0, library)
        .battlefield(
            0,
            std::iter::once(ready(card("Captain Benjamin Sisko").unwrap()))
                .chain(lands.into_iter().map(ready))
                .chain(
                    (0..officers).map(|at| ready(r122_typed(&format!("Officer {at}"), "Officer"))),
                ),
        )
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Captain Benjamin Sisko")?;
    r122_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        DecisionKind::YesNo(_) => Some(Answer::YesNo(YesNoChoice(true))),
        _ => None,
    })?;
    for (name, value) in names.iter().zip(1usize..) {
        // Sisko is an Officer himself.
        let expected = if value <= officers + 1 {
            Zone::Battlefield
        } else {
            Zone::Exile
        };
        assert_eq!(r122_zone(&driver, name), expected, "{name}");
    }
    r122_replays(&driver);
    Ok(())
}

// Gwyndala, the Progeny

#[rstest]
fn r122_gwyndala_equips_her_heirloom_and_turns_the_tokens_into_copies(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let blade: Ability = serde_json::from_str(
        r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":3,"toughness":0}}]}}"#,
    )
    .unwrap();
    let blade = Arc::new(
        CardDefinition::new("Blade")
            .with_type(CardType::Artifact)
            .with_subtype("Equipment")
            .with_mana_cost(cost(0, &[]))
            .with_rules(blade),
    );
    let scenario = Scenario::new(seats)
        .hand(0, [card("Gwyndala, the Progeny").unwrap()])
        .battlefield(
            0,
            std::iter::once(ready(blade)).chain((0..4).map(|_| ready(island()))),
        )
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let blade = r122_id(&driver, "Blade")?;
    driver.cast("Gwyndala, the Progeny")?;
    r122_resolve(&mut driver, |_| None)?;
    let gwyndala = r122_id(&driver, "Gwyndala, the Progeny")?;
    let heirloom = r122_id(&driver, "Neuroflux Heirloom")?;
    let state = driver.state();
    assert!(state.object(heirloom).unwrap().kind() == ObjectKind::Token);
    assert_eq!(
        state
            .object(heirloom)
            .unwrap()
            .permanent()
            .unwrap()
            .attached_to()
            .map(|host| host.id),
        Some(gwyndala)
    );
    assert_eq!(r122_size(&driver, "Gwyndala, the Progeny"), (2, 2));
    let target = Target::Object(driver.state().object(blade).unwrap().reference());
    driver.begin_activation("Gwyndala, the Progeny")?;
    r122_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        DecisionKind::ChooseTargets(_) => Some(Answer::ChooseTargets(TargetChoice::Targets(vec![
            target.clone(),
        ]))),
        _ => None,
    })?;
    // The token is a Blade now, still on Gwyndala: she gets +3/+0 instead of +1/+1.
    assert_eq!(
        characteristics(driver.state(), heirloom)
            .unwrap()
            .name_for(&driver.state().object(heirloom).unwrap().card().name),
        "Blade"
    );
    assert_eq!(r122_size(&driver, "Gwyndala, the Progeny"), (4, 1));
    r122_replays(&driver);
    Ok(())
}

// Infinite Diversity

#[rstest]
#[case::one_type(&[("Elf", "Elf")], 1)]
#[case::two_types(&[("Elf", "Elf"), ("Goblin", "Goblin")], 2)]
#[case::a_borg_not_counted(&[("Elf", "Elf"), ("Goblin", "Goblin"), ("Drone", "Borg")], 2)]
fn r122_infinite_diversity_casts_a_copy_for_each_creature_type(
    #[values(2, 3, 4)] seats: usize,
    #[case] creatures: &[(&str, &str)],
    #[case] copies: i32,
) -> Result<(), ScriptError> {
    let gift: Ability =
        serde_json::from_str(r#"{"Spell":{"effects":[{"GainLife":{"amount":1}}]}}"#).unwrap();
    let gift = Arc::new(
        CardDefinition::new("Gift")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(1, &[]))
            .with_rules(gift),
    );
    let scenario = Scenario::new(seats)
        .hand(0, [card("Infinite Diversity").unwrap()])
        .graveyard(0, [gift])
        .battlefield(
            0,
            (0..9).map(|_| ready(mountain())).chain(
                creatures
                    .iter()
                    .map(|(name, subtype)| ready(r122_typed(name, subtype))),
            ),
        )
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    let gift = driver.target_named("Gift").or_else(|_| {
        let id = r122_id(&driver, "Gift")?;
        Ok::<Target, ScriptError>(Target::Object(
            driver.state().object(id).unwrap().reference(),
        ))
    })?;
    driver.begin_cast("Infinite Diversity")?;
    r122_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::PayMana(_) => Some(Answer::PayMana(PayChoice::Auto)),
        DecisionKind::ChooseTargets(_) => Some(Answer::ChooseTargets(TargetChoice::Targets(vec![
            gift.clone(),
        ]))),
        DecisionKind::YesNo(_) => Some(Answer::YesNo(YesNoChoice(true))),
        _ => None,
    })?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life() - life, copies);
    assert_eq!(r122_zone(&driver, "Infinite Diversity"), Zone::Exile);
    assert_eq!(r122_zone(&driver, "Gift"), Zone::Exile);
    r122_replays(&driver);
    Ok(())
}

// Kira Nerys, Firebrand

/// What the named creature attacks, if it attacks.
fn r122_attacks(driver: &Driver, name: &str) -> Option<AttackTarget> {
    let id = r122_id(&driver, name).unwrap();
    driver
        .state()
        .combat()
        .attackers()
        .iter()
        .find(|attacker| attacker.object() == id)
        .map(mtg_engine::combat::Attacker::target)
}

#[rstest]
fn r122_kira_makes_a_creature_entering_in_combat_enter_tapped_and_attacking(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let victim = seats - 1;
    let flash = |name: &str| keyworded(name, 2, 2, &[Keyword::Flash]);
    let scenario = Scenario::new(seats)
        .hand(0, [flash("Early"), flash("Ambusher")])
        .battlefield(
            0,
            [
                ready(card("Kira Nerys, Firebrand").unwrap()),
                ready(free_creature("Raider", 2, 2)),
            ],
        )
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    // Before the attack nothing attacks: the creature enters untapped and only grows.
    driver.cast("Early")?;
    r122_resolve(&mut driver, |_| None)?;
    assert!(r122_attacks(&driver, "Early").is_none());
    assert!(
        !driver
            .state()
            .object(r122_id(&driver, "Early")?)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    assert_eq!(r122_size(&driver, "Early"), (3, 3));
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Raider", 1)])?;
    driver.cast("Ambusher")?;
    r122_resolve(&mut driver, |decision| match &decision.kind {
        DecisionKind::ChooseOption(options) => options
            .options
            .iter()
            .position(|option| *option == ChoiceOption::Player(pid(victim)))
            .map(|at| Answer::ChooseOption(OptionChoice(at))),
        _ => None,
    })?;
    let ambusher = r122_id(&driver, "Ambusher")?;
    assert!(
        driver
            .state()
            .object(ambusher)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    assert_eq!(
        r122_attacks(&driver, "Ambusher"),
        Some(AttackTarget::Player(pid(victim)))
    );
    assert_eq!(r122_size(&driver, "Ambusher"), (3, 3));
    r122_replays(&driver);
    Ok(())
}

// The Weaver King

#[rstest]
fn r122_weaver_king_takes_a_creature_each_opponent_lost_from_the_library_this_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(0, [ready(card("The Weaver King").unwrap())])
        .library(
            1,
            [free_creature("Zombie", 2, 2), island(), island(), island()],
        )
        .graveyard(1, [free_creature("Old Bones", 1, 1)])
        .at(2, 0, Step::Main1);
    if seats > 2 {
        scenario = scenario.graveyard(2, [free_creature("Ghoul", 2, 2)]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("The Weaver King", 1)])?;
    driver.idle_until(2, Step::EndCombat)?;
    let state = driver.state();
    let zombie = r122_id(&driver, "Zombie")?;
    assert_eq!(state.object(zombie).unwrap().zone(), Zone::Battlefield);
    assert_eq!(controller_of(state, zombie), Some(pid(0)));
    // Two islands milled with it stay; a creature card already in a graveyard stays too.
    assert_eq!(r122_zone(&driver, "Old Bones"), Zone::Graveyard(pid(1)));
    if seats > 2 {
        assert_eq!(r122_zone(&driver, "Ghoul"), Zone::Graveyard(pid(2)));
    }
    r122_replays(&driver);
    Ok(())
}
