//! Goal: a script's compound construct keeps every detail of the printed rule: "or another" events, one trigger for each
//! creature, a color set with the pump it is printed with, an Aura of one kind of permanent, an unblockable bestowed
//! creature and the artifact that becomes a creature.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn clean(text: &str) -> Vec<Value> {
    assert_eq!(gaps(text), Vec::<String>::new());
    abilities(text)
}

#[rstest]
#[case::another(" or another", true)]
#[case::the_card_alone("", false)]
fn a_script_that_lists_the_card_and_its_kind_says_or_another(
    #[case] words: &str,
    #[case] apart: bool,
) {
    let text = format!(
        "Name:Subject\nManaCost:2\nTypes:Creature Elf\nPT:2/2\nT:Mode$ ChangesZone | ValidCard$ Card.Self,Creature.YouCtrl | Origin$ Any | Destination$ Battlefield | TriggerZones$ Battlefield | Execute$ Gain | TriggerDescription$ Whenever CARDNAME{words} creature you control enters, you gain 1 life.\nSVar:Gain:DB$ GainLife | LifeAmount$ 1\n"
    );
    let found = clean(&text);
    let when = &found[0]["Triggered"]["when"];
    assert_eq!(when.get("AnyOf").is_some(), apart, "{found:?}");
    if apart {
        let events = when["AnyOf"].as_array().unwrap();
        assert_eq!(events[0]["ZoneChange"]["what"], json!(["This"]));
        assert!(
            events[1]["ZoneChange"]["what"]
                .as_array()
                .unwrap()
                .contains(&json!("Other")),
            "{found:?}"
        );
    }
}

#[rstest]
#[case::a_group("Creature.YouCtrl", false)]
#[case::the_card("Card.Self", true)]
fn damage_dealt_once_is_a_batch_only_for_one_object(#[case] source: &str, #[case] batch: bool) {
    let text = format!(
        "Name:Subject\nManaCost:2\nTypes:Enchantment\nT:Mode$ DamageDealtOnce | CombatDamage$ True | ValidSource$ {source} | TriggerZones$ Battlefield | Execute$ Gain\nSVar:Gain:DB$ GainLife | LifeAmount$ 1\n"
    );
    let found = clean(&text);
    let when = &found[0]["Triggered"]["when"];
    assert_eq!(when.get("Batch").is_some(), batch, "{found:?}");
    if !batch {
        assert!(when.get("DealsDamage").is_some(), "{found:?}");
    }
}

#[test]
fn a_color_printed_with_the_pump_is_set_with_it() {
    let text = "Name:Subject\nManaCost:1 R\nTypes:Instant\nA:SP$ Pump | ValidTgts$ Creature | KW$ Haste | SubAbility$ DBDraw | SpellDescription$ Target creature becomes red and gains haste until end of turn. Draw a card.\nSVar:DBDraw:DB$ Draw | SubAbility$ DBColor | NumCards$ 1\nSVar:DBColor:DB$ Animate | Colors$ Red | OverwriteColors$ True | Defined$ Targeted\n";
    let found = clean(text);
    let effects = found[0]["Spell"]["effects"].as_array().unwrap();
    assert_eq!(effects.len(), 2, "{found:?}");
    assert_eq!(
        effects[0]["Continuous"]["mods"],
        json!([{"SetColors": ["Red"]}, {"AddKeyword": "Haste"}])
    );
    assert!(effects[1].get("Draw").is_some());
}

#[test]
fn an_aura_of_artifacts_has_no_condition_and_adds_its_types_before_its_size() {
    let text = "Name:Subject\nManaCost:2 U\nTypes:Enchantment Aura\nK:Enchant:Artifact\nS:Mode$ Continuous | AffectedDefined$ Enchanted | Affected$ Artifact | SetPower$ 5 | SetToughness$ 5 | AddType$ Creature\n";
    let found = clean(text);
    let body = &found[1]["Static"];
    assert!(body.get("condition").is_none(), "{found:?}");
    assert_eq!(
        body["mods"],
        json!([{"AddType": "Creature"}, {"SetPT": {"power": 5, "toughness": 5}}])
    );
}

#[test]
fn a_bestowed_creature_that_cannot_be_blocked_gives_it_in_the_sentence_of_the_aura() {
    let text = "Name:Subject\nManaCost:2 U\nTypes:Enchantment Creature Elf\nPT:1/1\nK:Bestow:5 U\nS:Mode$ CantBlockBy | ValidAttacker$ Creature.Self,Creature.EnchantedBy | Description$ CARDNAME can't be blocked.\nS:Mode$ Continuous | AffectedDefined$ Enchanted | Affected$ Creature | AddPower$ 1 | AddToughness$ 1 | Description$ Enchanted creature gets +1/+1 and can't be blocked.\n";
    let found = clean(text);
    assert_eq!(
        found[1..],
        [
            json!({"Static": {"affected": "This", "mods": ["CantBeBlocked"]}}),
            json!({"Static": {"affected": "Attached", "mods": [{"ModifyPT": {"power": 1, "toughness": 1}}]}}),
            json!({"Static": {"affected": "Attached", "mods": ["CantBeBlocked"]}}),
        ]
    );
}

#[test]
fn a_noncreature_artifact_that_becomes_a_creature_sets_the_types_it_is_given() {
    let text = "Name:Subject\nManaCost:3\nTypes:Artifact\nA:AB$ Animate | Cost$ 3 | Defined$ Self | Power$ 3 | Toughness$ 3 | Keywords$ Flying | Types$ Creature,Artifact,Elf | SpellDescription$ This artifact becomes a 3/3 Elf artifact creature with flying until end of turn.\n";
    let found = clean(text);
    assert_eq!(
        found[0]["Activated"]["effects"][0]["Continuous"]["mods"],
        json!([
            {"SetPT": {"power": 3, "toughness": 3}},
            {"SetCreatureTypes": ["Elf"]},
            {"AddType": "Artifact"},
            {"AddType": "Creature"},
            {"AddKeyword": "Flying"}
        ])
    );
}

#[test]
fn a_supertype_the_alternatives_share_stands_before_their_choice_of_types() {
    let text = "Name:Subject\nManaCost:2\nTypes:Sorcery\nA:SP$ Draw | NumCards$ X\nSVar:X:Count$Valid Creature.Legendary+YouCtrl,Planeswalker.Legendary+YouCtrl\n";
    let found = clean(text);
    let shown = found[0].to_string();
    assert!(
        shown.contains(
            r#"[{"Supertype":"Legendary"},{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}"#
        ),
        "{shown}"
    );
}
