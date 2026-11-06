//! Goal: static parameters of round 3 piece P7 translate to the modifications the engine has, one table per rule, and a
//! translated rule changes a game the way the card prints.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::format_collect,
    clippy::single_match_else,
    clippy::manual_assert,
    clippy::len_zero
)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::scenario::ready;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn static_of(line: &str) -> Vec<Value> {
    abilities(&format!("Name:Subject\nTypes:Artifact\n{line}\n"))
}

#[rstest]
#[case::panharmonicon(
    "S:Mode$ Panharmonicon | ValidMode$ ChangesZone,ChangesZoneAll | ValidCard$ Permanent.YouCtrl | ValidCause$ Artifact,Creature | Destination$ Battlefield",
    json!({"TriggerAgain": {"cause": {"ZoneChange": {"what": [{"AnyOf": [{"Type": "Artifact"}, {"Type": "Creature"}]}], "to": "Battlefield"}}}})
)]
#[case::teysa_karlov_dying(
    "S:Mode$ Panharmonicon | ValidMode$ ChangesZone,ChangesZoneAll | ValidCard$ Permanent.YouCtrl | ValidCause$ Creature | Origin$ Battlefield | Destination$ Graveyard",
    json!({"TriggerAgain": {"cause": {"ZoneChange": {"what": [{"Type": "Creature"}], "from": "Battlefield", "to": "Graveyard"}}}})
)]
#[case::isshin_attacking(
    "S:Mode$ Panharmonicon | ValidMode$ Attacks,AttackersDeclared,AttackersDeclaredOneTarget | ValidCard$ Permanent.YouCtrl | ValidCause$ Creature",
    json!({"TriggerAgain": {"cause": {"Attacks": {"what": [{"Type": "Creature"}]}}}})
)]
#[case::veyran_casting_or_copying(
    "S:Mode$ Panharmonicon | ValidMode$ SpellCast,SpellCopy,SpellCastOrCopy | ValidCard$ Permanent.YouCtrl | ValidCause$ Instant,Sorcery | ValidActivator$ You",
    json!({"TriggerAgain": {"cause": {"AnyOf": [
        {"SpellCast": {"what": [{"AnyOf": [{"Type": "Instant"}, {"Type": "Sorcery"}]}], "by": "You"}},
        {"SpellCopied": {"what": [{"AnyOf": [{"Type": "Instant"}, {"Type": "Sorcery"}]}], "by": "You"}}]}}})
)]
#[case::krang_drawing(
    "S:Mode$ Panharmonicon | ValidMode$ Drawn | ValidCard$ Permanent.YouCtrl",
    json!({"TriggerAgain": {"cause": {"Drew": {}}}})
)]
#[case::dr_beverly_crusher_life(
    "S:Mode$ Panharmonicon | ValidMode$ LifeGained | ValidPlayer$ You | ValidCard$ Permanent.YouCtrl",
    json!({"TriggerAgain": {"cause": {"LifeGained": {"who": "You"}}}})
)]
fn a_panharmonicon_line_names_the_event_that_triggers_again(
    #[case] line: &str,
    #[case] rule: Value,
) {
    assert_eq!(static_of(line), vec![json!({"Rule": {"rule": rule}})]);
}

#[rstest]
#[case::no_event(
    "S:Mode$ Panharmonicon | ValidCard$ Creature.YouCtrl+Legendary | ValidCause$ Creature",
    "static:Panharmonicon-any-cause"
)]
#[case::unknown_event(
    "S:Mode$ Panharmonicon | ValidMode$ RoomEntered | ValidCard$ Dungeon.YouOwn",
    "static:Panharmonicon-mode=RoomEntered"
)]
#[case::unread_key(
    "S:Mode$ Panharmonicon | ValidMode$ ChangesZone | ValidCause$ Land | Destination$ Battlefield | ValidZone$ Command",
    "key:Static.Panharmonicon.ValidZone"
)]
fn a_panharmonicon_line_it_cannot_read_stays_a_gap_by_name(#[case] line: &str, #[case] gap: &str) {
    let text = format!("Name:Subject\nTypes:Artifact\n{line}\n");
    assert!(
        gaps(&text).iter().any(|found| found == gap),
        "{:?}",
        gaps(&text)
    );
    assert_eq!(abilities(&text), Vec::<Value>::new());
}

