//! Goal: the drafts players read as wrong play their Oracle text at two to four seats: End the Festivities and Goblin Chainwhirler deal 1 damage to each opponent and to each creature and each planeswalker those opponents control, never to their controller or its permanents; Ghyrson Starn adds 2 damage whenever any other source its controller controls (a spell or a creature) deals exactly 1 damage to a player or a permanent, and not for 2 damage or an opponent's source.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::card::Card;
use mtg_core::card::CardType;
use mtg_engine::combat::DamageTarget;
use mtg_engine::events::Event;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{burn, cost, free_creature, mountain, pinger, walker};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::c4_control::{replays, settle};
use crate::common::{name_of, pid};

/// The abilities of the committed draft `crates/mtg-cli/drafts/<slug>.json`.
fn draft(slug: &str) -> Vec<Ability> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../mtg-cli/drafts/{slug}.json"));
    let card: Card = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    card.faces
        .into_iter()
        .flat_map(|face| face.abilities)
        .collect()
}

/// A free card of this type with the draft's abilities.
fn drafted(name: &str, slug: &str, kind: CardType) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(kind)
        .with_mana_cost(cost(0, &[]));
    if kind == CardType::Creature {
        card = card.with_power_toughness(2, 2);
    }
    for ability in draft(slug) {
        card = card.with_rules(ability);
    }
    Arc::new(card)
}

/// The damage each recipient was dealt this game, by player seat (`P<n>`) or permanent name.
fn damage(driver: &Driver) -> BTreeMap<String, u32> {
    let mut dealt = BTreeMap::new();
    for event in driver.engine().events() {
        if let Event::DamageDealt { target, amount, .. } = event {
            let key = match target {
                DamageTarget::Player(player) => format!("P{}", player.index()),
                DamageTarget::Object(id) => name_of(driver.state(), *id),
            };
            *dealt.entry(key).or_insert(0) += amount;
        }
    }
    dealt
}

/// Each seat controls `Bear <seat>` and `Walker <seat>`; seat 0 also has `card` in hand.
fn sweep_table(seats: usize, card: Arc<CardDefinition>) -> Scenario {
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1).hand(0, [card]);
    for seat in 0..seats {
        scenario = scenario.battlefield(
            seat,
            [
                ready(free_creature(&format!("Bear {seat}"), 2, 2)),
                ready(walker(&format!("Walker {seat}"), 0)),
            ],
        );
    }
    scenario
}

#[rstest]
fn each_opponent_and_each_creature_and_planeswalker_they_control_takes_one(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        ("End the Festivities", "end_the_festivities", CardType::Sorcery),
        ("Goblin Chainwhirler", "goblin_chainwhirler", CardType::Creature)
    )]
    card: (&str, &str, CardType),
) -> Result<(), ScriptError> {
    let (name, slug, kind) = card;
    let mut driver = Driver::from_scenario(sweep_table(seats, drafted(name, slug, kind)))?;
    driver.cast(name)?;
    settle(&mut driver, None)?;
    let mut expected = BTreeMap::new();
    for seat in 1..seats {
        expected.insert(format!("P{seat}"), 1);
        expected.insert(format!("Bear {seat}"), 1);
        expected.insert(format!("Walker {seat}"), 1);
    }
    assert_eq!(damage(&driver), expected);
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 20);
    replays(&driver);
    Ok(())
}

/// How seat 0 deals its damage.
#[derive(Debug, Clone, Copy)]
enum Source {
    /// An instant dealing this much damage.
    Spell(i32),
    /// A creature's "{T}: deal 1 damage to any target".
    Creature,
}

#[rstest]
fn ghyrson_adds_two_after_exactly_one_damage_from_any_other_source_you_control(
    #[values(2, 3, 4)] seats: usize,
    #[values(Source::Spell(1), Source::Spell(2), Source::Creature)] source: Source,
    #[values(true, false)] at_player: bool,
) -> Result<(), ScriptError> {
    let ghyrson = drafted("Ghyrson", "ghyrson_starn_kelermorph", CardType::Creature);
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(ghyrson),
                ready(mountain()),
                ready(pinger("Pinger", 0)),
            ],
        )
        .battlefield(1, [ready(free_creature("Ogre", 5, 5))]);
    if let Source::Spell(amount) = source {
        scenario = scenario.hand(0, [burn("Burn", amount)]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let target = if at_player {
        driver.target_player(1)
    } else {
        driver.target_named("Ogre")?
    };
    match source {
        Source::Spell(_) => {
            driver.cast_at("Burn", &[target])?;
        }
        Source::Creature => {
            driver.begin_activation("Pinger")?;
            driver.choose_targets(&[target])?;
        }
    }
    settle(&mut driver, None)?;
    let first = match source {
        Source::Spell(amount) => u32::try_from(amount).unwrap(),
        Source::Creature => 1,
    };
    let added = if first == 1 { 2 } else { 0 };
    let key = if at_player { "P1" } else { "Ogre" };
    assert_eq!(
        damage(&driver),
        BTreeMap::from([(key.to_owned(), first + added)])
    );
    replays(&driver);
    Ok(())
}

#[rstest]
fn ghyrson_ignores_one_damage_from_an_opponents_source(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let ghyrson = drafted("Ghyrson", "ghyrson_starn_kelermorph", CardType::Creature);
    let scenario = Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(0, [ready(ghyrson)])
        .battlefield(1, [ready(mountain())])
        .hand(1, [burn("Burn", 1)]);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_player(0);
    driver.cast_at("Burn", &[target])?;
    settle(&mut driver, None)?;
    assert_eq!(damage(&driver), BTreeMap::from([("P0".to_owned(), 1)]));
    Ok(())
}
