//! Goal: the r28-4 engine pieces play out at two to four seats: a Defiler's rule lets 2 life pay for one symbol of its color of the spells it names (one per rule, never another color, kind or player), a splice cost of life or of a permanent picked is paid with the spell and offered only when it can be, and a spell or permanent turned another color for good stays that color; every game replays from its log.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::copying::{replays, run};
use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::{Keyword, SpliceOnto};
use mtg_cards::modify::GameRule;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::CostsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup, replay};
use mtg_testkit::cards::{bear, burn, forest, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The permanent card types, as "permanent spells" selects them.
fn permanent_spells(color: Color) -> Selector {
    let kinds = [
        CardType::Artifact,
        CardType::Battle,
        CardType::Creature,
        CardType::Enchantment,
        CardType::Land,
        CardType::Planeswalker,
    ];
    Selector(vec![
        Pred::Color(color),
        Pred::AnyOf(kinds.into_iter().map(Pred::Type).collect()),
    ])
}

/// An artifact with "As an additional cost to cast <color> permanent spells, you may pay 2 life. Those spells cost one
/// symbol of the color less ...".
fn defiler(color: Color) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Defiler")
            .with_type(CardType::Artifact)
            .with_rules(Ability::Rule {
                rule: GameRule::PayLifeForOneSymbol {
                    who: Ref::You,
                    color,
                    spells: permanent_spells(color),
                },
                condition: None,
                from: ZoneKind::Battlefield,
            }),
    )
}

/// A card of this type and mana cost.
fn spell(kind: CardType, cost: &str) -> Arc<CardDefinition> {
    let card = CardDefinition::new("Spell")
        .with_type(kind)
        .with_mana_cost(ManaCost::parse(cost).unwrap());
    Arc::new(if kind == CardType::Creature {
        card.with_power_toughness(2, 2)
    } else {
        card.with_rules(
            serde_json::from_str::<Ability>(r#"{"Spell":{"effects":[{"Draw":{}}]}}"#).unwrap(),
        )
    })
}

/// Whether seat 0 may cast its spell now.
fn castable(driver: &Driver) -> bool {
    let id = driver.state().zone(Zone::Hand(pid(0)))[0];
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.castable.contains(&id),
        _ => false,
    }
}

#[rstest]
#[case::one_symbol_for_life(CardType::Creature, "{G}{G}", 1, 0, true, 18)]
#[case::only_one_symbol_per_rule(CardType::Creature, "{G}{G}{G}", 1, 0, false, 20)]
#[case::two_rules_two_symbols(CardType::Creature, "{G}{G}{G}", 2, 0, true, 16)]
#[case::not_a_permanent_spell(CardType::Instant, "{G}{G}", 1, 0, false, 20)]
#[case::not_generic_mana(CardType::Creature, "{2}{G}", 1, 0, false, 20)]
#[case::an_opponent_s_rule(CardType::Creature, "{G}{G}", 1, 1, false, 20)]
fn two_life_pays_for_one_symbol_of_the_rule_s_color(
    #[values(2, 3, 4)] seats: usize,
    #[case] kind: CardType,
    #[case] cost: &str,
    #[case] rules: usize,
    #[case] owner: usize,
    #[case] can: bool,
    #[case] life: i32,
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut mine = vec![ready(forest())];
        let mut theirs = Vec::new();
        let held = if owner == 0 { &mut mine } else { &mut theirs };
        held.extend((0..rules).map(|_| ready(defiler(Color::Green))));
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, [spell(kind, cost)])
            .battlefield(0, mine)
            .battlefield(1, theirs)
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    assert_eq!(castable(&driver), can);
    if can {
        driver.cast("Spell")?;
        run(&mut driver, seats)?;
        let id = driver.id_of("Spell")?;
        assert_eq!(driver.state().object(id).unwrap().zone(), Zone::Battlefield);
    }
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), life);
    replays(&driver, &setup);
    Ok(())
}

/// An Arcane instant that deals 2 damage to any target, with splice onto Arcane for `cost`.
fn splicer(cost: &str) -> Arc<CardDefinition> {
    let cost: Cost = serde_json::from_str(cost).unwrap();
    Arc::new(
        (*burn("Splicer", 2))
            .clone()
            .with_subtype("Arcane")
            .with_rules(Ability::Keyword(Keyword::Splice {
                onto: SpliceOnto::Arcane,
                cost,
            })),
    )
}