fn card_with(types: &[CardType], name: &str, script: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name).with_mana_cost(mtg_testkit::cards::cost(0, &[]));
    for kind in types {
        card = card.with_type(*kind);
    }
    if types.contains(&CardType::Creature) {
        card = card.with_power_toughness(1, 1);
    }
    for value in abilities(script) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

const ENTERS: &str = "Name:Imp\nTypes:Creature Imp\nPT:1/1\nT:Mode$ ChangesZone | ValidCard$ Card.Self | Origin$ Any | Destination$ Battlefield | Execute$ Gain | TriggerDescription$ When this enters, you gain 1 life.\nSVar:Gain:DB$ GainLife | LifeAmount$ 1\n";
const PANHARMONICON: &str = "Name:Echo\nTypes:Artifact\nS:Mode$ Panharmonicon | ValidMode$ ChangesZone,ChangesZoneAll | ValidCard$ Permanent.YouCtrl | ValidCause$ CAUSE | Destination$ Battlefield\n";

/// Life of seat 0 after it casts the Imp with the Echo in play and everything has resolved.
fn life_after_casting(cause: &str) -> i32 {
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [ready(card_with(
                &[CardType::Artifact],
                "Echo",
                &PANHARMONICON.replace("CAUSE", cause),
            ))],
        )
        .hand(0, [card_with(&[CardType::Creature], "Imp", ENTERS)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let start = life(&driver);
    driver.cast("Imp").unwrap();
    for _ in 0..50 {
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        if idle
            && matches!(
                driver.engine().decision().map(|d| &d.kind),
                Some(DecisionKind::Priority(_))
            )
        {
            return life(&driver) - start;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack never emptied");
}

fn life(driver: &Driver) -> i32 {
    driver
        .state()
        .player(PlayerId::from_index(0))
        .unwrap()
        .life()
}

#[rstest]
#[case::an_entering_creature_triggers_twice("Artifact,Creature", 2)]
#[case::a_cause_the_rule_does_not_name_triggers_once("Artifact", 1)]
fn the_translated_rule_doubles_only_the_triggers_its_cause_names(
    #[case] cause: &str,
    #[case] gained: i32,
) {
    assert_eq!(life_after_casting(cause), gained);
}

fn stats_card(pt: &str, line: &str) -> String {
    format!(
        "Name:Subject\nTypes:Artifact Creature Construct\nPT:{pt}\n{line}\nSVar:X:Count$Valid Artifact.YouCtrl\n"
    )
}

#[rstest]
#[case::bronze_guardian_power("*/5", "SetPower$ X", json!({"power": {"Count": [{"Type": "Artifact"}, {"ControlledBy": "You"}]}, "toughness": 5}))]
#[case::aven_trailblazer_toughness("2/*", "SetToughness$ X", json!({"power": 2, "toughness": {"Count": [{"Type": "Artifact"}, {"ControlledBy": "You"}]}}))]
fn a_defining_line_that_sets_one_half_keeps_the_printed_other_half(
    #[case] pt: &str,
    #[case] set: &str,
    #[case] stats: Value,
) {
    let line = format!("S:Mode$ Continuous | CharacteristicDefining$ True | {set}");
    assert_eq!(
        abilities(&stats_card(pt, &line)),
        vec![json!({"Static": {"affected": "This", "mods": [{"SetPT": stats}], "defining": true}})]
    );
}

#[rstest]
#[case::plus_star("1+*/2", "CharacteristicDefining$ True | SetPower$ X")]
#[case::both_stars("*/*", "CharacteristicDefining$ True | SetPower$ X")]
#[case::wrong_half("*/3", "CharacteristicDefining$ True | SetToughness$ X")]
#[case::not_defining("6/5", "Affected$ Creature.OppCtrl | SetToughness$ 1")]
fn a_line_with_one_half_it_cannot_complete_stays_a_gap(#[case] pt: &str, #[case] set: &str) {
    let text = stats_card(pt, &format!("S:Mode$ Continuous | {set}"));
    assert!(
        gaps(&text).contains(&"key:Static.Continuous.half-SetPT".to_owned()),
        "{:?}",
        gaps(&text)
    );
}

#[rstest]
#[case::no_other_artifact(0, (1, 5))]
#[case::two_other_artifacts(2, (3, 5))]
fn a_half_set_defining_power_follows_the_count_in_a_game(
    #[case] others: usize,
    #[case] stats: (i32, i32),
) {
    let line = "S:Mode$ Continuous | CharacteristicDefining$ True | SetPower$ X";
    let mut card = CardDefinition::new("Guardian")
        .with_type(CardType::Artifact)
        .with_type(CardType::Creature)
        .with_power_toughness(0, 5);
    for value in abilities(&stats_card("*/5", line)) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    let rock = || card_with(&[CardType::Artifact], "Rock", "Name:Rock\nTypes:Artifact\n");
    let mut seat = vec![ready(Arc::new(card))];
    seat.extend((0..others).map(|_| ready(rock())));
    let driver =
        Driver::from_scenario(Scenario::new(2).at(1, 0, Step::Main1).battlefield(0, seat)).unwrap();
    let id = driver.id_of("Guardian").unwrap();
    let chars = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    assert_eq!((chars.power.unwrap(), chars.toughness.unwrap()), stats);
}

fn continuous(line: &str) -> Vec<Value> {
    abilities(&format!("Name:Subject\nTypes:Enchantment Aura\n{line}\n"))
}

#[rstest]
#[case::darkest_hour(
    "S:Mode$ Continuous | Affected$ Creature | SetColor$ Black",
    json!({"Static": {"affected": {"Each": [{"Type": "Creature"}]}, "mods": [{"SetColors": ["Black"]}]}})
)]
#[case::all_slivers_colorless(
    "S:Mode$ Continuous | Affected$ Goblin | SetColor$ Colorless",
    json!({"Static": {"affected": {"Each": [{"Subtype": "Goblin"}]}, "mods": [{"SetColors": []}]}})
)]
#[case::every_color(
    "S:Mode$ Continuous | AffectedDefined$ Self | SetColor$ All",
    json!({"Static": {"affected": "This", "mods": [{"SetColors": ["White", "Blue", "Black", "Red", "Green"]}]}})
)]
#[case::enchanted_creature_is_a_wizard_elf(
    "S:Mode$ Continuous | AffectedDefined$ Enchanted | Affected$ Creature | AddType$ Wizard & Elf | RemoveCreatureTypes$ True",
    json!({"Static": {"affected": "Attached", "mods": [{"SetCreatureTypes": ["Wizard", "Elf"]}]}})
)]
#[case::types_join_without_the_removal(
    "S:Mode$ Continuous | AffectedDefined$ Enchanted | Affected$ Creature | AddType$ Wizard",
    json!({"Static": {"affected": "Attached", "mods": [{"AddSubtype": "Wizard"}]}})
)]
#[case::equipped_creature_is_every_type(
    "S:Mode$ Continuous | AffectedDefined$ Enchanted | Affected$ Creature | AddPower$ 1 | AddToughness$ 1 | AddAllCreatureTypes$ True",
    json!({"Static": {"affected": "Attached", "mods": [{"ModifyPT": {"power": 1, "toughness": 1}}, "AddAllCreatureTypes"]}})
)]
fn a_continuous_line_changes_colours_and_creature_types(
    #[case] line: &str,
    #[case] expected: Value,
) {
    assert_eq!(continuous(line), vec![expected]);
}

