//! Goal: the audit findings stay fixed: a `Triggered*` word names what the engine holds for the trigger's mode, a permanent card outside the battlefield is not an instant or sorcery, an accepted key that nothing reads and a condition the engine would recheck make a gap, and no mapper checks keys on its own.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

/// A trigger line whose part deals damage to the object a `Defined$` word names.
fn trigger(line: &str, word: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Elf\nT:{line} | Execute$ Go\nSVar:Go:DB$ DealDamage | Defined$ {word} | NumDmg$ 2\n"
    )
}

/// What the damage goes to, or the gaps.
fn target_of(text: &str) -> Result<Value, Vec<String>> {
    let found = gaps(text);
    if !found.is_empty() {
        return Err(found);
    }
    let list = abilities(text);
    Ok(list[0]["Triggered"]["effects"][0]["DealDamage"]["to"].clone())
}

const SELF_BLOCKED_BY: &str =
    "Mode$ AttackerBlockedByCreature | ValidCard$ Card.Self | ValidBlocker$ Creature";

#[rstest]
#[case::the_blocker_is_the_dealer(SELF_BLOCKED_BY, "TriggeredBlockerLKICopy", Ok("Dealer"))]
#[case::the_blocked_one_is_the_card(SELF_BLOCKED_BY, "TriggeredAttacker", Ok("This"))]
#[case::no_blocker_is_held_without_a_pattern_for_it(
    "Mode$ AttackerBlockedByCreature | ValidCard$ Card.Self",
    "TriggeredBlocker",
    Err("defined:TriggeredBlocker-in-AttackerBlockedByCreature")
)]
#[case::a_plain_block_holds_no_blocker(
    "Mode$ AttackerBlocked | ValidCard$ Card.Self",
    "TriggeredBlocker",
    Err("defined:TriggeredBlocker-in-AttackerBlocked")
)]
#[case::this_blocks(
    "Mode$ Blocks | ValidCard$ Card.Self",
    "TriggeredBlocker",
    Ok("This")
)]
#[case::this_blocks_the_attacker_is_not_held(
    "Mode$ Blocks | ValidCard$ Card.Self",
    "TriggeredAttacker",
    Err("defined:TriggeredAttacker-in-Blocks")
)]
#[case::another_blocks_holds_two_creatures(
    "Mode$ Blocks | ValidCard$ Creature",
    "TriggeredBlocker",
    Err("defined:TriggeredBlocker-in-Blocks")
)]
#[case::damage_source(
    "Mode$ DamageDone | ValidSource$ Creature.YouCtrl | ValidTarget$ Player",
    "TriggeredSource",
    Ok("Dealer")
)]
#[case::damage_recipient(
    "Mode$ DamageDone | ValidSource$ Creature.YouCtrl | ValidTarget$ Player",
    "TriggeredTarget",
    Ok("Triggering")
)]
#[case::damage_in_one_batch_has_no_dealer(
    "Mode$ DamageDoneOnce | ValidSource$ Creature.YouCtrl | ValidTarget$ Player",
    "TriggeredSource",
    Err("defined:TriggeredSource-in-DamageDoneOnce")
)]
#[case::the_card_that_dies_is_this(
    "Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self",
    "TriggeredCard",
    Ok("This")
)]
#[case::another_card_that_dies(
    "Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Creature.YouCtrl",
    "TriggeredCard",
    Ok("Triggering")
)]
#[case::a_draw_holds_only_the_player(
    "Mode$ Drawn | ValidCard$ Card.YouCtrl",
    "TriggeredCard",
    Err("defined:TriggeredCard-in-Drawn")
)]
#[case::the_player_who_draws(
    "Mode$ Drawn | ValidCard$ Card.YouCtrl",
    "TriggeredPlayer",
    Ok("Triggering")
)]
#[case::the_player_who_discards_is_not_the_card(
    "Mode$ Discarded | ValidCard$ Card.Self",
    "TriggeredPlayer",
    Ok("Triggering")
)]
#[case::a_word_outside_a_trigger_is_unknown(
    "Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You",
    "TriggeredBlocker",
    Err("defined:TriggeredBlocker-in-Phase")
)]
fn triggered_words_name_what_the_engine_holds(
    #[case] line: &str,
    #[case] word: &str,
    #[case] expected: Result<&str, &str>,
) {
    let found = target_of(&trigger(line, word));
    match expected {
        Ok(reference) => assert_eq!(found, Ok(json!(reference))),
        Err(gap) => assert_eq!(found, Err(vec![gap.to_owned()])),
    }
}

