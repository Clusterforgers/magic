//! Goal: the modifier keys of the verbs on objects are read once for every verb: a damage map, a pump zone, a choice of
//! keyword or counter, a delayed end-step effect, a counter destination, a granted static ability, an attached token and the
//! copy a delayed cast trigger makes; each either means what the card prints or stays a gap named after the key.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{burn, cost, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn card(script: &str, ty: CardType, stats: Option<(i32, i32)>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Subject").with_type(ty);
    if ty != CardType::Creature {
        card = card.with_mana_cost(cost(0, &[Color::Red]));
    }
    if let Some((power, toughness)) = stats {
        card = card.with_power_toughness(power, toughness);
    }
    for value in abilities(script) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

fn settle(driver: &mut Driver) {
    for _ in 0..80 {
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        if idle
            && matches!(
                driver.engine().decision().map(|d| &d.kind),
                Some(DecisionKind::Priority(_))
            )
        {
            return;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack never emptied");
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver
        .state()
        .player(PlayerId::from_index(seat))
        .unwrap()
        .life()
}

const CHAR: &str = "Name:Subject\nTypes:Instant\nManaCost:2 R\nA:SP$ DealDamage | ValidTgts$ Any | NumDmg$ 4 | DamageMap$ True | SubAbility$ Self\nSVar:Self:DB$ DealDamage | Defined$ You | NumDmg$ 2 | SubAbility$ Done\nSVar:Done:DB$ DamageResolve\n";

#[test]
fn a_damage_map_deals_every_damage_of_its_chain() {
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [card(CHAR, CardType::Instant, None)])
        .battlefield(0, [ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_player(1);
    driver.begin_cast("Subject").unwrap();
    driver.choose_targets(&[target]).unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    settle(&mut driver);
    assert_eq!((life(&driver, 1), life(&driver, 0)), (16, 18));
}

#[rstest]
#[case::damage_all(
    "Name:T\nTypes:Sorcery\nA:SP$ DealDamage | ValidTgts$ Player | NumDmg$ 1 | DamageMap$ True | SubAbility$ D\nSVar:D:DB$ DamageAll | NumDmg$ 1 | ValidCards$ Creature | SubAbility$ R\nSVar:R:DB$ DamageResolve\n"
)]
#[case::brothers_of_fire(
    "Name:T\nTypes:Creature Elf\nPT:1/1\nA:AB$ DealDamage | Cost$ 1 R R | ValidTgts$ Any | NumDmg$ 1 | SubAbility$ D | DamageMap$ True\nSVar:D:DB$ DealDamage | Defined$ You | NumDmg$ 1 | SubAbility$ R\nSVar:R:DB$ DamageResolve\n"
)]
fn a_damage_map_chain_has_no_gap(#[case] script: &str) {
    assert!(gaps(script).is_empty(), "{:?}", gaps(script));
}

#[test]
fn a_damage_map_word_other_than_true_is_a_gap() {
    let text =
        "Name:T\nTypes:Instant\nA:SP$ DealDamage | ValidTgts$ Any | NumDmg$ 1 | DamageMap$ Maybe\n";
    assert_eq!(gaps(text), ["key:DamageMap=Maybe"]);
}

fn pump(line: &str) -> String {
    format!(
        "Name:T\nTypes:Instant\nA:SP$ Pump | {line}\nSVar:D:DB$ DealDamage | Defined$ TargetedController | NumDmg$ 2\n"
    )
}

#[rstest]
#[case::battlefield("ValidTgts$ Creature | NumAtt$ 1 | PumpZone$ Battlefield", None)]
#[case::spell_target_only(
    "ValidTgts$ Card | TargetType$ Spell | TgtZone$ Stack | PumpZone$ Stack | SubAbility$ D",
    None
)]
#[case::graveyard(
    "ValidTgts$ Creature.YouOwn | KW$ Flashback | PumpZone$ Graveyard",
    Some("key:Pump.PumpZone=Graveyard")
)]
#[case::hand(
    "Defined$ Self | KW$ Flash | PumpZone$ Hand | Duration$ Permanent",
    Some("key:Pump.PumpZone=Hand")
)]
#[case::stack_that_gains(
    "Defined$ Self | KW$ Rebound | PumpZone$ Stack",
    Some("key:Pump.PumpZone=Stack")
)]
fn a_pump_of_a_card_outside_the_battlefield_is_a_gap(
    #[case] line: &str,
    #[case] gap: Option<&str>,
) {
    let found = gaps(&pump(line));
    match gap {
        Some(gap) => assert_eq!(found, [gap]),
        None => assert!(found.is_empty(), "{found:?}"),
    }
}