#[rstest]
#[case::chosen_colour(
    "S:Mode$ Continuous | AffectedDefined$ Self | SetColor$ ChosenColor",
    "key:Static.Continuous.SetColor=ChosenColor"
)]
#[case::chosen_type(
    "S:Mode$ Continuous | AffectedDefined$ Enchanted | AddType$ ChosenType | RemoveCreatureTypes$ True",
    "key:Static.Continuous.AddType=ChosenType"
)]
#[case::defining_a_keyword(
    "S:Mode$ Continuous | Affected$ Card.Self | CharacteristicDefining$ True | AddKeyword$ Flash",
    "key:Static.Continuous.defining-other-than-stats"
)]
fn a_colour_or_type_word_it_cannot_read_stays_a_gap_by_name(#[case] line: &str, #[case] gap: &str) {
    let text = format!("Name:Subject\nTypes:Enchantment Aura\n{line}\n");
    assert!(
        gaps(&text).iter().any(|found| found == gap),
        "{:?}",
        gaps(&text)
    );
}

#[rstest]
#[case::black(
    "S:Mode$ Continuous | Affected$ Creature | SetColor$ Black",
    "Black",
    ""
)]
#[case::colorless(
    "S:Mode$ Continuous | Affected$ Creature | SetColor$ Colorless",
    "",
    ""
)]
#[case::creature_types_replaced(
    "S:Mode$ Continuous | Affected$ Creature | AddType$ Wizard | RemoveCreatureTypes$ True",
    "Red",
    "Wizard"
)]
fn a_translated_static_recolours_and_retypes_a_creature_in_a_game(
    #[case] line: &str,
    #[case] colors: &str,
    #[case] types: &str,
) {
    let field = card_with(
        &[CardType::Enchantment],
        "Field",
        &format!("Name:Field\nTypes:Enchantment\n{line}\n"),
    );
    let goblin = CardDefinition::new("Pest")
        .with_type(CardType::Creature)
        .with_subtype("Goblin")
        .with_mana_cost(mtg_testkit::cards::cost(0, &[mtg_core::color::Color::Red]))
        .with_power_toughness(1, 1);
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(field), ready(Arc::new(goblin))]);
    let driver = Driver::from_scenario(scenario).unwrap();
    let id = driver.id_of("Pest").unwrap();
    let chars = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    let subtypes: Vec<String> = chars.subtypes.iter().map(|s| s.0.clone()).collect();
    let shown: String = chars
        .colors
        .iter()
        .map(|color| format!("{color:?}"))
        .collect();
    assert_eq!(
        (shown.as_str(), subtypes.join(" ")),
        (colors, types.to_owned())
    );
}

