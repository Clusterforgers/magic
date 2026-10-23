//! Goal: mana restricted by what it is spent on (a list of creature types, a card type with the abilities of the same type, a mana value or an X in the cost, a spell cast from a graveyard, a spell the caster does not own) pays only for a spell or an ability of that kind, at two to four seats, and a spell cast from a graveyard is told from the same spell cast from a hand.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::DecisionKind;
use mtg_engine::spend::Spend;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use CardType::{Artifact, Creature, Instant};

/// A spell of these types and subtypes with this cost, `x` of its symbols being {X}.
fn spell(types: &[CardType], subtypes: &[&str], cost: &str, x: u8) -> Arc<CardDefinition> {
    let mut definition = CardDefinition::new("Spell")
        .with_mana_cost(ManaCost::parse(cost).unwrap())
        .with_x_in_cost(x);
    for card_type in types {
        definition = definition.with_type(*card_type);
    }
    for subtype in subtypes {
        definition = definition.with_subtype(*subtype);
    }
    if types.contains(&Creature) {
        definition = definition.with_power_toughness(1, 1);
    }
    Arc::new(definition)
}

/// A permanent of these types and subtypes with the ability "{2}: You gain 2 life."
fn source(name: &str, types: &[CardType], subtypes: &[&str]) -> Arc<CardDefinition> {
    let mut definition = CardDefinition::new(name).with_rules(
        serde_json::from_str(
            r#"{"Activated":{"cost":[{"Mana":"{2}"}],"effects":[{"GainLife":{"amount":2}}]}}"#,
        )
        .unwrap(),
    );
    for card_type in types {
        definition = definition.with_type(*card_type);
    }
    for subtype in subtypes {
        definition = definition.with_subtype(*subtype);
    }
    Arc::new(definition)
}

/// A permanent that lets its controller cast the artifacts in their graveyard.
fn key() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Key")
            .with_type(Artifact)
            .with_rules(
                serde_json::from_str(
                    r#"{"Rule":{"rule":{"MayCast":{"from":"Graveyard","what":[{"Type":"Artifact"}]}}}}"#,
                )
                .unwrap(),
            ),
    )
}

fn named(name: &str) -> mtg_engine::PresetPermanent {
    match name {
        "Forest" => ready(forest()),
        "Island" => ready(island()),
        "Mountain" => ready(mountain()),
        other => ready(card(other).unwrap()),
    }
}

/// A table of `seats` where seat 0 holds the spell and has these permanents in play.
fn table(
    seats: usize,
    spell: Arc<CardDefinition>,
    mine: &[&str],
    more: Vec<Arc<CardDefinition>>,
) -> Result<Driver, ScriptError> {
    let mut permanents: Vec<_> = mine.iter().map(|name| named(name)).collect();
    permanents.extend(more.into_iter().map(ready));
    Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [spell])
            .battlefield(0, permanents),
    )
}

fn castable(driver: &Driver) -> Result<bool, ScriptError> {
    match &driver.decision()?.kind {
        DecisionKind::Priority(options) => Ok(!options.castable.is_empty()),
        _ => Err(ScriptError::Expectation("priority was expected".into())),
    }
}

fn bear() -> Arc<CardDefinition> {
    free_creature("Bear", 2, 2)
}

#[rstest]
#[case::an_elemental_spell("Flamebraider", &[Creature], &["Elemental"], "{R}{G}", 0, true)]
#[case::a_goblin_spell("Flamebraider", &[Creature], &["Goblin"], "{R}{G}", 0, false)]
#[case::a_dinosaur_spell("Ixalli's Lorekeeper", &[Creature], &["Dinosaur"], "{U}", 0, true)]
#[case::an_elf_spell("Ixalli's Lorekeeper", &[Creature], &["Elf"], "{U}", 0, false)]
#[case::a_vampire_spell("Master of Dark Rites", &[Creature], &["Vampire"], "{B}{B}{B}", 0, true)]
#[case::a_cleric_spell("Master of Dark Rites", &[Creature], &["Cleric"], "{B}{B}{B}", 0, true)]
#[case::a_demon_spell("Master of Dark Rites", &[Creature], &["Demon"], "{B}{B}{B}", 0, true)]
#[case::a_zombie_spell("Master of Dark Rites", &[Creature], &["Zombie"], "{B}{B}{B}", 0, false)]
#[case::an_artifact_spell("Vedalken Engineer", &[Artifact], &[], "{U}{U}", 0, true)]
#[case::a_creature_spell("Vedalken Engineer", &[Creature], &[], "{U}{U}", 0, false)]
#[case::a_cost_with_x("Troyan, Gutsy Explorer", &[Instant], &[], "{G}{U}", 1, true)]
#[case::a_cost_without_x("Troyan, Gutsy Explorer", &[Instant], &[], "{G}{U}", 0, false)]
fn the_mana_of_a_source_pays_only_for_the_spells_it_allows(
    #[values(2, 3, 4)] seats: usize,
    #[case] source: &str,
    #[case] types: &[CardType],
    #[case] subtypes: &[&str],
    #[case] cost: &str,
    #[case] x: u8,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let definition = spell(types, subtypes, cost, x);
    // The sacrifice of Master of Dark Rites needs another creature.
    let driver = table(seats, definition, &[source], vec![bear()])?;
    assert_eq!(castable(&driver)?, offered);
    Ok(())
}