#[rstest]
#[case::two("Flying,Lifelink", 2)]
#[case::three("Flying,Deathtouch,Lifelink", 3)]
#[case::four("Flying,Vigilance,Deathtouch,Haste", 4)]
fn a_choice_of_keyword_is_one_of_the_options(#[case] words: &str, #[case] options: usize) {
    let text =
        format!("Name:T\nTypes:Creature Elf\nPT:1/1\nA:AB$ Pump | Cost$ 1 | KWChoice$ {words}\n");
    let found = abilities(&text);
    let mut node = &found[0]["Activated"]["effects"][0];
    let mut seen = 1;
    while let Some(next) = node.pointer("/Either/second/0") {
        seen += 1;
        node = next;
    }
    assert_eq!(seen, options);
}

#[test]
fn a_choice_of_keyword_for_a_group_is_a_gap() {
    let text = "Name:T\nTypes:Instant\nA:SP$ Pump | Defined$ Valid Creature.YouCtrl | KWChoice$ Flying,Vigilance\n";
    assert_eq!(gaps(text), ["key:Pump.KWChoice-for-a-group"]);
}

#[test]
fn a_keyword_choice_gives_the_first_keyword_to_the_passive_answer() {
    let text = "Name:Subject\nTypes:Creature Elf\nPT:1/1\nA:AB$ Pump | Cost$ 0 | KWChoice$ Flying,Lifelink\n";
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card(text, CardType::Creature, Some((1, 1))))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Subject").unwrap();
    driver.pay_tapping(&[], 0).unwrap();
    settle(&mut driver);
    let id = driver.id_of("Subject").unwrap();
    let has = |keyword| mtg_engine::keywords::has_keyword(driver.state(), id, &keyword);
    assert!(has(Keyword::Flying));
    assert!(!has(Keyword::Lifelink));
}

#[rstest]
#[case::sacrifice("Sacrifice", true)]
#[case::destroy("Destroy", true)]
#[case::exile_in_your_turn_word("SacrificeCtrl", false)]
fn a_pump_that_ends_the_creature_at_the_end_step_says_so(#[case] word: &str, #[case] read: bool) {
    let text = format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nA:AB$ Pump | Cost$ 2 | Defined$ Self | NumAtt$ +1 | AtEOT$ {word}\n"
    );
    assert_eq!(gaps(&text).is_empty(), read, "{:?}", gaps(&text));
}

#[test]
fn a_pump_that_sacrifices_at_the_end_step_takes_the_creature_then() {
    let text = "Name:Subject\nTypes:Creature Elf\nPT:1/1\nA:AB$ Pump | Cost$ 0 | Defined$ Self | NumAtt$ +4 | AtEOT$ Sacrifice\n";
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card(text, CardType::Creature, Some((1, 1))))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Subject").unwrap();
    driver.pay_tapping(&[], 0).unwrap();
    settle(&mut driver);
    assert!(driver.id_of("Subject").is_ok());
    for _ in 0..60 {
        if driver.state().turn().step() == Step::Cleanup || driver.id_of("Subject").is_err() {
            break;
        }
        driver.answer_passively().unwrap();
    }
    assert!(driver.id_of("Subject").is_err(), "the creature stays");
}

#[test]
fn a_targeted_pump_that_ends_at_the_end_step_holds_the_target_by_name() {
    let text =
        "Name:T\nTypes:Instant\nA:SP$ Pump | ValidTgts$ Creature | KW$ Haste | AtEOT$ Sacrifice\n";
    let found = abilities(text);
    let effects = &found[0]["Spell"]["effects"];
    assert_eq!(effects[1]["RepeatEach"]["over"], json!({"Target": 0}));
    assert!(
        effects[1]
            .pointer("/RepeatEach/effects/0/DelayedTrigger")
            .is_some()
    );
}

#[rstest]
#[case::graveyard("Graveyard", None)]
#[case::exile("Exile", Some("key:Counter.Destination=Exile"))]
#[case::library("TopOfLibrary", Some("key:Counter.Destination=TopOfLibrary"))]
fn a_countered_spell_goes_to_the_graveyard_or_the_translation_is_a_gap(
    #[case] word: &str,
    #[case] gap: Option<&str>,
) {
    let text = format!(
        "Name:T\nTypes:Instant\nA:SP$ Counter | TargetType$ Spell | ValidTgts$ Card | Destination$ {word}\n"
    );
    let found = gaps(&text);
    assert_eq!(found.first().map(String::as_str), gap);
}