const GRANTS_A_STATIC: &str = "Name:Banner\nTypes:Enchantment\nS:Mode$ Continuous | Affected$ Creature.YouCtrl | AddStaticAbility$ Boost | Description$ Creatures you control have \"This creature gets +1/+1.\"\nSVar:Boost:Mode$ Continuous | AffectedDefined$ Self | AddPower$ 1 | AddToughness$ 1\n";

#[test]
fn a_granted_static_line_is_a_static_the_creatures_gain() {
    assert_eq!(
        abilities(GRANTS_A_STATIC),
        vec![json!({"Static": {
            "affected": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]},
            "mods": [{"GrantAbility": {"Static": {"affected": "This", "mods": [{"ModifyPT": {"power": 1, "toughness": 1}}]}}}],
        }})]
    );
}

#[rstest]
#[case::a_rule(
    "Mode$ CantGainLife | ValidPlayer$ You",
    "key:Static.Continuous.AddStaticAbility-not-a-static"
)]
#[case::grants_in_turn(
    "Mode$ Continuous | AffectedDefined$ Self | AddKeyword$ Flying | AddStaticAbility$ Boost",
    "key:Static.Continuous.AddStaticAbility-shape"
)]
#[case::a_missing_part("", "key:Static.Continuous.AddStaticAbility-not-a-part")]
fn a_granted_static_it_cannot_carry_stays_a_gap_by_name(#[case] part: &str, #[case] gap: &str) {
    let part = if part.is_empty() {
        String::new()
    } else {
        format!("SVar:Boost:{part}\n")
    };
    let text = format!(
        "Name:Banner\nTypes:Enchantment\nS:Mode$ Continuous | Affected$ Creature.YouCtrl | AddStaticAbility$ Boost\n{part}"
    );
    assert!(
        gaps(&text).iter().any(|found| found == gap),
        "{:?}",
        gaps(&text)
    );
}

