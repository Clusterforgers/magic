//! Goal: pack e14d's mechanics play whole at tables of two, three and four seats: recruit draws, discards and makes a Human Soldier only for a nonland discard; protection with an exception keeps the Auras it spares attached while plain protection removes even its own Aura; a suspected creature has menace and is not offered as a blocker, and a creature card returned to the battlefield is suspected as the permanent it became.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::RefCell;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::macros;
use mtg_cards::refs::Ref;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind};
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, free_creature, mountain};
use mtg_testkit::{Driver, ScriptError, attached_to, ready};
use rstest::rstest;

use crate::common::pid;
use crate::macro_keywords::{permanents, play, table};

/// A creature that recruits when it attacks.
fn recruiter() -> Arc<CardDefinition> {
    let effects =
        serde_json::to_string(&macros::action_effects("recruit", 0, &Ref::You).unwrap()).unwrap();
    let ability: Ability = serde_json::from_str(&format!(
        r#"{{"Triggered":{{"when":{{"Attacks":{{"what":["This"]}}}},"effects":{effects}}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Recruiter")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(ability),
    )
}

#[rstest]
fn recruit_makes_a_soldier_only_for_a_nonland_discard(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] nonland: bool,
) -> Result<(), ScriptError> {
    let top = if nonland {
        burn("Shock", 2)
    } else {
        mountain()
    };
    let scenario =
        table(seats, vec![recruiter()], &[free_creature("Wall", 0, 4)]).library(0, vec![top]);
    let mut driver = Driver::from_scenario(scenario)?;
    play(
        &mut driver,
        &["Recruiter"],
        &[],
        &|_, _| None,
        Step::EndCombat,
    )?;
    let state = driver.state();
    assert_eq!(state.zone(Zone::Hand(pid(0))).len(), 0);
    assert_eq!(state.zone(Zone::Graveyard(pid(0))).len(), 1);
    assert_eq!(permanents(&driver, 0), 1 + usize::from(nonland));
    if nonland {
        assert!(driver.id_of("Human Soldier").is_ok());
    }
    Ok(())
}

/// A white Aura with these rules besides "Enchant creature".
fn white_aura(name: &str, rules: &[&str]) -> Arc<CardDefinition> {
    let enchant: Ability =
        serde_json::from_str(r#"{"Keyword":{"Enchant":[{"Type":"Creature"}]}}"#).unwrap();
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Enchantment)
        .with_subtype("Aura")
        .with_mana_cost(ManaCost::parse("{W}").unwrap())
        .with_rules(enchant);
    for rule in rules {
        card = card.with_rules(serde_json::from_str::<Ability>(rule).unwrap());
    }
    Arc::new(card)
}

/// Enchanted creature has protection from white, and the effect doesn't remove what `spares` matches.
fn ward(spares: &str) -> Arc<CardDefinition> {
    let rule = format!(
        r#"{{"Static":{{"affected":"Attached","mods":[{{"AddKeyword":{{"ProtectionSparing":{{"from":[{{"Color":"White"}}],"spares":{spares}}}}}}}]}}}}"#
    );
    white_aura("Ward", &[&rule])
}

/// Where seat 0's card of that name is: on the battlefield or in its graveyard.
fn zone_of(driver: &Driver, name: &str) -> Option<Zone> {
    let state = driver.state();
    [Zone::Battlefield, Zone::Graveyard(pid(0))]
        .into_iter()
        .find(|zone| {
            state
                .zone(*zone)
                .iter()
                .any(|id| state.object(*id).is_some_and(|o| o.card().name == name))
        })
}

#[rstest]
#[case::this_aura(r#"["This"]"#, false)]
#[case::auras(r#"[{"Subtype":"Aura"}]"#, true)]
fn protection_with_an_exception_keeps_the_spared_auras_on(
    #[values(2, 3, 4)] seats: usize,
    #[case] spares: &str,
    #[case] other_stays: bool,
) -> Result<(), ScriptError> {
    let scenario = table(
        seats,
        vec![free_creature("Bear", 2, 2)],
        &[free_creature("Wall", 0, 4)],
    )
    .battlefield(
        0,
        vec![
            ready(free_creature("Bear", 2, 2)),
            attached_to(ready(ward(spares)), 0, 0),
            attached_to(ready(white_aura("Halo", &[])), 0, 0),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::BeginCombat)?;
    assert_eq!(zone_of(&driver, "Ward"), Some(Zone::Battlefield));
    let halo = if other_stays {
        Zone::Battlefield
    } else {
        Zone::Graveyard(pid(0))
    };
    assert_eq!(zone_of(&driver, "Halo"), Some(halo));
    Ok(())
}

#[rstest]
fn plain_protection_removes_its_own_aura(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let rule = r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":{"Protection":[{"Color":"White"}]}}]}}"#;
    let scenario = table(
        seats,
        vec![free_creature("Bear", 2, 2)],
        &[free_creature("Wall", 0, 4)],
    )
    .battlefield(
        0,
        vec![
            ready(free_creature("Bear", 2, 2)),
            attached_to(ready(white_aura("Ward", &[rule])), 0, 0),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::BeginCombat)?;
    assert_eq!(zone_of(&driver, "Ward"), Some(Zone::Graveyard(pid(0))));
    Ok(())
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

/// The names of the objects.
fn names_of(driver: &Driver, ids: impl Iterator<Item = mtg_core::ids::ObjectId>) -> Vec<String> {
    ids.map(|id| driver.state().object(id).unwrap().card().name.clone())
        .collect()
}

#[rstest]
fn a_suspected_creature_has_menace_and_cant_block(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(
        seats,
        vec![accuser()],
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
                offered.replace(Some(names_of(driver, ids)));
                None
            }
            _ => None,
        }
    };
    play(&mut driver, &["Accuser"], &[], &hook, Step::EndCombat)?;
    assert_eq!(offered.take(), Some(vec!["Guard".to_owned()]));
    let wall = driver.id_of("Wall")?;
    assert!(keywords_of(driver.state(), wall).contains(&&Keyword::Menace));
    Ok(())
}

#[rstest]
fn a_creature_returned_to_the_battlefield_is_the_one_suspected(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // "Return target creature card from your graveyard to the battlefield. Suspect it." (It Doesn't Add Up)
    let spell: Ability = serde_json::from_str(
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"OwnedBy":"You"},{"In":"Graveyard"}]}}],"effects":[{"Move":{"to":{"Battlefield":{}},"what":{"Target":0}}},{"Suspect":{"what":{"Target":0}}}]}}"#,
    )
    .unwrap();
    let doubt = Arc::new(
        CardDefinition::new("Doubt")
            .with_type(CardType::Sorcery)
            .with_mana_cost(ManaCost::parse("{0}").unwrap())
            .with_rules(spell),
    );
    let scenario = table(seats, vec![], &[free_creature("Wall", 0, 4)])
        .hand(0, vec![doubt])
        .graveyard(0, vec![free_creature("Bear", 2, 2)]);
    let mut driver = Driver::from_scenario(scenario)?;
    let bear = driver.state().zone(Zone::Graveyard(pid(0)))[0];
    let named = driver.state().object(bear).unwrap().reference();
    driver.cast_at("Doubt", &[Target::Object(named)])?;
    crate::macro_keywords::fight_on(&mut driver, Step::Main1)?;
    let bear = driver.id_of("Bear")?;
    let held = driver.state().object(bear).unwrap().permanent().unwrap();
    assert!(held.suspected());
    assert!(keywords_of(driver.state(), bear).contains(&&Keyword::Menace));
    Ok(())
}
