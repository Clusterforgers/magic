//! Goal: the reader table (`refs.rs`) and the writer table (`remember.rs`) map Forge's defined references and remember keys
//! mechanically: a word reads in the scopes its row names and stays a gap elsewhere, what an effect's statics remember is
//! fixed as they begin, a group cannot last per object, and a translation set against a near-miss Oracle reads `Differs`.
use std::collections::BTreeSet;

use rstest::rstest;
use serde_json::{Value, json};

use mtg_cards::load::Library;
use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::{PrintedCard, PrintedFace};

use super::super::refs::{self, Scope};
use super::super::remember;
use super::{abilities, gaps};
use crate::forge::script::Params;

fn instant(body: &str) -> String {
    format!("Name:Alpha\nTypes:Instant\n{body}\n")
}

/// A counter of target spell followed by an effect whose statics remember `word`.
fn counter_then_effect(word: &str) -> String {
    instant(&format!(
        "A:SP$ Counter | TargetType$ Spell | ValidTgts$ Card | SubAbility$ DBEffect\nSVar:DBEffect:DB$ Effect | StaticAbilities$ CantBeCast | RememberObjects$ {word}\nSVar:CantBeCast:Mode$ CantBeCast | ValidCard$ Card | Caster$ Player.IsRemembered"
    ))
}

#[rstest]
#[case::the_targets_controller("TargetedController", json!({"ControllerOf": {"Target": 0}}))]
#[case::the_targets_owner("TargetedOwner", json!({"OwnerOf": {"Target": 0}}))]
#[case::you("You", json!("You"))]
fn an_effects_statics_fix_the_player_a_word_names(#[case] word: &str, #[case] who: Value) {
    let script = counter_then_effect(word);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let effects = &abilities(&script)[0]["Spell"]["effects"];
    assert_eq!(effects[1]["Rule"]["rule"]["Cant"]["who"], who);
}

#[rstest]
#[case::an_unknown_word("Nonsense", "key:Effect.RememberObjects=Nonsense")]
#[case::a_player_beside_an_object(
    "Targeted & TargetedController",
    "defined:Targeted & TargetedController"
)]
fn a_word_outside_the_table_stays_a_gap(#[case] word: &str, #[case] gap: &str) {
    assert_eq!(gaps(&counter_then_effect(word)), vec![gap.to_owned()]);
}

#[test]
fn every_object_a_valid_string_selects_is_fixed_as_the_statics_begin() {
    let script = instant(
        "A:SP$ Effect | StaticAbilities$ Weak | RememberObjects$ Valid Creature.OppCtrl\nSVar:Weak:Mode$ Continuous | Affected$ Card.IsRemembered | AddPower$ -1 | AddToughness$ -1",
    );
    assert_eq!(gaps(&script), Vec::<String>::new());
    let effect = &abilities(&script)[0]["Spell"]["effects"][0]["Continuous"];
    assert_eq!(
        effect["affected"],
        json!({"Each": [{"Type": "Creature"}, {"ControlledBy": "Opponents"}]})
    );
}

#[test]
fn a_counter_duration_over_a_group_stays_a_gap() {
    let script = instant(
        "A:SP$ Effect | StaticAbilities$ Stay | RememberObjects$ Valid Creature.OppCtrl | Duration$ Permanent | ForgetOnMoved$ Battlefield | ForgetCounter$ STUN\nSVar:Stay:Mode$ Continuous | Affected$ Card.IsRemembered | AddHiddenKeyword$ This card doesn't untap during your untap step.",
    );
    assert!(
        gaps(&script).contains(&"key:Effect.ForgetCounter-on-a-group".to_owned()),
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::parent_targets_controller("ParentTargetedController", Scope::Now, true)]
#[case::this_targeted_owner("ThisTargetedOwner", Scope::Now, true)]
#[case::enchanted_owner("EnchantedOwner", Scope::Now, true)]
#[case::active_player("ActivePlayer", Scope::Now, true)]
#[case::corrected_self("CorrectedSelf", Scope::Now, true)]
// A word read again while an effect's trigger or shield lasts must name the same objects throughout.
#[case::lasting_controller("TargetedController", Scope::Lasting, false)]
#[case::lasting_attached("Enchanted", Scope::Lasting, false)]
#[case::lasting_parent("ParentTarget", Scope::Lasting, true)]
// A target an earlier step moved is another object (CR 400.7): a reflexive trigger does not bind it.
#[case::reflexive_target("Targeted", Scope::Reflexive, false)]
#[case::reflexive_remembered("RememberedLKI", Scope::Reflexive, true)]
#[case::imprint_equipped("Equipped", Scope::Imprint, true)]
#[case::imprint_controller("TargetedController", Scope::Imprint, false)]
fn a_word_reads_only_in_the_scopes_of_its_row(
    #[case] word: &str,
    #[case] scope: Scope,
    #[case] known: bool,
) {
    assert_eq!(refs::knows(word, scope), known);
}

#[rstest]
#[case::destroy_lki("Destroy", "RememberLKI", true)]
#[case::destroy_all_lki("DestroyAll", "RememberLKI", false)]
#[case::hidden_move_targets("ChangeZone.Hidden", "RememberTargets", false)]
#[case::move_targets("ChangeZone", "RememberTargets", true)]
#[case::acted_on_is_no_move("Pump", "RememberPumped", false)]
fn a_writer_key_binds_only_on_the_apis_of_its_rows(
    #[case] api: &str,
    #[case] key: &str,
    #[case] wanted: bool,
) {
    let p = Params::parse(&format!("{key}$ True"));
    assert_eq!(remember::wanted(&p, api), wanted);
}

#[test]
fn an_imprint_key_reads_as_the_remember_key_of_its_verb() {
    assert_eq!(
        remember::imprint_as("Mill", "Imprint"),
        Some("RememberMilled")
    );
    assert_eq!(remember::imprint_as("Draw", "Imprint"), None);
    assert!(remember::acts_on("Pump", "RememberPumped"));
    assert!(!remember::acts_on("PumpAll", "RememberPumped"));
}

fn verdict(oracle: &str, script: &str) -> Verdict {
    let card = PrintedCard {
        oracle_id: "00000000-0000-4000-8000-00000000f020".into(),
        name: "Alpha".into(),
        keywords: Vec::new(),
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: "Alpha".into(),
            mana_cost: "{1}{U}".into(),
            type_line: "Instant".into(),
            oracle_text: oracle.into(),
            ..PrintedFace::default()
        }],
    };
    let names = BTreeSet::from(["alpha".to_owned()]);
    let json = json!({"faces": [{"abilities": abilities(script)}]}).to_string();
    let report = check_exact(
        &json,
        &card,
        &Library::default(),
        &names,
        &mtg_engine::caps(),
    );
    report.verdict()
}