#[test]
fn a_creature_with_a_granted_static_gets_its_modification_in_a_game() {
    let banner = card_with(&[CardType::Enchantment], "Banner", GRANTS_A_STATIC);
    let scenario = Scenario::new(2).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(banner),
            ready(mtg_testkit::cards::free_creature("Pest", 1, 1)),
        ],
    );
    let driver = Driver::from_scenario(scenario).unwrap();
    let id = driver.id_of("Pest").unwrap();
    let chars = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(2), Some(2)));
}

#[rstest]
#[case::aspect_of_wolf(
    "S:Mode$ Continuous | AffectedDefined$ AttachedBy Self | AddPower$ 4 | AddToughness$ 4",
    json!({"Static": {"affected": "Attached", "mods": [{"ModifyPT": {"power": 4, "toughness": 4}}]}})
)]
#[case::with_a_type_word(
    "S:Mode$ Continuous | AffectedDefined$ AttachedBy Self | Affected$ Creature | AddPower$ 1 | AddToughness$ 1",
    json!({"Static": {"affected": "Attached", "mods": [{"ModifyPT": {"power": 1, "toughness": 1}}]}})
)]
fn what_a_permanent_is_attached_to_is_the_attached_object(
    #[case] line: &str,
    #[case] expected: Value,
) {
    let found = abilities(&format!("Name:Subject\nTypes:Enchantment Aura\n{line}\n"));
    assert_eq!(found, vec![expected]);
}

#[test]
fn an_aura_that_boosts_what_it_is_attached_to_boosts_that_creature_in_a_game() {
    let line =
        "S:Mode$ Continuous | AffectedDefined$ AttachedBy Self | AddPower$ 4 | AddToughness$ 4";
    let aura = Arc::new(
        CardDefinition::new("Wolf")
            .with_type(CardType::Enchantment)
            .with_subtype("Aura")
            .with_rules(
                serde_json::from_value::<Ability>(
                    abilities(&format!("Name:Wolf\nTypes:Enchantment Aura\n{line}\n")).remove(0),
                )
                .unwrap(),
            ),
    );
    let scenario = Scenario::new(2).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(mtg_testkit::cards::free_creature("Pest", 1, 1)),
            mtg_testkit::scenario::attached_to(ready(aura), 0, 0),
        ],
    );
    let driver = Driver::from_scenario(scenario).unwrap();
    let id = driver.id_of("Pest").unwrap();
    let chars = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(5), Some(5)));
}

#[rstest]
#[case::cast_from_the_graveyard_for_life(
    "S:Mode$ Continuous | Affected$ Artifact.YouCtrl | MayPlay$ True | EffectZone$ Battlefield | AffectedZone$ Graveyard | RaiseCost$ PayLife<3> | Description$ You may cast artifact spells from your graveyard by paying 3 life.",
    json!({"Rule": {"rule": {"MayCast": {"what": [{"Type": "Artifact"}], "from": "Graveyard", "additional": [{"PayLife": 3}]}}}})
)]
#[case::discarding_a_card(
    "S:Mode$ Continuous | Affected$ Creature.YouOwn | MayPlay$ True | EffectZone$ Battlefield | AffectedZone$ Graveyard | RaiseCost$ Discard<1/Card> | Description$ You may cast creature spells from your graveyard by discarding a card.",
    json!({"Rule": {"rule": {"MayCast": {"what": [{"Type": "Creature"}], "from": "Graveyard", "additional": [{"Discard": {}}]}}}})
)]
fn a_permission_to_cast_with_an_extra_cost_keeps_the_cost(
    #[case] line: &str,
    #[case] expected: Value,
) {
    assert_eq!(static_of(line), vec![expected]);
}

