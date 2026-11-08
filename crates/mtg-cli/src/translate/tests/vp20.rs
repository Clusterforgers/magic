//! Goal: every atom of the filter grammar that round 20 maps reads as its selector, reads back as the Oracle text that
//! says it, and reads `Differs` beside a text with a narrower or wider selector (a shared color has no writer
//! yet and stays `Unchecked`, never `Accepted`); every atom of the table of gaps stays a
//! gap named by its row.
use std::collections::BTreeSet;

use rstest::rstest;
use serde_json::{Value, json};

use mtg_cards::load::Library;
use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::{PrintedCard, PrintedFace};

use super::super::card::translate_script;
use super::super::ctx::World;
use super::super::filter::alternatives;
use super::super::valid_grammar::UNMAPPED;
use super::world;
use crate::forge::script::Script;

/// A world that also knows the card names and the party types the cases use.
fn wider() -> World {
    let mut world = world();
    world
        .subtypes
        .extend(["Cleric", "Warrior", "Swamp", "Druid", "Saga"].map(str::to_owned));
    world
        .names
        .extend(["Alpha, the Test", "Aether Burst"].map(str::to_owned));
    world
}

fn abilities(text: &str) -> Vec<Value> {
    translate_script(&wider(), &Script::parse(text))
        .faces
        .into_iter()
        .next()
        .map(|face| face.abilities)
        .unwrap_or_default()
}

fn gaps(text: &str) -> Vec<String> {
    translate_script(&wider(), &Script::parse(text))
        .gaps()
        .map(|gap| gap.0.clone())
        .collect()
}

/// The predicates of `Count$Valid <valid>` in a spell's damage, with the script's extra lines.
fn count_of(valid: &str, extra: &str) -> Value {
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ DealDamage | ValidTgts$ Any | NumDmg$ Y\nSVar:Y:Count$Valid {valid}\n{extra}"
    );
    let found = abilities(&text);
    assert_eq!(gaps(&text), Vec::<String>::new(), "{valid}");
    found[0]["Spell"]["effects"][0]["DealDamage"]["amount"]["Count"].clone()
}

fn source_stat(left: &str, op: &str, right: &str) -> Value {
    json!({"Test": {"left": {"Stat": {"of": "Candidate", "stat": left}}, "op": op,
        "right": {"Stat": {"of": "This", "stat": right}}}})
}

