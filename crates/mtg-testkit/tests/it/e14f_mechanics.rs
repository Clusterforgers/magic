//! Goal: pack e14f's mechanics play whole at tables of two, three and four seats: a suspected creature that loses all abilities can block again (its "can't block" is an ability, CR 701.60c); a haunt creature that dies is exiled haunting a creature and its ability works when that creature dies; a Case is solved at its controller's end step and its solved ability works from then on; a card revealed from the opening hand stays in hand and its trigger works at the first upkeep only; a permanent gets its counters as it turns face up; the legend rule stops for the permanents a selector picks only.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::RefCell;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, Supertype};
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{CardsChoice, Target, TargetChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind, MulliganChoice, PriorityAction};
use mtg_engine::format::Normal;
use mtg_engine::keywords::keywords_of;
use mtg_engine::setup::GameSetup;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, replay};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;
use crate::macro_keywords::{fight_on, play, table};

/// A card with the rules written as JSON.
fn card(name: &str, types: &[CardType], rules: &[&str]) -> Arc<CardDefinition> {
    let mut def = CardDefinition::new(name);
    for kind in types {
        def = def.with_type(*kind);
    }
    if types.contains(&CardType::Creature) {
        def = def.with_power_toughness(2, 2);
    }
    def.mana_cost = Some(mtg_core::mana::ManaCost::parse("{0}").unwrap());
    def.rules = rules
        .iter()
        .map(|json| Arc::new(serde_json::from_str::<Ability>(json).unwrap()))
        .collect();
    Arc::new(def)
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// A creature that suspects a target creature an opponent controls when it attacks.
fn accuser() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Suspect":{"what":{"Target":0}}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Accuser")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(ability),
    )
}

/// An enchantment whose static ability takes every ability from the creatures its controller's opponents control.
fn silence() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"mods":["LoseAbilities"]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Silence")
            .with_type(CardType::Enchantment)
            .with_rules(ability),
    )
}

/// The names of the objects.
fn names_of(driver: &Driver, ids: impl Iterator<Item = mtg_core::ids::ObjectId>) -> Vec<String> {
    ids.map(|id| driver.state().object(id).unwrap().card().name.clone())
        .collect()
}

#[rstest]
fn a_suspected_creature_without_abilities_can_block(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] silenced: bool,
) -> Result<(), ScriptError> {
    let mut mine = vec![accuser()];
    if silenced {
        mine.push(silence());
    }
    let scenario = table(
        seats,
        mine,
        &[free_creature("Wall", 0, 4), free_creature("Guard", 0, 4)],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let offered: RefCell<Option<Vec<String>>> = RefCell::new(None);
    let hook = |driver: &Driver, decision: &Decision| -> Option<Answer> {
        match &decision.kind {
            DecisionKind::ChooseTargets(options) => {
                let wall = options.legal.iter().find(|target| {
                    matches!(target, Target::Object(object)
                        if driver.state().object(object.id).is_some_and(|o| o.card().name == "Wall"))
                })?;
                Some(Answer::ChooseTargets(TargetChoice::Targets(vec![*wall])))
            }
            DecisionKind::DeclareBlockers(options) if decision.player == pid(1) => {
                let ids = options.blockers.iter().map(|option| option.object);
                let mut names = names_of(driver, ids);
                names.sort();
                offered.replace(Some(names));
                None
            }
            _ => None,
        }
    };
    play(&mut driver, &["Accuser"], &[], &hook, Step::EndCombat)?;
    let expected: &[&str] = if silenced {
        &["Guard", "Wall"]
    } else {
        &["Guard"]
    };
    assert_eq!(
        offered.take(),
        Some(expected.iter().map(|s| (*s).to_owned()).collect())
    );
    let wall = driver.id_of("Wall")?;
    let held = driver.state().object(wall).unwrap().permanent().unwrap();
    assert!(held.suspected());
    assert_eq!(
        keywords_of(driver.state(), wall).contains(&&Keyword::Menace),
        !silenced
    );
    Ok(())
}

const HAUNTED_GAIN: &str = r#"{"Triggered":{"from":"Exile","when":{"ZoneChange":{"what":[{"HauntedBy":"This"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"GainLife":{"amount":3}}]}}"#;
const SLAY: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#;

/// The object named `name` that lies in exile.
fn in_exile(driver: &Driver, name: &str) -> Option<ObjectId> {
    driver
        .state()
        .zone(Zone::Exile)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().card().name == name)
}