#[test]
fn an_extra_cost_that_is_not_read_stays_a_gap_by_name() {
    let line = "S:Mode$ Continuous | Affected$ Artifact.YouCtrl | MayPlay$ True | EffectZone$ Battlefield | AffectedZone$ Graveyard | RaiseCost$ Frobnicate<1> | Description$ x";
    let text = format!("Name:Subject\nTypes:Artifact\n{line}\n");
    assert!(
        gaps(&text).iter().any(|found| found.starts_with("cost:")),
        "{:?}",
        gaps(&text)
    );
}

#[test]
fn a_cast_from_the_graveyard_pays_the_extra_life_in_a_game() {
    let line = "S:Mode$ Continuous | Affected$ Artifact.YouCtrl | MayPlay$ True | EffectZone$ Battlefield | AffectedZone$ Graveyard | RaiseCost$ PayLife<3> | Description$ x";
    let tomb = card_with(
        &[CardType::Enchantment],
        "Tomb",
        &format!("Name:Tomb\nTypes:Enchantment\n{line}\n"),
    );
    let idol = card_with(&[CardType::Artifact], "Idol", "Name:Idol\nTypes:Artifact\n");
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(tomb)])
        .graveyard(0, [idol]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let start = life(&driver);
    driver.cast("Idol").unwrap();
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively().unwrap();
    }
    assert_eq!(life(&driver), start - 3);
}

#[test]
fn a_blocker_that_cant_block_a_kind_translates_to_blocks_only_the_rest() {
    let line =
        "S:Mode$ CantBlockBy | ValidAttacker$ Creature.powerGE2 | ValidBlocker$ Creature.Self";
    let found = abilities(&format!(
        "Name:Subject\nTypes:Creature Elf\nPT:1/1\n{line}\n"
    ));
    let outside = json!({"AnyOf": [
        {"Not": {"Type": "Creature"}},
        {"Not": {"Compare": {"op": "Ge", "stat": "Power", "to": 2}}},
    ]});
    assert_eq!(
        found,
        vec![json!({"Static": {"affected": "This", "mods": [{"CanBlockOnly": [outside]}]}})]
    );
}