#[rstest]
#[case::kicked("Card.Kicked", "", json!("Kicked"))]
#[case::cast_for_mayhem("Card.CastSa Spell.Mayhem", "", json!({"CastWith": "Mayhem"}))]
#[case::cast_with_treasure_mana(
    "Card.CastSa Spell.ManaFromTreasure",
    "",
    json!({"SpentFrom": {"from": {"subtypes": ["Treasure"]}, "least": 1}})
)]
#[case::cast_for_no_mana(
    "Creature.CastSa Spell.ManaSpent EQ0",
    "",
    json!({"Compare": {"stat": "ManaSpent", "op": "Eq", "to": 0}})
)]
#[case::legendary_equipment(
    "Creature.EquippedByValid Equipment.Legendary",
    "",
    json!({"HasAttached": [{"Subtype": "Equipment"}, {"Supertype": "Legendary"}]})
)]
#[case::color_of_the_equipped("Creature.SharesColorWith Equipped", "", json!({"SharesColor": "Attached"}))]
#[case::color_of_your_legend(
    "Card.SharesColorWith Valid Creature.Legendary+YouCtrl",
    "",
    json!({"SharesColor": {"Each": [{"Type": "Creature"}, {"Supertype": "Legendary"}, {"ControlledBy": "You"}]}})
)]
#[case::colors_against_a_part(
    "Creature.numColorsEQX",
    "SVar:X:Number$2\n",
    json!({"Compare": {"stat": "Colors", "op": "Eq", "to": 2}})
)]
#[case::counters_against_a_part(
    "Artifact.counters_GEX_CHARGE",
    "SVar:X:Number$20\n",
    json!({"CounterCount": {"kind": "charge", "op": "Ge", "to": 20}})
)]
#[case::greatest_of_a_kind(
    "Creature.greatestCMC_Creature",
    "",
    json!({"GreatestAmong": {"stat": "ManaValue", "group": [{"Type": "Creature"}]}})
)]
#[case::greatest_artifact(
    "Artifact.greatestCMC_Artifact+YouCtrl",
    "",
    json!({"GreatestAmong": {"stat": "ManaValue", "group": [{"Type": "Artifact"}]}})
)]
#[case::lowest_nonland(
    "Permanent.nonLand+lowestCMC",
    "",
    json!({"LeastAmong": {"stat": "ManaValue", "group": [{"Not": {"Type": "Land"}}]}})
)]
#[case::islandwalk(
    "Creature.hasKeywordLandwalk:Island",
    "",
    json!({"HasKeyword": {"Landwalk": [{"Subtype": "Island"}]}})
)]
#[case::enchant_creature(
    "Aura.hasKeywordEnchant:Creature",
    "",
    json!({"HasKeyword": {"Enchant": [{"Type": "Creature"}]}})
)]
#[case::attacking_their_planeswalkers(
    "Creature.attacking Valid Planeswalker.OppCtrl",
    "",
    json!({"AttackingPlaneswalkerOf": "Opponents"})
)]
#[case::party(
    "Creature.Party",
    "",
    json!({"AnyOf": [{"Subtype": "Cleric"}, {"Subtype": "Rogue"}, {"Subtype": "Warrior"}, {"Subtype": "Wizard"}]})
)]
#[case::name_with_a_comma("Card.namedAlpha, the Test", "", json!({"Named": "Alpha, the Test"}))]
#[case::name_with_underscores("Card.namedAether_Burst", "", json!({"Named": "Aether Burst"}))]
#[case::historic_base(
    "Historic",
    "",
    json!({"AnyOf": [{"Type": "Artifact"}, {"Supertype": "Legendary"}, {"Subtype": "Saga"}]})
)]
#[case::outlaw_base(
    "Outlaw",
    "",
    json!({"AnyOf": [{"Subtype": "Assassin"}, {"Subtype": "Mercenary"}, {"Subtype": "Pirate"}, {"Subtype": "Rogue"}, {"Subtype": "Warlock"}]})
)]
#[case::self_base("Self", "", json!("This"))]
fn a_mapped_atom_reads_as_its_predicate(
    #[case] valid: &str,
    #[case] extra: &str,
    #[case] expected: Value,
) {
    let found = count_of(valid, extra);
    let preds = found.as_array().cloned().unwrap_or_default();
    assert!(preds.contains(&expected), "{valid}: {found}");
}

#[rstest]
#[case::power_below("Creature.powerLTCount$CardPower", source_stat("Power", "Lt", "Power"))]
#[case::mana_value_at_most(
    "Creature.cmcLECount$CardManaCost",
    source_stat("ManaValue", "Le", "ManaValue")
)]
#[case::toughness_above(
    "Creature.toughnessGTCount$CardToughness",
    source_stat("Toughness", "Gt", "Toughness")
)]
fn a_stat_against_the_source_reads_as_a_test(#[case] valid: &str, #[case] expected: Value) {
    let text = format!("Name:T\nTypes:Sorcery\nA:SP$ Destroy | ValidTgts$ {valid}\n");
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = json!(abilities(&text)).to_string();
    assert!(found.contains(&expected.to_string()), "{found}");
}

#[rstest]
#[case::a_name_keeps_its_comma("Card.namedAlpha, the Test", &["Card.namedAlpha, the Test"])]
#[case::a_name_then_another_alternative(
    "Card.namedAlpha, the Test,Land.Basic",
    &["Card.namedAlpha, the Test", "Land.Basic"]
)]
#[case::plain_alternatives("Instant.YouCtrl,Sorcery.YouCtrl", &["Instant.YouCtrl", "Sorcery.YouCtrl"])]
#[case::a_spaced_alternative_without_a_name("Creature, Land", &["Creature", " Land"])]
fn alternatives_split_at_commas_outside_names(#[case] spec: &str, #[case] expected: &[&str]) {
    assert_eq!(alternatives(spec), expected);
}