#[rstest]
#[case::battlefield("Battlefield", None)]
#[case::graveyard("Graveyard", Some("key:DestroyAll.Zone=Graveyard"))]
fn destroy_all_names_its_zone(#[case] word: &str, #[case] gap: Option<&str>) {
    let text =
        format!("Name:T\nTypes:Sorcery\nA:SP$ DestroyAll | ValidCards$ Creature | Zone$ {word}\n");
    let found = gaps(&text);
    assert_eq!(found.first().map(String::as_str), gap, "{found:?}");
}

#[rstest]
#[case::one_kind("P1P1", 1)]
#[case::keyword_kinds("Vigilance,Reach,Trample", 3)]
#[case::mixed("P1P1,First Strike,Trample", 3)]
#[case::named("Verse", 1)]
fn counter_kinds_listed_with_commas_are_a_choice(#[case] kinds: &str, #[case] options: usize) {
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ PutCounter | ValidTgts$ Creature | CounterType$ {kinds} | CounterNum$ 1\n"
    );
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    let found = abilities(&text);
    let mut node = &found[0]["Spell"]["effects"][0];
    let mut seen = 1;
    while let Some(next) = node.pointer("/Either/second/0") {
        seen += 1;
        node = next;
    }
    assert_eq!(seen, options);
}

#[rstest]
#[case::each_from_source("EachFromSource")]
#[case::existing("ExistingCounter")]
#[case::unknown("Wibble")]
fn a_counter_rule_or_an_unknown_kind_is_a_gap(#[case] kind: &str) {
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ PutCounter | ValidTgts$ Creature | CounterType$ {kind}\n"
    );
    assert_eq!(gaps(&text), ["key:PutCounter.CounterType"]);
}

#[test]
fn counters_put_by_a_pick_of_the_resolution_go_on_the_creature_picked() {
    let text = "Name:Subject\nTypes:Sorcery\nManaCost:R\nA:SP$ PutCounter | Choices$ Creature.YouCtrl | CounterType$ P1P1 | CounterNum$ 2\n";
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [card(text, CardType::Sorcery, None)])
        .battlefield(0, [ready(mountain()), ready(free_creature("Bear", 2, 2))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast_tapping("Subject", &["Mountain"]).unwrap();
    settle(&mut driver);
    let bear = driver.id_of("Bear").unwrap();
    let counters = driver
        .state()
        .object(bear)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne);
    assert_eq!(counters, 2);
}

#[test]
fn a_pick_together_with_a_target_is_a_gap() {
    let text = "Name:T\nTypes:Sorcery\nA:SP$ PutCounter | ValidTgts$ Creature | Choices$ Creature.YouCtrl | CounterType$ P1P1\n";
    assert_eq!(gaps(text), ["key:PutCounter.Choices-with-a-target"]);
}

#[test]
fn an_animation_without_an_object_changes_the_source() {
    let text = "Name:T\nTypes:Creature Elf\nPT:1/1\nA:AB$ Animate | Cost$ 1 | Types$ Creature | Duration$ Permanent | Power$ 2 | Toughness$ 3\n";
    let found = abilities(text);
    assert_eq!(
        found[0]["Activated"]["effects"][0]["Continuous"]["affected"],
        "This"
    );
}

#[rstest]
#[case::flying(
    "Mode$ Continuous | Affected$ Creature.YouCtrl | AddKeyword$ Flying | Description$ x",
    true
)]
#[case::own_ability_grant(
    "Mode$ Continuous | Affected$ Creature.YouCtrl | AddAbility$ Draw | Description$ x",
    false
)]
fn an_animation_grants_a_static_ability_that_gives_nothing_in_turn(
    #[case] static_line: &str,
    #[case] read: bool,
) {
    let text = format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nA:AB$ Animate | Cost$ B | Defined$ Self | staticAbilities$ S1\nSVar:S1:{static_line}\nSVar:Draw:AB$ Draw | Cost$ T\n"
    );
    let found = abilities(&text);
    if read {
        let grant = &found[0]["Activated"]["effects"][0]["Continuous"]["mods"][0]["GrantAbility"];
        assert!(grant.get("Static").is_some(), "{found:?}");
    } else {
        assert!(found.is_empty() || !gaps(&text).is_empty());
    }
}

#[test]
fn a_perpetual_static_grant_is_a_gap() {
    let text = "Name:T\nTypes:Instant\nA:SP$ Animate | Defined$ Self | staticAbilities$ S1 | Duration$ Perpetual\nSVar:S1:Mode$ Continuous | Affected$ Card.Self | AddKeyword$ Flying\n";
    assert_eq!(gaps(text), ["duration:Perpetual"]);
}

