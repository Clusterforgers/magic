//! Goal: rules the w16 triage held back match Oracle and read back as it: a copy the printed text does not call "other"
//! drops `Other`, a trigger whose printed text has no "may" is not optional, a cost count relative to the caster stays a
//! gap; and the read-back accepts the Oracle wordings of the same rules ("If you do", "an additional counter", "another
//! card", "a commander", an alternative cost from hand, a token's name, "the power of", "its owner", "your combat step",
//! "enchanted land") while a rule of another meaning still reads `Mismatch`.
use std::collections::BTreeSet;

use rstest::rstest;
use serde_json::{Value, json};

use mtg_cards::load::Library;
use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::{PrintedCard, PrintedFace};

use super::{abilities, gaps};

fn clone(choices: &str, description: &str) -> String {
    format!(
        "Name:Test Clone\nTypes:Creature Shapeshifter\nPT:0/0\nK:ETBReplacement:Copy:DBCopy:Optional\nSVar:DBCopy:DB$ Clone | Choices$ {choices} | SpellDescription$ {description}\n"
    )
}

#[rstest]
#[case::any_card_in_a_graveyard(
    "Creature.Other | ChoiceZone$ Graveyard",
    "You may have CARDNAME enter as a copy of any creature card in a graveyard.",
    false
)]
#[case::any_land_with_other_types(
    "Land.Other",
    "You may have CARDNAME enter as a copy of any land, except it's a Cave in addition to its other types.",
    false
)]
#[case::you_control(
    "Artifact.YouCtrl+Other,Creature.YouCtrl+Other",
    "You may have this creature enter as a copy of an artifact or creature you control.",
    false
)]
#[case::another_creature(
    "Creature.Other",
    "You may have CARDNAME enter as a copy of another creature on the battlefield.",
    true
)]
#[case::any_other_creature(
    "Creature.Other",
    "You may have CARDNAME enter as a copy of any other creature.",
    true
)]
fn a_copy_keeps_other_only_where_printed(
    #[case] choices: &str,
    #[case] description: &str,
    #[case] other: bool,
) {
    let script = clone(choices, description);
    let found = json!(abilities(&script)).to_string();
    assert!(found.contains("AsCopy"), "{found} {:?}", gaps(&script));
    assert_eq!(found.contains("\"Other\""), other, "{found}");
}

#[rstest]
#[case::no_may(
    "Whenever one or more +1/+1 counters are put on CARDNAME, add one mana of any color.",
    false
)]
#[case::you_may(
    "Whenever one or more +1/+1 counters are put on CARDNAME, you may draw a card.",
    true
)]
#[case::another_player_may(
    "Whenever one or more +1/+1 counters are put on CARDNAME, target player may draw a card.",
    true
)]
fn a_decider_asks_only_where_the_printed_text_says_may(#[case] printed: &str, #[case] asks: bool) {
    let script = format!(
        "Name:Test Frog\nTypes:Creature Frog\nPT:1/1\nT:Mode$ CounterAddedOnce | ValidCard$ Card.Self | TriggerZones$ Battlefield | OptionalDecider$ You | CounterType$ P1P1 | Execute$ TrigDraw | TriggerDescription$ {printed}\nSVar:TrigDraw:DB$ Draw\n"
    );
    let found = json!(abilities(&script)).to_string();
    assert!(found.contains("Triggered"), "{found} {:?}", gaps(&script));
    assert_eq!(found.contains("\"May\""), asks, "{found}");
}