#[rstest]
#[case::party_needs_its_types("Creature.Party", "valid:Party")]
#[case::color_of_mana(
    "Card.SharesColorWith ActivationColor",
    "valid:SharesColorWith ActivationColor"
)]
#[case::color_of_a_bare_word("Card.SharesColorWith", "valid:SharesColorWith")]
#[case::a_negated_cast_fact(
    "Card.CastSa Spell.!ManaFromTreasure",
    "valid:CastSa Spell.!ManaFromTreasure"
)]
#[case::a_kind_that_is_no_card_type("Creature.greatestCMC_Bogus", "valid:greatestCMC_Bogus")]
#[case::counters_against_an_amount("Creature.counters_GEX_P1P1", "valid:counters_GEX_P1P1")]
fn a_near_miss_stays_a_gap(#[case] valid: &str, #[case] gap: &str) {
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ DealDamage | ValidTgts$ Any | NumDmg$ Y\nSVar:Y:Count$Valid {valid}\nSVar:X:Count$CardPower\n"
    );
    let found = translate_script(&world(), &Script::parse(&text))
        .gaps()
        .map(|gap| gap.0.clone())
        .collect::<Vec<_>>();
    assert!(found.iter().any(|name| name == gap), "{valid}: {found:?}");
}

#[test]
fn every_row_of_the_table_of_gaps_stays_a_gap_named_by_its_row() {
    let words: BTreeSet<&str> = UNMAPPED.iter().map(|row| row.word).collect();
    assert_eq!(words.len(), UNMAPPED.len(), "a word has two rows");
    for row in UNMAPPED {
        let text = format!(
            "Name:T\nTypes:Sorcery\nA:SP$ DealDamage | ValidTgts$ Any | NumDmg$ Y\nSVar:Y:Count$Valid Card.{}\n",
            row.word
        );
        let key = format!("valid:{}", row.word);
        assert!(
            gaps(&text).contains(&key),
            "{}: {:?}",
            row.word,
            gaps(&text)
        );
    }
}

#[test]
fn an_argument_of_a_row_is_named_by_the_row() {
    let text = "Name:T\nTypes:Sorcery\nA:SP$ DealDamage | ValidTgts$ Any | NumDmg$ Y\nSVar:Y:Count$Valid Land.canProduceManaColor Colorless\n";
    assert_eq!(gaps(text), vec!["valid:canProduceManaColor".to_owned()]);
}

#[test]
fn every_row_of_the_table_of_gaps_is_in_use_in_the_checkout() {
    let checkout = std::path::Path::new(&std::env::var("HOME").unwrap_or_default())
        .join(".cache/mtg-cli/forge");
    let Ok(scripts) = crate::forge::script::read_checkout(&checkout) else {
        return;
    };
    let text = format!("{scripts:?}");
    for row in UNMAPPED {
        assert!(text.contains(row.word), "{} is in no script", row.word);
    }
}

#[test]
fn a_party_choice_stays_a_gap() {
    let text = "Name:T\nTypes:Sorcery\nA:SP$ ChooseCard | Defined$ Player | Choices$ Creature | ChooseEach$ Party | Reveal$ True\n";
    assert!(
        gaps(text).iter().any(|gap| gap == "valid-base:Party"),
        "{:?}",
        gaps(text)
    );
}