/// Whether the Wall of seat 1, which can't block creatures `attacker` names, may block the Runner of the given power.
fn wall_can_block(attacker: &str, power: i32) -> bool {
    let wall = CardDefinition::new("Wall")
        .with_type(CardType::Creature)
        .with_power_toughness(0, 9);
    let line =
        format!("S:Mode$ CantBlockBy | ValidAttacker$ {attacker} | ValidBlocker$ Creature.Self");
    let script = format!("Name:Wall\nTypes:Creature Wall\nPT:0/9\n{line}\n");
    let wall = abilities(&script).into_iter().fold(wall, |card, rule| {
        card.with_rules(serde_json::from_value::<Ability>(rule).unwrap())
    });
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [ready(mtg_testkit::cards::free_creature("Runner", power, 3))],
        )
        .battlefield(1, [ready(Arc::new(wall))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Runner", 1)]).unwrap();
    for _ in 0..40 {
        match &driver.decision().unwrap().kind {
            DecisionKind::DeclareBlockers(_) => {
                return driver.block(&[("Wall", "Runner")]).is_ok();
            }
            _ if driver.state().turn().step() == Step::EndCombat => return false,
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("combat did not reach blocks");
}

#[rstest]
#[case::power_two_attacker_is_refused("Creature.powerGE2", 2, false)]
#[case::power_one_attacker_is_blocked("Creature.powerGE2", 1, true)]
#[case::power_one_at_most_two_is_refused("Creature.powerLE2", 1, false)]
#[case::power_three_above_the_limit_is_blocked("Creature.powerLE2", 3, true)]
fn a_blocker_cant_block_the_attackers_its_line_names(
    #[case] attacker: &str,
    #[case] power: i32,
    #[case] blocks: bool,
) {
    assert_eq!(wall_can_block(attacker, power), blocks);
}

#[rstest]
#[case::doran_the_siege_tower("Creature.YouCtrl", json!("You"))]
#[case::every_creature("Creature", json!("EachPlayer"))]
fn assigning_damage_by_toughness_of_every_creature_is_a_rule_of_the_players(
    #[case] valid: &str,
    #[case] who: Value,
) {
    let text = format!(
        "Name:Subject\nTypes:Creature Elf\nPT:0/5\nS:Mode$ CombatDamageToughness | ValidCard$ {valid}\n"
    );
    assert_eq!(
        abilities(&text),
        vec![json!({"Rule": {"rule": {"ToughnessAssignsDamage": who}}})]
    );
}

/// The damage the Runner (0 power, 5 toughness) of seat 0 deals to seat 1 when it attacks unblocked, with a Doran line or not.
fn damage_dealt_by_a_wall_of_toughness(line: &str) -> i32 {
    let script = format!("Name:Runner\nTypes:Creature Wall\nPT:0/5\n{line}\n");
    let card = abilities(&script).into_iter().fold(
        CardDefinition::new("Runner")
            .with_type(CardType::Creature)
            .with_power_toughness(0, 5),
        |card, rule| card.with_rules(serde_json::from_value::<Ability>(rule).unwrap()),
    );
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(Arc::new(card))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let before = life_of(&driver, 1);
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Runner", 1)]).unwrap();
    for _ in 0..60 {
        if driver.state().turn().step() == Step::EndCombat {
            break;
        }
        driver.answer_passively().unwrap();
    }
    before - life_of(&driver, 1)
}

fn life_of(driver: &Driver, seat: usize) -> i32 {
    driver
        .state()
        .player(PlayerId::from_index(seat))
        .unwrap()
        .life()
}

#[rstest]
#[case::with_the_line("S:Mode$ CombatDamageToughness | ValidCard$ Creature.YouCtrl", 5)]
#[case::without_it("", 0)]
fn the_translated_line_makes_a_creature_hit_for_its_toughness(
    #[case] line: &str,
    #[case] dealt: i32,
) {
    assert_eq!(damage_dealt_by_a_wall_of_toughness(line), dealt);
}

const OOZE: &str = "Name:Necrotic Ooze\nTypes:Creature Ooze\nPT:2/2\nS:Mode$ Continuous | AffectedDefined$ Self | EffectZone$ Battlefield | GainsAbilitiesOf$ Creature | GainsAbilitiesOfZones$ Graveyard | Description$ As long as CARDNAME is on the battlefield, it has all activated abilities of all creature cards in all graveyards.\n";

const SHAMAN: &str = "Name:Shaman\nTypes:Creature Elf\nPT:1/1\nA:AB$ GainLife | Cost$ T | LifeAmount$ 3 | SpellDescription$ You gain 3 life.\n";

#[rstest]
#[case::creature_cards_in_graveyards(
    "GainsAbilitiesOf$ Creature | GainsAbilitiesOfZones$ Graveyard",
    json!({"GainAbilitiesOf": {"Each": [{"Type": "Creature"}, {"In": "Graveyard"}]}})
)]
#[case::creature_cards_in_your_graveyard(
    "GainsAbilitiesOf$ Creature.YouOwn | GainsAbilitiesOfZones$ Graveyard",
    json!({"GainAbilitiesOf": {"Each": [{"Type": "Creature"}, {"OwnedBy": "You"}, {"In": "Graveyard"}]}})
)]
fn a_static_that_has_the_abilities_of_other_cards_names_them_by_zone(
    #[case] words: &str,
    #[case] modification: Value,
) {
    let line =
        format!("S:Mode$ Continuous | AffectedDefined$ Self | EffectZone$ Battlefield | {words}");
    assert_eq!(
        abilities(&format!("Name:Ooze\nTypes:Creature Ooze\nPT:2/2\n{line}\n")),
        vec![json!({"Static": {"affected": "This", "mods": [modification]}})]
    );
}