#[rstest]
#[case::each_player_counts_yours("Player", "Artifact.YouCtrl", true)]
#[case::your_spells("You", "Artifact.YouCtrl", false)]
#[case::each_player_counts_all("Player", "Artifact", false)]
fn a_count_relative_to_the_caster_is_a_gap(
    #[case] activator: &str,
    #[case] counted: &str,
    #[case] gap: bool,
) {
    let script = format!(
        "Name:Test Hum\nTypes:Enchantment\nS:Mode$ RaiseCost | ValidCard$ Artifact | Activator$ {activator} | Type$ Spell | Amount$ X | Relative$ True | Description$ Artifact spells cost more.\nSVar:X:Count$Valid {counted}\n"
    );
    let found = gaps(&script);
    assert_eq!(
        found.contains(&"key:Static.CostModifier.Relative-caster-count".to_owned()),
        gap,
        "{found:?}"
    );
}

fn printed(type_line: &str, oracle: &str) -> PrintedCard {
    PrintedCard {
        oracle_id: "00000000-0000-4000-8000-0000000f2600".into(),
        name: "Alpha".into(),
        keywords: Vec::new(),
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: "Alpha".into(),
            mana_cost: "{2}{B}".into(),
            type_line: type_line.into(),
            oracle_text: oracle.into(),
            ..PrintedFace::default()
        }],
    }
}

fn verdict(type_line: &str, oracle: &str, abilities: &Value) -> Verdict {
    let card = printed(type_line, oracle);
    let names = BTreeSet::from(["alpha".to_owned()]);
    let json = json!({"faces": [{"abilities": abilities}]}).to_string();
    check_exact(
        &json,
        &card,
        &Library::default(),
        &names,
        &mtg_engine::caps(),
    )
    .verdict()
}

const SORCERY: &str = "Sorcery";
const CREATURE: &str = "Creature \u{2014} Druid";
const FLAY: &str = "Target player discards a card at random. Then that player discards another card at random unless they pay {1}.";
const BONES: &str = "As an additional cost to cast this spell, sacrifice a creature.\nReturn a creature card from your graveyard to the battlefield, then return another creature card from your graveyard to your hand.";
const FURY: &str = "You may exile two red cards from your hand rather than pay this spell's mana cost.\nDraw a card.";
const LITA: &str = "{3}{W}, {T}: Create a 5/5 colorless Vehicle artifact token named Zeppelin with flying and crew 3.";
const MIGHTFORM: &str = "Landfall \u{2014} Whenever a land you control enters, double the power of target creature you control until end of turn.";
const PHARIKA: &str = "{B}{G}: Exile target creature card from a graveyard. Its owner creates a 1/1 black and green Snake enchantment creature token with deathtouch.";
const JESKA: &str = "Alpha enters with a loyalty counter on her for each time you've cast a commander from the command zone this game.";
const WOMBAT: &str = "Permanents you control have \"Whenever one or more +1/+1 counters are put on this permanent, put an additional +1/+1 counter on it. This ability triggers only once each turn.\"";
const CINNAMON: &str = "At the beginning of your combat step, you may put a +1/+1 counter on target noncreature Food you control. If you do, it becomes a 0/0 Knight creature in addition to its other types.";
const BERTA: &str =
    "Whenever one or more +1/+1 counters are put on Alpha, add one mana of any color.";
const BODY: &str =
    "You may have this creature enter as a copy of any creature card in a graveyard.";

fn flay(random: bool) -> Value {
    json!([{"Spell": {"effects": [
        {"Discard": {"random": true, "who": {"Target": 0}}},
        {"Pay": {"cost": "{1}", "unpaid": [{"Discard": {"random": random, "who": {"Target": 0}}}], "who": {"Target": 0}}}
    ], "targets": [{"what": "Player"}]}}])
}

fn bones(second: &Value) -> Value {
    let from = json!({"Each": [{"Type": "Creature"}, {"OwnedBy": "You"}, {"In": "Graveyard"}]});
    json!([
        {"AdditionalCost": {"cost": [{"Sacrifice": {"filter": [{"Type": "Creature"}]}}], "required": true}},
        {"Spell": {"effects": [
            {"Choose": {"as": "picked", "from": from}},
            {"Move": {"to": {"Battlefield": {}}, "what": {"Bound": "picked"}}},
            {"Choose": {"as": "picked2", "from": from}},
            {"Move": {"to": second, "what": {"Bound": "picked2"}}}
        ]}}
    ])
}