#[test]
fn the_trigger_object_base_names_the_cards_that_share_its_name() {
    let text = "Name:T\nTypes:Enchantment\nT:Mode$ SpellCast | ValidCard$ Creature | TriggerZones$ Battlefield | Execute$ TrigReturn | TriggerDescription$ Whenever a player casts a creature spell, return all cards with the same name as that spell from graveyards to the battlefield.\nSVar:TrigReturn:DB$ ChangeZoneAll | ChangeType$ Triggered.sameName | Origin$ Graveyard | Destination$ Battlefield\n";
    assert_eq!(gaps(text), Vec::<String>::new());
    let found = json!(abilities(text)).to_string();
    assert!(found.contains(r#"{"SharesName":"Triggering"}"#), "{found}");
}

fn verdict(type_line: &str, oracle: &str, found: &[Value]) -> Verdict {
    let card = PrintedCard {
        oracle_id: "00000000-0000-4000-8000-0000000f2000".into(),
        name: "Alpha".into(),
        keywords: Vec::new(),
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: "Alpha".into(),
            mana_cost: "{2}{U}".into(),
            type_line: type_line.into(),
            oracle_text: oracle.into(),
            power: Some("2".into()),
            toughness: Some("2".into()),
            ..PrintedFace::default()
        }],
    };
    let names = BTreeSet::from(["alpha".to_owned()]);
    let json = json!({"faces": [{"abilities": found}]}).to_string();
    check_exact(
        &json,
        &card,
        &Library::default(),
        &names,
        &mtg_engine::caps(),
    )
    .verdict()
}

const ASSASSIN: &str = "Name:Alpha\nTypes:Creature Elf\nPT:2/2\nA:AB$ Destroy | Cost$ T | ValidTgts$ Creature.hasKeywordLandwalk:Island\n";
const SCALES: &str = "Name:Alpha\nTypes:Enchantment\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ TrigDestroy\nSVar:TrigDestroy:DB$ Destroy | ValidTgts$ Permanent.nonLand+lowestCMC\n";
const FAVOR: &str = "Name:Alpha\nTypes:Enchantment\nS:Mode$ Continuous | Affected$ Creature.greatestCMC_Creature | AddKeyword$ Flying\n";
const KICKED: &str =
    "Name:Alpha\nTypes:Instant\nA:SP$ Counter | TargetType$ Spell | ValidTgts$ Card.Kicked\n";
const JADED: &str = "Name:Alpha\nTypes:Instant\nA:SP$ Counter | TargetType$ Spell | ValidTgts$ Card | ConditionDefined$ Targeted | ConditionPresent$ Spell.SharesColorWith Valid Creature.YouCtrl\n";
const BANNER: &str = "Name:Alpha\nTypes:Artifact Equipment\nK:Equip:2\nS:Mode$ Continuous | Affected$ Creature.SharesColorWith Equipped | AddPower$ 1 | AddToughness$ 1\n";
const MIRKO: &str = "Name:Alpha\nTypes:Creature Elf\nPT:2/2\nT:Mode$ Phase | Phase$ End of Turn | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ TrigReturn\nSVar:TrigReturn:DB$ ChangeZone | Origin$ Graveyard | Destination$ Hand | ValidTgts$ Creature.YouOwn+powerLTCount$CardPower\n";
const REACTOR: &str = "Name:Alpha\nTypes:Sorcery\nA:SP$ Destroy | ValidTgts$ Artifact.counters_GEX_CHARGE\nSVar:X:Number$3\n";
const KASSANDRA: &str = "Name:Alpha\nTypes:Enchantment\nT:Mode$ DamageDone | ValidSource$ Creature.YouCtrl+EquippedByValid Equipment.Legendary | ValidTarget$ Player | CombatDamage$ True | TriggerZones$ Battlefield | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw\n";