fn token_card(text: &str) -> super::super::card::CardOut {
    use crate::forge::script::Script;
    let mut world = super::world();
    for (stem, face) in [
        (
            "w_aura",
            "Name:Test Aura\nTypes:Enchantment Aura\nK:Enchant:Permanent:permanent\n",
        ),
        (
            "role_test",
            "Name:Test Role\nTypes:Enchantment Aura Role\nK:Enchant:Creature:creature\n",
        ),
    ] {
        world.tokens.insert(
            stem.to_owned(),
            Script::parse(face).faces.into_iter().next().unwrap(),
        );
    }
    super::super::card::translate_script(&world, &Script::parse(text))
}

fn token_gaps(text: &str) -> Vec<String> {
    token_card(text).gaps().map(|gap| gap.0.clone()).collect()
}

#[test]
fn an_aura_token_is_created_and_attached_to_the_target() {
    let text = "Name:T\nTypes:Sorcery\nA:SP$ Token | TokenScript$ w_aura | TokenOwner$ You | AttachedTo$ Targeted | ValidTgts$ Permanent.Other\n";
    assert!(token_gaps(text).is_empty(), "{:?}", token_gaps(text));
    let found = token_card(text).faces.remove(0).abilities;
    let effects = &found[0]["Spell"]["effects"];
    let name = effects[0]["CreateToken"]["as"].clone();
    assert_eq!(effects[1]["Attach"]["what"], json!({ "Bound": name }));
    assert_eq!(effects[1]["Attach"]["to"], json!({"Target": 0}));
}

#[rstest]
#[case::role(
    "Name:T\nTypes:Sorcery\nA:SP$ Token | TokenScript$ role_test | TokenOwner$ You | AttachedTo$ Targeted | ValidTgts$ Creature.YouCtrl\n",
    "token:Role-rule"
)]
#[case::to_a_player(
    "Name:T\nTypes:Sorcery\nA:SP$ Token | TokenScript$ w_aura | TokenOwner$ You | AttachedTo$ TriggeredDefendingPlayer\n",
    "key:Token.AttachedTo=TriggeredDefendingPlayer"
)]
#[case::target_without_attach(
    "Name:T\nTypes:Sorcery\nA:SP$ Token | TokenScript$ w_aura | TokenOwner$ You | ValidTgts$ Creature\n",
    "key:Token.ValidTgts"
)]
fn a_token_attachment_the_rules_do_not_cover_is_a_gap(#[case] text: &str, #[case] gap: &str) {
    assert_eq!(token_gaps(text), [gap]);
}

const MIRROR: &str = "Name:Subject\nTypes:Sorcery\nManaCost:R\nA:SP$ DelayedTrigger | Mode$ SpellCast | ValidCard$ Instant,Sorcery | ValidActivatingPlayer$ You | ThisTurn$ True | Execute$ Copy\nSVar:Copy:DB$ CopySpellAbility | Defined$ TriggeredSpellAbility | MayChooseTarget$ True\n";

#[test]
fn the_next_spell_cast_is_copied_by_a_delayed_cast_trigger() {
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [card(MIRROR, CardType::Sorcery, None), burn("Bolt", 3)])
        .battlefield(0, [ready(mountain()), ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast_tapping("Subject", &["Mountain"]).unwrap();
    settle(&mut driver);
    let target = driver.target_player(1);
    driver.begin_cast("Bolt").unwrap();
    driver.choose_targets(&[target]).unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    for _ in 0..80 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively().unwrap();
    }
    assert_eq!(life(&driver, 1), 14);
}

#[rstest]
#[case::attack(
    "Card.Self+attacking",
    "Activate only if CARDNAME is attacking.",
    json!({"Compare": {"left": {"Count": ["This", "Attacking"]}, "op": "Ge", "right": 1}})
)]
#[case::entered(
    "Card.Self+ThisTurnEntered",
    "Activate only if CARDNAME entered this turn.",
    json!({"Compare": {"left": {"Count": ["This", "EnteredThisTurn"]}, "op": "Ge", "right": 1}})
)]
fn an_activate_only_if_self_line_is_a_condition_on_activation(
    #[case] valid: &str,
    #[case] text: &str,
    #[case] expected: Value,
) {
    let script = format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nA:AB$ Draw | Cost$ 1 | IsPresent$ {valid} | SpellDescription$ {text}\n"
    );
    assert_eq!(abilities(&script)[0]["Activated"]["condition"], expected);
}

#[test]
fn a_resolution_check_on_self_stays_a_gap() {
    let script = "Name:T\nTypes:Creature Elf\nPT:1/1\nA:AB$ Draw | Cost$ 1 | IsPresent$ Card.Self+tapped | SpellDescription$ If CARDNAME is tapped, draw a card.\n";
    assert_eq!(gaps(script), ["key:AB.IsPresent=Self"]);
}
