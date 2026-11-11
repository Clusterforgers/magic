//! Goal: the pieces of ep33 translate as the card prints them. "Whenever you collect evidence" (`Mode$ CollectEvidence`) is the action of the player it names, and `FaceDown$ True` onto the battlefield is a face-down move under the player `GainControl$` names; another key on the line, a face-down move elsewhere or one with set characteristics stays a gap. Restricted mana reads your commander, no spells, and the special actions of turning face up, unlocking and foretelling, and "the exiled card" of a memory a line writes is what the source exiled.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn trigger(line: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Elf\nT:{line} | TriggerZones$ Battlefield | Execute$ Go\nSVar:Go:DB$ Draw | NumCards$ 1\n"
    )
}

#[rstest]
#[case::you("ValidPlayer$ You", json!({"action": "CollectedEvidence", "who": "You"}))]
#[case::an_opponent(
    "ValidPlayer$ Opponent",
    json!({"action": "CollectedEvidence", "who": "Opponents"})
)]
fn ep33_collect_evidence_is_did(#[case] line: &str, #[case] node: Value) {
    let script = trigger(&format!("Mode$ CollectEvidence | {line}"));
    let expected = normal(json!({"Triggered": {"when": {"Did": node}, "effects": [{"Draw": {}}]}}));
    assert_eq!(abilities(&script), vec![expected]);
}

#[test]
fn ep33_collect_evidence_with_another_key_is_a_gap() {
    let script = trigger("Mode$ CollectEvidence | ValidPlayer$ You | Amount$ 4");
    assert!(!gaps(&script).is_empty());
}

/// Ashcloud Phoenix's dies trigger with `part` added to its move.
fn phoenix(part: &str, destination: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Phoenix\nPT:4/1\nT:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self | Execute$ Back | TriggerDescription$ When CARDNAME dies, return it face down.\nSVar:Back:DB$ ChangeZone | Defined$ TriggeredNewCardLKICopy | Origin$ Graveyard | Destination$ {destination} | FaceDown$ True{part}\n"
    )
}

#[rstest]
#[case::under_you(" | GainControl$ True", json!({"under": "You"}))]
#[case::under_the_owner("", json!({}))]
fn ep33_a_card_returned_face_down(#[case] part: &str, #[case] enters: Value) {
    let script = phoenix(part, "Battlefield");
    assert_eq!(gaps(&script), Vec::<String>::new());
    let found = abilities(&script);
    assert_eq!(
        found[0]["Triggered"]["effects"],
        json!([{"Move": {"what": "This", "to": {"BattlefieldFaceDown": enters}}}])
    );
}

#[rstest]
#[case::tapped(" | Tapped$ True", "Battlefield")]
#[case::with_counters(" | WithCountersType$ P1P1", "Battlefield")]
#[case::a_set_power(" | FaceDownPower$ 3", "Battlefield")]
#[case::a_set_type(" | FaceDownSetType$ Land & Forest", "Battlefield")]
#[case::into_the_hand("", "Hand")]
#[case::not_true(" | GainControl$ True", "Battlefield")]
fn ep33_a_face_down_move_the_engine_lacks_is_a_gap(#[case] part: &str, #[case] destination: &str) {
    let script = if part == " | GainControl$ True" {
        phoenix(part, destination).replace("FaceDown$ True", "FaceDown$ False")
    } else {
        phoenix(part, destination)
    };
    assert!(!gaps(&script).is_empty(), "{:?}", abilities(&script));
}

fn pipes(amount: &str) -> String {
    format!(
        "Name:T\nTypes:Enchantment\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Dread | TriggerDescription$ When CARDNAME enters, manifest dread.\nSVar:Dread:DB$ ManifestDread{amount}\n"
    )
}

#[rstest]
#[case::once("", 4)]
#[case::twice(" | Amount$ 2", 8)]
fn ep33_manifest_dread_repeats_the_whole_action(#[case] amount: &str, #[case] steps: usize) {
    let script = pipes(amount);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let effects = &abilities(&script)[0]["Triggered"]["effects"];
    let effects = effects.as_array().unwrap();
    assert_eq!(effects.len(), steps);
    assert_eq!(effects[steps - 1], json!({"Manifest": {}}));
}