#[rstest]
#[case::in_a_graveyard("Graveyard", true)]
#[case::on_the_battlefield("Battlefield", false)]
fn a_permanent_card_outside_the_battlefield_is_not_an_instant(
    #[case] origin: &str,
    #[case] restricted: bool,
) {
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ {origin} | Destination$ Hand | ValidTgts$ Permanent.YouOwn\n"
    );
    let found = gaps(&text);
    let shown = serde_json::to_string(&abilities(&text)).unwrap_or_default();
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(shown.contains("\"Planeswalker\""), restricted, "{shown}");
}

#[rstest]
#[case::checked_again_by_the_engine(
    "Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | IsPresent$ Elf.Other+YouCtrl | NoResolvingCheck$ True",
    &["key:Trigger.NoResolvingCheck"]
)]
#[case::without_a_condition_it_changes_nothing(
    "Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | NoResolvingCheck$ True",
    &[]
)]
fn a_condition_the_engine_rechecks_is_not_translated_as_unchecked(
    #[case] line: &str,
    #[case] expected: &[&str],
) {
    let found = gaps(&trigger(line, "Self"));
    assert_eq!(found, expected);
}

#[rstest]
#[case::divided_damage_with_its_total(
    "SP$ DealDamage | ValidTgts$ Any | NumDmg$ 3 | TargetMax$ 3 | DividedAsYouChoose$ 3",
    &[]
)]
#[case::divided_damage_with_another_total(
    "SP$ DealDamage | ValidTgts$ Any | NumDmg$ 2 | TargetMax$ 3 | DividedAsYouChoose$ 3",
    &["key:NumDmg-differs-from-DividedAsYouChoose"]
)]
#[case::a_target_beside_a_defined_object(
    "SP$ GainControl | ValidTgts$ Opponent | Defined$ Self",
    &["key:GainControl.Defined-unread"]
)]
#[case::next_turn_is_not_read_as_this_turn(
    "SP$ DelayedTrigger | Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | NextTurn$ True | Execute$ Go",
    &["key:DelayedTrigger.NextTurn"]
)]
fn a_key_a_mapper_lists_is_read_or_the_line_is_a_gap(#[case] line: &str, #[case] expected: &[&str]) {
    let text = format!("Name:T\nTypes:Sorcery\nA:{line}\nSVar:Go:DB$ Draw | NumCards$ 1\n");
    assert_eq!(gaps(&text), expected);
}

#[test]
fn no_mapper_checks_the_keys_of_a_line_on_its_own() {
    // A loop over the keys of a line that is not `ctx::check_keys` would accept keys without the audit seeing them.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/translate");
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{e}")).flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.extension().is_none_or(|ext| ext != "rs") || name == "ctx.rs" {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        if text.contains("for (key, _) in p.iter()")
            || text.contains("for (key, _) in part.iter()")
            || text.contains("for (key, value) in p.iter()")
        {
            offenders.push(name);
        }
    }
    assert!(offenders.is_empty(), "{offenders:?}");
}

#[test]
fn the_audit_names_a_promised_key_that_is_never_read() {
    use crate::forge::script::{Params, audit};
    let (read, unread) = audit(|| {
        let line = Params::parse("DB$ Draw | NumCards$ 2 | Defined$ You");
        line.promise("Draw", &["NumCards", "Defined"]);
        line.get("NumCards").map(str::to_owned)
    });
    assert_eq!(read.as_deref(), Some("2"));
    assert_eq!(unread.as_deref(), Some("Draw.Defined"));
    let (_, none) = audit(|| {
        let line = Params::parse("DB$ Draw | NumCards$ 2");
        line.promise("Draw", &["NumCards", "Defined"]);
        let _ = line.get("NumCards");
    });
    assert_eq!(none, None);
}