fn fury(count: u32) -> Value {
    json!([
        {"Spell": {"effects": [{"Draw": {}}]}},
        {"FreeCast": {"cost": [{"ExileFromHand": {"count": count, "filter": [{"Color": "Red"}, "Other"]}}]}}
    ])
}

fn lita(name: &str) -> Value {
    json!([{"Activated": {"cost": [{"Mana": "{3}{W}"}, "Tap"], "effects": [{"CreateToken": {"token": {"Custom": {
        "abilities": [{"Keyword": "Flying"}, {"Keyword": {"Crew": 3}}],
        "name": name, "power": 5, "subtypes": ["Vehicle"], "toughness": 5, "types": ["Artifact"]
    }}}}]}}])
}

fn mightform(target: &Value) -> Value {
    json!([{"Triggered": {
        "effects": [{"Continuous": {"affected": {"Target": 0}, "duration": "UntilEndOfTurn", "mods": [
            {"ModifyPT": {"power": {"Stat": {"of": {"Target": 0}, "stat": "Power"}}, "toughness": 0}}
        ]}}],
        "targets": [{"what": {"Object": target}}],
        "when": {"ZoneChange": {"to": "Battlefield", "what": [{"Type": "Land"}, {"ControlledBy": "You"}]}}
    }}])
}

fn pharika(who: &Value) -> Value {
    json!([{"Activated": {"cost": "{B}{G}", "effects": [
        {"Exile": {"what": {"Target": 0}}},
        {"CreateToken": {"token": {"Custom": {"abilities": [{"Keyword": "Deathtouch"}], "colors": ["Black", "Green"],
            "name": "Snake", "power": 1, "subtypes": ["Snake"], "toughness": 1, "types": ["Enchantment", "Creature"]}},
            "who": who}}
    ], "targets": [{"what": {"Object": [{"Type": "Creature"}, {"In": "Graveyard"}]}}]}}])
}

fn jeska(counter: &str) -> Value {
    json!([{"Replacement": {"by": {"WithCounters": {"count": {"CommanderCasts": "You"}, "counter": counter}},
        "when": {"Enters": {"what": ["This"]}}}}])
}

fn wombat(count: u32) -> Value {
    json!([{"Static": {"affected": {"Each": [{"ControlledBy": "You"}]}, "mods": [{"GrantAbility": {"Triggered": {
        "effects": [{"PutCounters": {"counter": "+1/+1", "on": "This", "count": count}}],
        "limit": "OncePerTurn",
        "when": {"Batch": {"CountersPut": {"counter": "+1/+1", "what": ["This"]}}}
    }}}]}}])
}

fn cinnamon(power: u32) -> Value {
    json!([{"Triggered": {
        "effects": [{"May": {"effects": [
            {"PutCounters": {"counter": "+1/+1", "on": {"Target": 0}}},
            {"Continuous": {"affected": {"Target": 0}, "duration": "Forever", "mods": [
                {"SetPT": {"power": power, "toughness": 0}}, {"AddSubtype": "Knight"}, {"AddType": "Creature"}
            ]}}
        ]}}],
        "targets": [{"what": {"Object": [{"Subtype": "Food"}, {"Not": {"Type": "Creature"}}, {"ControlledBy": "You"}]}}],
        "when": {"StepBegins": {"step": "BeginCombat", "whose": "You"}}
    }}])
}

fn berta(asks: bool) -> Value {
    let mana = json!({"AddMana": {"mana": {"AnyColor": 1}}});
    let effects = if asks {
        json!([{"May": {"effects": [mana]}}])
    } else {
        json!([mana])
    };
    json!([{"Triggered": {"effects": effects,
        "when": {"Batch": {"CountersPut": {"counter": "+1/+1", "what": ["This"]}}}}}])
}