#[rstest]
#[case::a_count_amount(" | Amount$ X")]
#[case::twice_remembered(" | Amount$ 2 | RememberManifested$ True")]
fn ep33_manifest_dread_the_engine_lacks_is_a_gap(#[case] amount: &str) {
    assert!(!gaps(&pipes(amount)).is_empty());
}

fn restricted(valid: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact\nA:AB$ Mana | Cost$ T | Produced$ U | RestrictValid$ {valid} | SpellDescription$ Add {{U}}.\n"
    )
}

#[rstest]
#[case::your_commander("Spell.IsCommander+YouOwn", json!([{"Spell": {"facts": ["YourCommander"]}}]))]
#[case::no_spells("nonSpell", json!([{"Ability": {}}, "OtherCosts"]))]
#[case::turn_face_up("Static.isTurnFaceUp", json!([{"Special": "TurnFaceUp"}]))]
#[case::foretell_or_instants(
    "Static.Foretelling,Spell.Instant,Spell.Sorcery",
    json!([{"Special": "Foretell"}, {"Spell": {"types": ["Instant"]}}, {"Spell": {"types": ["Sorcery"]}}])
)]
#[case::unlock(
    "Spell.Enchantment,Static.Unlock,Static.isTurnFaceUp",
    json!([{"Spell": {"types": ["Enchantment"]}}, {"Special": "Unlock"}, {"Special": "TurnFaceUp"}])
)]
fn ep33_restricted_mana_uses(#[case] valid: &str, #[case] only: Value) {
    let script = restricted(valid);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let found = abilities(&script);
    assert_eq!(found[0]["Activated"]["effects"][0]["AddMana"]["only"], only);
}

#[rstest]
#[case::turn_creatures_up("Static.isTurnFaceUp+Creature")]
#[case::any_commander("Spell.IsCommander")]
#[case::morph_up("Static.MorphUp")]
fn ep33_restricted_mana_the_engine_lacks_is_a_gap(#[case] valid: &str) {
    assert!(!gaps(&restricted(valid)).is_empty());
}

/// A creature that exiles a target creature as it enters (with `keep` on that move) and returns "the exiled card"
/// (`<memory>.ExiledWithSource`) as it leaves.
fn crab(memory: &str, keep: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Crab\nPT:2/2\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Gone | TriggerDescription$ When CARDNAME enters, exile target creature.\nSVar:Gone:DB$ ChangeZone | ValidTgts$ Creature | Origin$ Battlefield | Destination$ Exile{keep}\nT:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Any | ValidCard$ Card.Self | Execute$ Back | TriggerDescription$ When CARDNAME leaves the battlefield, return the exiled card.\nSVar:Back:DB$ ChangeZone | Defined$ {memory}.ExiledWithSource | Origin$ Exile | Destination$ Battlefield\n"
    )
}

#[rstest]
#[case::imprinted("Imprinted", " | Imprint$ True")]
#[case::remembered("Remembered", " | RememberChanged$ True")]
fn ep33_the_card_exiled_with_the_source(#[case] memory: &str, #[case] keep: &str) {
    let script = crab(memory, keep);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let found = abilities(&script);
    assert_eq!(
        found[1]["Triggered"]["effects"][0]["Move"]["what"],
        json!({"Each": [{"ExiledWith": "This"}, {"In": "Exile"}]})
    );
}

#[rstest]
#[case::no_memory_written("Imprinted", "")]
#[case::another_memory("Chosen", " | RememberChanged$ True")]
fn ep33_the_card_exiled_with_the_source_needs_its_memory(#[case] memory: &str, #[case] keep: &str) {
    assert!(!gaps(&crab(memory, keep)).is_empty());
}

fn voting(line: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Human\nPT:1/1\nS:Mode$ Continuous | {line} | Description$ While voting, you get an additional vote.\n"
    )
}

#[test]
fn ep33_an_additional_vote() {
    let script = voting("Affected$ You | AdditionalVote$ 1");
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(
        abilities(&script),
        vec![json!({"Rule": {"rule": {"ExtraVote": "You"}}})]
    );
}

#[rstest]
#[case::two_votes("Affected$ You | AdditionalVote$ 2")]
#[case::opponents("Affected$ Opponent | AdditionalVote$ 1")]
fn ep33_an_additional_vote_the_engine_lacks_is_a_gap(#[case] line: &str) {
    assert!(!gaps(&voting(line)).is_empty());
}