#[rstest]
#[case::on_the_battlefield(
    "GainsAbilitiesOf$ Creature.Other",
    "key:Static.Continuous.GainsAbilitiesOf-battlefield"
)]
#[case::in_two_zones(
    "GainsAbilitiesOf$ Creature | GainsAbilitiesOfZones$ Graveyard,Exile",
    "key:Static.Continuous.GainsAbilitiesOfZones=Graveyard,Exile"
)]
#[case::loyalty_only(
    "GainsAbilitiesOf$ Planeswalker.Other | GainsValidAbilities$ Activated.Loyalty | GainsAbilitiesOfZones$ Graveyard",
    "key:Static.Continuous.GainsValidAbilities"
)]
#[case::triggers_too(
    "GainsAbilitiesOf$ Card.ExiledWithSource | GainsTriggerAbsOf$ Card.ExiledWithSource | GainsAbilitiesOfZones$ Exile",
    "key:Static.Continuous.GainsTriggerAbsOf"
)]
fn a_static_with_abilities_it_cannot_read_stays_a_gap_by_name(
    #[case] words: &str,
    #[case] gap: &str,
) {
    let line =
        format!("S:Mode$ Continuous | AffectedDefined$ Self | EffectZone$ Battlefield | {words}");
    let text = format!("Name:Ooze\nTypes:Creature Ooze\nPT:2/2\n{line}\n");
    assert!(
        gaps(&text).iter().any(|found| found == gap),
        "{:?}",
        gaps(&text)
    );
}

fn definition(script: &str, name: &str) -> Arc<CardDefinition> {
    let card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2);
    Arc::new(abilities(script).into_iter().fold(card, |card, rule| {
        card.with_rules(serde_json::from_value::<Ability>(rule).unwrap())
    }))
}

/// Seat 0's Ooze with the Shaman in the graveyard of `in_seat` (or on the battlefield of seat 1): the abilities the Ooze may activate.
fn ooze_abilities(shaman_in_graveyard_of: Option<usize>) -> usize {
    let ooze = definition(OOZE, "Necrotic Ooze");
    let shaman = definition(SHAMAN, "Shaman");
    let mut scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(ooze)]);
    scenario = match shaman_in_graveyard_of {
        Some(seat) => scenario.graveyard(seat, [shaman]),
        None => scenario.battlefield(1, [ready(shaman)]),
    };
    let driver = Driver::from_scenario(scenario).unwrap();
    driver.offered_abilities("Necrotic Ooze").unwrap().len()
}

#[rstest]
#[case::a_card_in_your_graveyard(Some(0), 1)]
#[case::a_card_in_an_opponents_graveyard(Some(1), 1)]
#[case::a_creature_on_the_battlefield_gives_nothing(None, 0)]
fn the_ooze_may_activate_the_abilities_of_creature_cards_in_graveyards(
    #[case] graveyard: Option<usize>,
    #[case] offered: usize,
) {
    assert_eq!(ooze_abilities(graveyard), offered);
}

#[test]
fn an_ability_gained_from_a_graveyard_card_is_paid_by_the_ooze_and_resolves() {
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(definition(OOZE, "Necrotic Ooze"))])
        .graveyard(1, [definition(SHAMAN, "Shaman")]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let start = life(&driver);
    driver.begin_activation("Necrotic Ooze").unwrap();
    for _ in 0..30 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && life(&driver) != start
        {
            break;
        }
        driver.answer_passively().unwrap();
    }
    assert_eq!(life(&driver), start + 3);
    let ooze = driver.id_of("Necrotic Ooze").unwrap();
    assert!(
        driver
            .state()
            .object(ooze)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
}
