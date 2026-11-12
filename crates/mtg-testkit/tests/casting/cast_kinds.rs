//! Goal: a trigger on casting a spell of a kind (a subtype, several subtypes, "permanent", "multicolored", "historic", a subtype with a card type) fires for a matching spell on the stack and not for a near-miss at 2 to 4 seats, and the cards that read those kinds do what they say: a token for a Merfolk or a multicolored spell, a copy of a Faerie or Wizard permanent spell, and one Clue a turn for any number of historic spells.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Supertype};
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, mountain};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const SEAT: PlayerId = PlayerId(0);

/// A spell to cast: its types, whether it is legendary, its subtypes and its colors.
#[derive(Clone, Copy)]
struct Spell {
    types: &'static [CardType],
    legendary: bool,
    subtypes: &'static [&'static str],
    colors: &'static [Color],
}

impl Spell {
    const fn new(
        types: &'static [CardType],
        subtypes: &'static [&'static str],
        colors: &'static [Color],
    ) -> Spell {
        Spell {
            types,
            legendary: false,
            subtypes,
            colors,
        }
    }

    const fn legendary(self) -> Spell {
        Spell {
            legendary: true,
            ..self
        }
    }

    /// The card, called "Test Spell", costing one mana of each of its colors or {1}.
    fn card(self) -> Arc<CardDefinition> {
        let generic = u32::from(self.colors.is_empty());
        let mut card = CardDefinition::new("Test Spell").with_mana_cost(cost(generic, self.colors));
        for found in self.types {
            card = card.with_type(*found);
        }
        for name in self.subtypes {
            card = card.with_subtype(*name);
        }
        if self.legendary {
            card = card.with_supertype(Supertype::Legendary);
        }
        if self.types.contains(&CardType::Creature) {
            card = card.with_power_toughness(1, 1);
        }
        Arc::new(card)
    }
}

const CREATURE: &[CardType] = &[CardType::Creature];
const ARTIFACT: &[CardType] = &[CardType::Artifact];
const ENCHANTMENT: &[CardType] = &[CardType::Enchantment];
const INSTANT: &[CardType] = &[CardType::Instant];
const RED: &[Color] = &[Color::Red];
const GREEN: &[Color] = &[Color::Green];
const RED_GREEN: &[Color] = &[Color::Red, Color::Green];

const PERMANENT: &str = r#"{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]}"#;
const HISTORIC: &str =
    r#"{"AnyOf":[{"Type":"Artifact"},{"Supertype":"Legendary"},{"Subtype":"Saga"}]}"#;

/// An enchantment that gains its controller 1 life whenever they cast a spell of the kind (the `what` of the pattern).
fn watcher(kind: &str) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"Triggered":{{"when":{{"SpellCast":{{"what":[{kind}],"by":"You"}}}},"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#
    );
    let ability: Ability = serde_json::from_str(&json).unwrap();
    Arc::new(
        CardDefinition::new("Watcher")
            .with_type(CardType::Enchantment)
            .with_rules(ability),
    )
}

/// A table of `seats` where seat 0 has the watcher, `others` on its battlefield, lands and the spell in hand.
fn table(
    seats: usize,
    watching: Option<Arc<CardDefinition>>,
    others: &[&str],
    spell: Spell,
) -> Scenario {
    let mut mine: Vec<_> = watching.into_iter().map(ready).collect();
    mine.extend(others.iter().map(|name| ready(corpus::card(name).unwrap())));
    mine.extend((0..3).map(|_| ready(mountain())));
    mine.extend((0..3).map(|_| ready(forest())));
    Scenario::new(seats)
        .battlefield(0, mine)
        .hand(0, [spell.card()])
}

fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    panic!("the stack never emptied");
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(SEAT).unwrap().life()
}

fn on_battlefield(driver: &Driver, name: &str) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == name)
        .count()
}