#[rstest]
#[case::islandwalk(
    ASSASSIN,
    "Creature — Elf",
    "{T}: Destroy target creature with islandwalk.",
    Verdict::Accepted
)]
#[case::swampwalk(
    ASSASSIN,
    "Creature — Elf",
    "{T}: Destroy target creature with swampwalk.",
    Verdict::Mismatch
)]
#[case::any_creature(
    ASSASSIN,
    "Creature — Elf",
    "{T}: Destroy target creature.",
    Verdict::Mismatch
)]
#[case::lowest(
    SCALES,
    "Enchantment",
    "At the beginning of your upkeep, destroy target nonland permanent with the lowest mana value.",
    Verdict::Accepted
)]
#[case::highest(
    SCALES,
    "Enchantment",
    "At the beginning of your upkeep, destroy target nonland permanent with the greatest mana value.",
    Verdict::Mismatch
)]
#[case::lowest_of_any_permanent(
    SCALES,
    "Enchantment",
    "At the beginning of your upkeep, destroy target permanent with the lowest mana value.",
    Verdict::Mismatch
)]
#[case::greatest_creature(
    FAVOR,
    "Enchantment",
    "Each creature with the greatest mana value has flying.",
    Verdict::Accepted
)]
#[case::greatest_artifact(
    FAVOR,
    "Enchantment",
    "Each artifact with the greatest mana value has flying.",
    Verdict::Mismatch
)]
#[case::every_creature(FAVOR, "Enchantment", "Each creature has flying.", Verdict::Mismatch)]
#[case::kicked(KICKED, "Instant", "Counter target kicked spell.", Verdict::Accepted)]
#[case::any_spell(KICKED, "Instant", "Counter target spell.", Verdict::Mismatch)]
#[case::multicolored_spell(
    KICKED,
    "Instant",
    "Counter target multicolored spell.",
    Verdict::Mismatch
)]
#[case::shares_with_yours(
    JADED,
    "Instant",
    "Counter target spell if it shares a color with a creature you control.",
    Verdict::Unchecked
)]
#[case::shares_with_theirs(
    JADED,
    "Instant",
    "Counter target spell if it shares a color with a creature an opponent controls.",
    Verdict::Unchecked
)]
#[case::shares_a_type(
    JADED,
    "Instant",
    "Counter target spell if it shares a card type with a creature you control.",
    Verdict::Unchecked
)]
#[case::color_of_equipped(
    BANNER,
    "Artifact — Equipment",
    "Creatures that share a color with equipped creature get +1/+1.\nEquip {2}",
    Verdict::Unchecked
)]
#[case::type_of_equipped(
    BANNER,
    "Artifact — Equipment",
    "Creatures that share a creature type with equipped creature get +1/+1.\nEquip {2}",
    Verdict::Unchecked
)]
#[case::power_below(
    MIRKO,
    "Creature — Elf",
    "At the beginning of your end step, return target creature card with power less than Alpha's from your graveyard to your hand.",
    Verdict::Accepted
)]
#[case::power_above(
    MIRKO,
    "Creature — Elf",
    "At the beginning of your end step, return target creature card with power greater than Alpha's from your graveyard to your hand.",
    Verdict::Mismatch
)]
#[case::three_counters(
    REACTOR,
    "Sorcery",
    "Destroy target artifact with three or more charge counters on it.",
    Verdict::Accepted
)]
#[case::two_counters(
    REACTOR,
    "Sorcery",
    "Destroy target artifact with two or more charge counters on it.",
    Verdict::Mismatch
)]
#[case::legendary_equipment(
    KASSANDRA,
    "Enchantment",
    "Whenever a creature you control with a legendary Equipment attached to it deals combat damage to a player, draw a card.",
    Verdict::Accepted
)]
#[case::any_equipment(
    KASSANDRA,
    "Enchantment",
    "Whenever a creature you control with an Equipment attached to it deals combat damage to a player, draw a card.",
    Verdict::Mismatch
)]
fn a_mapped_atom_reads_back_as_its_oracle_text(
    #[case] script: &str,
    #[case] type_line: &str,
    #[case] oracle: &str,
    #[case] expected: Verdict,
) {
    assert_eq!(gaps(script), Vec::<String>::new());
    assert_eq!(
        verdict(type_line, oracle, &abilities(script)),
        expected,
        "{oracle}"
    );
}

const PADEEM: &str = "Name:Alpha\nTypes:Enchantment\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | TriggerZones$ Battlefield | IsPresent$ Artifact.greatestCMC_Artifact+YouCtrl | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw\n";

#[rstest]
#[case::among_all_artifacts(
    "At the beginning of your upkeep, if you control an artifact with the greatest mana value, draw a card."
)]
#[case::the_printed_wording(
    "At the beginning of your upkeep, if you control the artifact with the greatest mana value or tied for the greatest mana value, draw a card."
)]
fn an_extreme_beside_a_controller_has_no_words(#[case] oracle: &str) {
    assert_eq!(gaps(PADEEM), Vec::<String>::new());
    assert_eq!(
        verdict("Enchantment", oracle, &abilities(PADEEM)),
        Verdict::Unchecked
    );
}