// "Can't cast spells" for a player has no writer yet: the draft stays `Unchecked` whatever the text says, never `Accepted`.
#[rstest]
#[case::as_printed("Counter target spell. Its controller can't cast spells this turn.")]
#[case::its_owner("Counter target spell. Its owner can't cast spells this turn.")]
fn a_fixed_player_without_a_writer_stays_unchecked(#[case] oracle: &str) {
    let script = counter_then_effect("TargetedController");
    assert_eq!(verdict(oracle, &script), Verdict::Unchecked);
}

fn aura_verdict(oracle: &str, word: &str) -> Verdict {
    let card = PrintedCard {
        oracle_id: "00000000-0000-4000-8000-00000000f021".into(),
        name: "Alpha".into(),
        keywords: Vec::new(),
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: "Alpha".into(),
            mana_cost: "{1}{B}".into(),
            type_line: "Enchantment — Aura".into(),
            oracle_text: oracle.into(),
            ..PrintedFace::default()
        }],
    };
    let script = format!(
        "Name:Alpha\nTypes:Enchantment Aura\nK:Enchant:Creature\nA:SP$ Attach | ValidTgts$ Creature\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ Hurt\nSVar:Hurt:DB$ DealDamage | Defined$ {word} | DamageSource$ Enchanted | NumDmg$ 1"
    );
    assert_eq!(gaps(&script), Vec::<String>::new());
    let names = BTreeSet::from(["alpha".to_owned()]);
    let json = json!({"faces": [{"abilities": abilities(&script)}]}).to_string();
    check_exact(
        &json,
        &card,
        &Library::default(),
        &names,
        &mtg_engine::caps(),
    )
    .verdict()
}

const ENSLAVED: &str = "Enchant creature\nAt the beginning of your upkeep, enchanted creature deals 1 damage to its owner.";

// The owner of the enchanted creature has no writer yet either: as printed or not, the draft is never `Accepted`.
#[rstest]
#[case::as_printed(ENSLAVED)]
#[case::its_controller(
    "Enchant creature\nAt the beginning of your upkeep, enchanted creature deals 1 damage to its controller."
)]
#[case::you(
    "Enchant creature\nAt the beginning of your upkeep, enchanted creature deals 1 damage to you."
)]
fn a_new_player_word_without_a_writer_stays_unchecked(#[case] oracle: &str) {
    assert_eq!(aura_verdict(oracle, "EnchantedOwner"), Verdict::Unchecked);
}

#[rstest]
#[case::as_printed(
    "Creatures your opponents control get -1/-1 until end of turn.",
    Verdict::Accepted
)]
#[case::yours(
    "Creatures you control get -1/-1 until end of turn.",
    Verdict::Mismatch
)]
#[case::other_amount(
    "Creatures your opponents control get -2/-2 until end of turn.",
    Verdict::Mismatch
)]
fn a_fixed_group_reads_back_as_printed_and_a_near_miss_differs(
    #[case] oracle: &str,
    #[case] expected: Verdict,
) {
    let script = instant(
        "A:SP$ Effect | StaticAbilities$ Weak | RememberObjects$ Valid Creature.OppCtrl\nSVar:Weak:Mode$ Continuous | Affected$ Card.IsRemembered | AddPower$ -1 | AddToughness$ -1",
    );
    assert_eq!(verdict(oracle, &script), expected);
}