#[rstest]
#[case::a_subtype(r#"{"Subtype":"Merfolk"}"#, Spell::new(CREATURE, &["Merfolk"], GREEN), true)]
#[case::a_subtype_among_others(r#"{"Subtype":"Merfolk"}"#, Spell::new(CREATURE, &["Merfolk", "Wizard"], GREEN), true)]
#[case::another_subtype(r#"{"Subtype":"Merfolk"}"#, Spell::new(CREATURE, &["Elf"], RED), false)]
#[case::no_subtype(r#"{"Subtype":"Merfolk"}"#, Spell::new(CREATURE, &[], RED), false)]
#[case::the_first_of_two_subtypes(r#"{"AnyOf":[{"Subtype":"Aura"},{"Subtype":"Equipment"}]}"#, Spell::new(ENCHANTMENT, &["Aura"], GREEN), true)]
#[case::the_second_of_two_subtypes(r#"{"AnyOf":[{"Subtype":"Aura"},{"Subtype":"Equipment"}]}"#, Spell::new(ARTIFACT, &["Equipment"], &[]), true)]
#[case::neither_of_two_subtypes(r#"{"AnyOf":[{"Subtype":"Aura"},{"Subtype":"Equipment"}]}"#, Spell::new(ARTIFACT, &["Vehicle"], &[]), false)]
#[case::a_subtype_and_a_type(r#"{"Type":"Creature"},{"Subtype":"Vampire"}"#, Spell::new(CREATURE, &["Vampire"], RED), true)]
#[case::the_subtype_on_another_type(r#"{"Type":"Creature"},{"Subtype":"Vampire"}"#, Spell::new(ENCHANTMENT, &["Vampire"], RED), false)]
#[case::a_creature_permanent(PERMANENT, Spell::new(CREATURE, &[], RED), true)]
#[case::an_artifact_permanent(PERMANENT, Spell::new(ARTIFACT, &[], &[]), true)]
#[case::an_enchantment_permanent(PERMANENT, Spell::new(ENCHANTMENT, &[], GREEN), true)]
#[case::an_instant_is_no_permanent(PERMANENT, Spell::new(INSTANT, &[], RED), false)]
#[case::two_colors(r#"{"Compare":{"stat":"Colors","op":"Ge","to":2}}"#, Spell::new(CREATURE, &[], RED_GREEN), true)]
#[case::one_color(r#"{"Compare":{"stat":"Colors","op":"Ge","to":2}}"#, Spell::new(CREATURE, &[], RED), false)]
#[case::no_color(r#"{"Compare":{"stat":"Colors","op":"Ge","to":2}}"#, Spell::new(ARTIFACT, &[], &[]), false)]
#[case::a_legendary_is_historic(HISTORIC, Spell::new(CREATURE, &[], RED).legendary(), true)]
#[case::an_artifact_is_historic(HISTORIC, Spell::new(ARTIFACT, &[], &[]), true)]
#[case::a_saga_is_historic(HISTORIC, Spell::new(ENCHANTMENT, &["Saga"], GREEN), true)]
#[case::a_creature_is_not_historic(HISTORIC, Spell::new(CREATURE, &[], RED), false)]
#[case::an_enchantment_is_not_historic(HISTORIC, Spell::new(ENCHANTMENT, &[], GREEN), false)]
fn a_cast_trigger_fires_for_the_kind_of_spell_it_names(
    #[values(2, 3, 4)] seats: usize,
    #[case] kind: &str,
    #[case] spell: Spell,
    #[case] fires: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, Some(watcher(kind)), &[], spell))?;
    let before = life(&driver);
    driver.cast("Test Spell")?;
    resolve(&mut driver)?;
    assert_eq!(life(&driver), before + i32::from(fires));
    Ok(())
}

#[rstest]
#[case::merfolk_makes_a_token("Deeproot Waters", Spell::new(CREATURE, &["Merfolk"], GREEN), "Merfolk", 1)]
#[case::another_subtype_makes_none("Deeproot Waters", Spell::new(CREATURE, &["Elf"], RED), "Merfolk", 0)]
#[case::two_colors_make_a_token("Hero of Precinct One", Spell::new(CREATURE, &[], RED_GREEN), "Human", 1)]
#[case::one_color_makes_none("Hero of Precinct One", Spell::new(CREATURE, &[], RED), "Human", 0)]
#[case::a_wizard_permanent_is_copied("Archmage of Echoes", Spell::new(CREATURE, &["Wizard"], GREEN), "Test Spell", 2)]
#[case::a_faerie_permanent_is_copied("Archmage of Echoes", Spell::new(ARTIFACT, &["Faerie"], &[]), "Test Spell", 2)]
#[case::a_wizard_instant_is_not_copied("Archmage of Echoes", Spell::new(INSTANT, &["Wizard"], GREEN), "Test Spell", 0)]
#[case::another_subtype_is_not_copied("Archmage of Echoes", Spell::new(CREATURE, &["Elf"], RED), "Test Spell", 1)]
fn the_cards_that_read_a_kind_of_spell_act_on_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] card: &str,
    #[case] spell: Spell,
    #[case] named: &str,
    #[case] expected: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, None, &[card], spell))?;
    driver.cast("Test Spell")?;
    resolve(&mut driver)?;
    assert_eq!(on_battlefield(&driver, named), expected);
    Ok(())
}

#[rstest]
fn a_limited_trigger_fires_once_in_a_turn_for_any_number_of_historic_spells(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] spells: usize,
) -> Result<(), ScriptError> {
    let artifacts: Vec<_> = (0..spells)
        .map(|n| {
            Arc::new(
                CardDefinition::new(format!("Gadget {n}"))
                    .with_type(CardType::Artifact)
                    .with_mana_cost(cost(1, &[])),
            )
        })
        .collect();
    let mut mine = vec![ready(corpus::card("Sarah Jane Smith").unwrap())];
    mine.extend((0..6).map(|_| ready(mountain())));
    let mut driver =
        Driver::from_scenario(Scenario::new(seats).battlefield(0, mine).hand(0, artifacts))?;
    for n in 0..spells {
        driver.cast(&format!("Gadget {n}"))?;
        resolve(&mut driver)?;
    }
    assert_eq!(on_battlefield(&driver, "Clue"), 1);
    Ok(())
}