/// `seats` players; player 0 holds an Arcane bolt and the splicer, with a Mountain and `bears` Grizzly Bears.
fn table(seats: usize, cost: &str, bears: usize) -> GameSetup {
    let bolt = Arc::new((*burn("Arcane Bolt", 3)).clone().with_subtype("Arcane"));
    let mut mine = vec![ready(mountain())];
    mine.extend((0..bears).map(|_| ready(bear())));
    Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [bolt, splicer(cost)])
        .battlefield(0, mine)
        .setup()
        .unwrap()
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// Whether the pending decision offers costs to choose.
fn offers_costs(driver: &Driver) -> bool {
    matches!(
        driver.decision().unwrap().kind,
        DecisionKind::ChooseCosts(_)
    )
}

const LIFE: &str = r#"[{"PayLife":3}]"#;
const SACRIFICE: &str = r#"[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}]"#;

#[rstest]
#[case::life(LIFE, 0, 17, 0)]
#[case::a_creature_sacrificed(SACRIFICE, 1, 20, 1)]
fn a_splice_cost_beyond_mana_is_paid_with_the_spell(
    #[values(2, 3, 4)] seats: usize,
    #[case] cost: &str,
    #[case] bears: usize,
    #[case] life_after: i32,
    #[case] sacrificed: usize,
) {
    let setup = table(seats, cost, bears);
    let mut driver = Driver::new(Engine::new(setup.clone()).unwrap()).unwrap();
    driver.begin_cast("Arcane Bolt").unwrap();
    assert!(offers_costs(&driver));
    driver
        .answer(Answer::ChooseCosts(CostsChoice::Costs(vec![0])))
        .unwrap();
    let foe = seats - 1;
    driver.choose_targets(&[driver.target_player(foe)]).unwrap();
    driver.choose_targets(&[driver.target_player(foe)]).unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    for _ in 0..20 {
        if driver.state().zone(Zone::Stack).is_empty() {
            break;
        }
        driver.pass().unwrap();
    }
    assert_eq!(life(&driver, foe), 15);
    assert_eq!(life(&driver, 0), life_after);
    let dead = driver
        .state()
        .zone(Zone::Graveyard(pid(0)))
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == "Grizzly Bears")
        .count();
    assert_eq!(dead, sacrificed);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

#[rstest]
fn a_splice_cost_with_nothing_to_pick_is_not_offered(#[values(2, 3, 4)] seats: usize) {
    let mut driver = Driver::new(Engine::new(table(seats, SACRIFICE, 0)).unwrap()).unwrap();
    driver.begin_cast("Arcane Bolt").unwrap();
    assert!(!offers_costs(&driver));
}

/// An instant that turns its target, a spell or a permanent, `color` for good.
fn lace(color: Color) -> Arc<CardDefinition> {
    let rules = serde_json::json!({ "Spell": {
        "targets": [{ "what": { "Object": [{ "AnyOf": ["Spell", { "In": "Battlefield" }] }] } }],
        "effects": [{ "Continuous": {
            "affected": { "Target": 0 },
            "mods": [{ "SetColors": [color] }],
            "duration": "Forever"
        } }]
    } });
    Arc::new(
        CardDefinition::new("Lace")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{R}").unwrap())
            .with_rules(serde_json::from_value::<Ability>(rules).unwrap()),
    )
}

#[rstest]
fn a_permanent_turned_another_color_stays_that_color(#[values(2, 3, 4)] seats: usize) {
    let scenario = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, [lace(Color::Black)])
            .battlefield(0, [ready(mountain()), ready(bear())])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario()).unwrap();
    let bear_id = driver.id_of("Grizzly Bears").unwrap();
    driver.begin_cast("Lace").unwrap();
    driver
        .choose_targets(&[driver.target_named("Grizzly Bears").unwrap()])
        .unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    run(&mut driver, seats).unwrap();
    let colors = mtg_engine::characteristics::characteristics(driver.state(), bear_id)
        .unwrap()
        .colors;
    assert_eq!(colors.iter().collect::<Vec<_>>(), vec![Color::Black]);
    replays(&driver, &setup);
}