fn body(of: &Value) -> Value {
    json!([{"Replacement": {"by": {"AsCopy": {"of": of}}, "when": {"Enters": {"what": ["This"]}}}}])
}

fn venom(target: &str) -> Value {
    json!([{"Keyword": {"Enchant": [{"Type": target}]}},
        {"Activated": {"cost": "{3}{B}", "effects": [{"Destroy": {"what": "Attached"}}]}}])
}

#[rstest]
#[case::flay(SORCERY, FLAY, flay(true), Verdict::Accepted)]
#[case::flay_chosen(SORCERY, FLAY, flay(false), Verdict::Mismatch)]
#[case::bones(SORCERY, BONES, bones(&json!("Hand")), Verdict::Accepted)]
#[case::bones_both_back(SORCERY, BONES, bones(&json!({"Battlefield": {}})), Verdict::Mismatch)]
#[case::fury(SORCERY, FURY, fury(2), Verdict::Accepted)]
#[case::fury_three(SORCERY, FURY, fury(3), Verdict::Mismatch)]
#[case::lita(
    "Artifact Creature \u{2014} Artificer",
    LITA,
    lita("Zeppelin"),
    Verdict::Accepted
)]
#[case::lita_blimp(
    "Artifact Creature \u{2014} Artificer",
    LITA,
    lita("Blimp"),
    Verdict::Mismatch
)]
#[case::mightform(CREATURE, MIGHTFORM, mightform(&json!([{"Type": "Creature"}, {"ControlledBy": "You"}])), Verdict::Accepted)]
#[case::mightform_any(CREATURE, MIGHTFORM, mightform(&json!([{"Type": "Creature"}])), Verdict::Mismatch)]
#[case::pharika(CREATURE, PHARIKA, pharika(&json!({"ControllerOf": {"Target": 0}})), Verdict::Accepted)]
#[case::pharika_you(CREATURE, PHARIKA, pharika(&json!("You")), Verdict::Mismatch)]
#[case::jeska(
    "Legendary Planeswalker \u{2014} Jeska",
    JESKA,
    jeska("loyalty"),
    Verdict::Accepted
)]
#[case::jeska_plus_one(
    "Legendary Planeswalker \u{2014} Jeska",
    JESKA,
    jeska("+1/+1"),
    Verdict::Mismatch
)]
#[case::wombat(CREATURE, WOMBAT, wombat(1), Verdict::Accepted)]
#[case::wombat_two(CREATURE, WOMBAT, wombat(2), Verdict::Mismatch)]
#[case::cinnamon(CREATURE, CINNAMON, cinnamon(0), Verdict::Accepted)]
#[case::cinnamon_one(CREATURE, CINNAMON, cinnamon(1), Verdict::Mismatch)]
#[case::berta(CREATURE, BERTA, berta(false), Verdict::Accepted)]
#[case::berta_may(CREATURE, BERTA, berta(true), Verdict::Mismatch)]
#[case::body(CREATURE, BODY, body(&json!([{"Type": "Creature"}, {"In": "Graveyard"}])), Verdict::Accepted)]
#[case::body_other(CREATURE, BODY, body(&json!([{"Type": "Creature"}, "Other", {"In": "Graveyard"}])), Verdict::Mismatch)]
#[case::venom(
    "Enchantment \u{2014} Aura",
    "Enchant land\n{3}{B}: Destroy enchanted land.",
    venom("Land"),
    Verdict::Accepted
)]
#[case::venom_creature(
    "Enchantment \u{2014} Aura",
    "Enchant creature\n{3}{B}: Destroy enchanted land.",
    venom("Creature"),
    Verdict::Mismatch
)]
fn the_oracle_wordings_read_back(
    #[case] type_line: &str,
    #[case] oracle: &str,
    #[case] abilities: Value,
    #[case] expected: Verdict,
) {
    assert_eq!(verdict(type_line, oracle, &abilities), expected, "{oracle}");
}