#[rstest]
#[case::mana_value_5(&["Forest", "Forest", "Forest"], "{3}{G}{U}", true)]
#[case::mana_value_4(&["Forest", "Forest", "Forest"], "{2}{G}{U}", false)]
fn a_mana_value_of_five_or_more_is_what_troyans_mana_pays_for(
    #[values(2, 3, 4)] seats: usize,
    #[case] lands: &[&str],
    #[case] cost: &str,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut mine = vec!["Troyan, Gutsy Explorer"];
    mine.extend(lands);
    let driver = table(seats, spell(&[Creature], &[], cost, 0), &mine, Vec::new())?;
    assert_eq!(castable(&driver)?, offered);
    Ok(())
}

#[rstest]
#[case::an_artifact_ability("Gadget", &[Artifact], &[], true)]
#[case::a_creature_ability("Totem", &[Creature], &[], false)]
fn the_mana_for_artifacts_pays_for_the_abilities_of_artifacts(
    #[values(2, 3, 4)] seats: usize,
    #[case] name: &str,
    #[case] types: &[CardType],
    #[case] subtypes: &[&str],
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let driver = table(
        seats,
        spell(&[Instant], &[], "{U}{U}{U}{U}", 0),
        &["Vedalken Engineer"],
        vec![source(name, types, subtypes)],
    )?;
    assert_eq!(!driver.offered_abilities(name)?.is_empty(), offered);
    Ok(())
}

#[rstest]
#[case::an_elemental_source("Hearth", &[Artifact], &["Elemental"], true)]
#[case::a_goblin_source("Camp", &[Artifact], &["Goblin"], false)]
fn the_mana_for_elementals_pays_for_the_abilities_of_elemental_sources(
    #[values(2, 3, 4)] seats: usize,
    #[case] name: &str,
    #[case] types: &[CardType],
    #[case] subtypes: &[&str],
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let driver = table(
        seats,
        spell(&[Instant], &[], "{U}{U}{U}{U}", 0),
        &["Flamebraider"],
        vec![source(name, types, subtypes)],
    )?;
    assert_eq!(!driver.offered_abilities(name)?.is_empty(), offered);
    Ok(())
}

/// A table where seat 0 has Lord of the Forsaken and the artifact `Rock` for {2} in the zone, with a permission to cast
/// artifacts from the graveyard.
fn graveyard_table(seats: usize, in_graveyard: bool) -> Result<Driver, ScriptError> {
    let rock = spell(&[Artifact], &[], "{1}", 0);
    let mut scenario =
        Scenario::new(seats).battlefield(0, vec![named("Lord of the Forsaken"), ready(key())]);
    scenario = if in_graveyard {
        scenario.graveyard(0, [rock])
    } else {
        scenario.hand(0, [rock])
    };
    Driver::from_scenario(scenario)
}

#[rstest]
#[case::from_a_graveyard(true, true)]
#[case::from_a_hand(false, false)]
fn lords_mana_pays_for_a_spell_cast_from_a_graveyard_alone(
    #[values(2, 3, 4)] seats: usize,
    #[case] in_graveyard: bool,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let driver = graveyard_table(seats, in_graveyard)?;
    assert_eq!(castable(&driver)?, offered);
    Ok(())
}

#[rstest]
#[case::cast_from_the_graveyard_of_its_owner(0, true)]
#[case::the_card_of_another_player(1, false)]
fn a_spell_is_told_from_a_graveyard_and_by_who_owns_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] owner: usize,
    #[case] owned_by_the_caster: bool,
) {
    let rock = spell(&[Artifact], &[], "{2}", 0);
    let driver = Driver::from_scenario(Scenario::new(seats).graveyard(owner, [rock])).unwrap();
    let id = driver.state().zone(Zone::Graveyard(pid(owner)))[0];
    let spend = Spend::for_spell(driver.state(), id, pid(0), false);
    assert!(spend.traits.from_graveyard);
    assert_eq!(spend.traits.not_owned, !owned_by_the_caster);
}