/// Seat 0 casts the named sorcery at the permanent named `at` and lets everything resolve.
fn slay(driver: &mut Driver, spell: &str, at: &str) -> Result<(), ScriptError> {
    let id = driver.id_of(at)?;
    let named = driver.state().object(id).unwrap().reference();
    driver.cast_at(spell, &[Target::Object(named)])?;
    fight_on(driver, Step::Main1)
}

#[rstest]
fn a_haunt_creature_haunts_a_creature_and_works_when_it_dies(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let spirit = card(
        "Spirit",
        &[CardType::Creature],
        &[r#"{"Keyword":"Haunt"}"#, HAUNTED_GAIN],
    );
    let scenario = Scenario::new(seats)
        .battlefield(0, vec![ready(spirit)])
        .battlefield(1, vec![ready(free_creature("Victim", 1, 1))])
        .hand(
            0,
            vec![
                card("Slay", &[CardType::Sorcery], &[SLAY]),
                card("Smite", &[CardType::Sorcery], &[SLAY]),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    let victim = driver.id_of("Victim")?;
    let haunted = driver.state().object(victim).unwrap().reference();
    slay(&mut driver, "Slay", "Spirit")?;
    let card = in_exile(&driver, "Spirit").expect("the card is exiled haunting");
    assert_eq!(
        driver.state().object(card).unwrap().haunting(),
        Some(haunted)
    );
    let before = life(&driver, 0);
    slay(&mut driver, "Smite", "Victim")?;
    assert_eq!(life(&driver, 0), before + 3, "the haunted creature died");
    let again = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(again.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn a_haunt_spell_haunts_after_resolving_and_works_only_for_its_creature(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let gift = card(
        "Gift",
        &[CardType::Sorcery],
        &[
            r#"{"Spell":{"effects":[{"GainLife":{"amount":1}}]}}"#,
            r#"{"Keyword":"Haunt"}"#,
            HAUNTED_GAIN,
        ],
    );
    let scenario = Scenario::new(seats)
        .battlefield(
            1,
            vec![
                ready(free_creature("Victim", 1, 1)),
                ready(free_creature("Bystander", 1, 1)),
            ],
        )
        .hand(
            0,
            vec![
                gift,
                card("Smite", &[CardType::Sorcery], &[SLAY]),
                card("Slay", &[CardType::Sorcery], &[SLAY]),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    let start = life(&driver, 0);
    driver.cast("Gift")?;
    fight_on(&mut driver, Step::Main1)?;
    let card = in_exile(&driver, "Gift").expect("the spell card is exiled haunting");
    let haunted = driver.state().object(card).unwrap().haunting().unwrap();
    let haunted = driver
        .state()
        .object(haunted.id)
        .unwrap()
        .card()
        .name
        .clone();
    let other = if haunted == "Victim" {
        "Bystander"
    } else {
        "Victim"
    };
    slay(&mut driver, "Smite", other)?;
    assert_eq!(life(&driver, 0), start + 1, "another creature died");
    slay(&mut driver, "Slay", &haunted)?;
    assert_eq!(life(&driver, 0), start + 4, "the haunted creature died");
    Ok(())
}

#[rstest]
fn a_case_is_solved_at_the_end_step_and_its_solved_ability_works(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let case = card(
        "Case",
        &[CardType::Enchantment],
        &[
            r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"if":{"Compare":{"left":{"Count":["This",{"Not":"Solved"}]},"op":"Ge","right":1}},"effects":[{"Solve":{"what":"This"}}]}}"#,
            r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":0}}],"condition":{"Compare":{"left":{"Count":["This","Solved"]},"op":"Ge","right":1}}}}"#,
        ],
    );
    let scenario = (0..seats).fold(
        Scenario::new(seats).battlefield(0, vec![ready(case), ready(free_creature("Bear", 2, 2))]),
        |scenario, seat| scenario.library(seat, vec![forest(); 12]),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let bear = driver.id_of("Bear")?;
    let id = driver.id_of("Case")?;
    let power = |driver: &Driver| characteristics(driver.state(), bear).unwrap().power;
    assert_eq!(power(&driver), Some(2));
    driver.idle_until(2, Step::Upkeep)?;
    let held = driver.state().object(id).unwrap().permanent().unwrap();
    assert!(held.solved());
    assert_eq!(power(&driver), Some(3));
    Ok(())
}

/// A creature that may be revealed from the opening hand to gain 3 life at the beginning of the first upkeep.
fn seer() -> Arc<CardDefinition> {
    card(
        "Seer",
        &[CardType::Creature],
        &[
            r#"{"OpeningReveal":{"when":{"StepBegins":{"step":"Upkeep"}},"effects":[{"GainLife":{"amount":3}}]}}"#,
        ],
    )
}

#[rstest]
fn a_card_revealed_from_the_opening_hand_works_at_the_first_upkeep_only(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] revealed: usize,
) -> Result<(), ScriptError> {
    let decks = (0..seats)
        .map(|seat| {
            (0..40)
                .map(|_| if seat == 0 { seer() } else { forest() })
                .collect()
        })
        .collect();
    let mut setup = GameSetup::new(Arc::new(Normal), 7, decks);
    setup.validate_decks = false;
    let mut driver = Driver::new(Engine::new(setup)?)?;
    while matches!(driver.decision()?.kind, DecisionKind::Mulligan(_)) {
        driver.answer(Answer::Mulligan(MulliganChoice::Keep))?;
    }
    let offered = match &driver.decision()?.kind {
        DecisionKind::ChooseCards(options) => options.from.clone(),
        other => panic!("the reveal is asked: {other:?}"),
    };
    let chosen: Vec<ObjectId> = offered.iter().copied().take(revealed).collect();
    driver.answer(Answer::ChooseCards(CardsChoice(chosen.clone())))?;
    for card in &chosen {
        let zone = driver.state().object(*card).unwrap().zone();
        assert_eq!(zone, Zone::Hand(pid(0)));
    }
    let start = life(&driver, 0);
    let gained = i32::try_from(revealed).unwrap() * 3;
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(life(&driver, 0), start + gained);
    driver.idle_until(3, Step::Draw)?;
    assert_eq!(life(&driver, 0), start + gained, "once only");
    Ok(())
}

#[rstest]
fn a_permanent_gets_its_counters_as_it_turns_face_up(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let hydra = card(
        "Hydra",
        &[CardType::Creature],
        &[
            r#"{"Keyword":{"Morph":[{"PayLife":1}]}}"#,
            r#"{"Replacement":{"when":{"TurnedFaceUp":{"what":["This"]}},"by":{"WithCounters":{"counter":"+1/+1","count":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},"Other"]}}}}}"#,
        ],
    );
    let mut mine: Vec<_> = (0..3).map(|_| ready(forest())).collect();
    mine.push(ready(free_creature("Bear", 2, 2)));
    mine.push(ready(free_creature("Elk", 2, 2)));
    let scenario = Scenario::new(seats)
        .hand(0, vec![hydra])
        .battlefield(0, mine);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Hydra")?;
    if let DecisionKind::ChooseOption(_) = driver.decision()?.kind {
        crate::common::casting::choose_way(&mut driver, "Cast face down")?;
    }
    driver.pay_tapping(&["Forest", "Forest", "Forest"], 0)?;
    fight_on(&mut driver, Step::Main1)?;
    let id = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().face_down().is_some())
        .expect("a face-down permanent");
    let counters = |driver: &Driver| {
        let object = driver.state().object(id).unwrap();
        object
            .permanent()
            .unwrap()
            .counters_of(&CounterKind::PlusOnePlusOne)
    };
    assert_eq!(
        counters(&driver),
        0,
        "a face-down creature has no abilities"
    );
    driver.answer(Answer::Priority(PriorityAction::TurnFaceUp(id)))?;
    assert_eq!(driver.state().object(id).unwrap().face_down(), None);
    assert_eq!(counters(&driver), 2, "one for each other creature");
    Ok(())
}

/// A legendary creature named "Twin", a Sliver or not.
fn twin(sliver: bool) -> Arc<CardDefinition> {
    let mut def = CardDefinition::new("Twin")
        .with_type(CardType::Creature)
        .with_supertype(Supertype::Legendary)
        .with_power_toughness(1, 1);
    if sliver {
        def = def.with_subtype("Sliver");
    }
    Arc::new(def)
}

#[rstest]
fn the_legend_rule_stops_for_the_selected_permanents_only(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] slivers: bool,
) -> Result<(), ScriptError> {
    let queen = card(
        "Queen",
        &[CardType::Enchantment],
        &[r#"{"Rule":{"rule":{"NoLegendRuleFor":[{"Subtype":"Sliver"},{"ControlledBy":"You"}]}}}"#],
    );
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            vec![ready(queen), ready(twin(slivers)), ready(twin(slivers))],
        )
        .battlefield(1, vec![ready(twin(slivers)), ready(twin(slivers))]);
    let mut driver = Driver::from_scenario(scenario)?;
    for _ in 0..10 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let count = |seat: usize| {
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter(|id| {
                let object = driver.state().object(**id).unwrap();
                object.card().name == "Twin" && object.controller() == pid(seat)
            })
            .count()
    };
    assert_eq!(count(0), if slivers { 2 } else { 1 });
    assert_eq!(count(1), 1, "the rule is its controller's");
    Ok(())
}
