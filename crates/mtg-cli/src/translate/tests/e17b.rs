//! Goal: pack e17b's memories read from Forge lines: a `DigUntil` that imprints the card found and remembers the rest (or the other way round) exiles each under its own name, the rest without the card found; a return whose remembered card enters with counters when of a kind is a choice between the two moves; "permanent spells" can't be cast keeps its permanent types; other shapes stay gaps.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

/// A sorcery that digs until a land with `keys` and then moves `then` (the found card) and the rest.
fn dig(keys: &str, then: &str, rest: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ DigUntil | Defined$ You | Valid$ Land | FoundDestination$ Exile | RevealedDestination$ Exile | {keys} | SubAbility$ DBThen | SpellDescription$ X.\nSVar:DBThen:DB$ ChangeZone | Defined$ {then} | Origin$ Exile | Destination$ Battlefield | SubAbility$ DBRest\nSVar:DBRest:DB$ ChangeZoneAll | ChangeType$ {rest} | Origin$ Exile | Destination$ Library | LibraryPosition$ -1 | RandomOrder$ True | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True | ClearImprinted$ True\n"
    )
}

fn effects(text: &str) -> Vec<Value> {
    assert_eq!(gaps(text), Vec::<String>::new(), "{text}");
    abilities(text)[0]["Spell"]["effects"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn bound(name: &str) -> Value {
    json!({ "Bound": name })
}

#[rstest]
#[case::imprint_found(
    "ImprintFound$ True | RememberRevealed$ True",
    "Imprinted",
    "Card.IsRemembered"
)]
#[case::remember_found(
    "RememberFound$ True | ImprintRevealed$ True",
    "Remembered",
    "Card.IsImprinted"
)]
fn the_card_found_and_the_rest_are_named_apart(
    #[case] keys: &str,
    #[case] found: &str,
    #[case] rest: &str,
) {
    let effects = effects(&dig(keys, found, rest));
    assert_eq!(
        effects[1],
        json!({"Exile": {"what": bound("found"), "as": "found_moved3"}})
    );
    assert_eq!(
        effects[2],
        json!({"Exile": {"what": bound("revealed2"), "as": "rest_moved4"}})
    );
    assert_eq!(effects[3]["Move"]["what"], bound("found_moved3"));
    assert_eq!(
        effects[4]["Move"]["what"]["Each"][0],
        json!({"Is": bound("rest_moved4")})
    );
}

#[rstest]
#[case::both_remembered("RememberFound$ True | RememberRevealed$ True")]
#[case::found_may_stay("ImprintFound$ True | RememberRevealed$ True | OptionalFoundMove$ True")]
fn other_dig_shapes_stay_gaps(#[case] keys: &str) {
    assert_ne!(
        gaps(&dig(keys, "Imprinted", "Card.IsRemembered")),
        Vec::<String>::new()
    );
}

/// Heroic Return's shape: an `Effect` remembering its target and itself with a `Moved` replacement, then the return.
fn returned(valid: &str, put: &str, back: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Effect | RememberObjects$ Targeted & Self | ReplacementEffects$ ETBCreat | ExileOnMoved$ Graveyard,Stack | SubAbility$ DBReturn | SpellDescription$ X.\nSVar:DBReturn:DB$ ChangeZone | Origin$ Graveyard | Destination$ Battlefield | ValidTgts$ Creature.YouOwn{back}\nSVar:ETBCreat:Event$ Moved | ValidCard$ {valid} | Destination$ Battlefield | ReplaceWith$ DBPutP1P1 | ReplacementResult$ Updated | Description$ X.\nSVar:DBPutP1P1:DB$ PutCounter | Defined$ ReplacedCard | CounterType$ P1P1 | ETB$ True | CounterNum$ {put}\n"
    )
}

#[rstest]
#[case::an_artifact("Artifact.IsRemembered", "2", json!({"Type": "Artifact"}), 2)]
#[case::a_creature("Creature.IsRemembered", "1", json!({"Type": "Creature"}), 1)]
fn a_returned_card_of_a_kind_gets_counters(
    #[case] valid: &str,
    #[case] put: &str,
    #[case] kind: Value,
    #[case] count: u32,
) {
    let effects = effects(&returned(valid, put, ""));
    let target = json!({"Target": 0});
    assert_eq!(
        effects,
        vec![json!({"If": {
            "condition": {"CountOf": {"subject": target, "selector": [kind], "op": "Ge", "right": 1}},
            "then": [{"Move": {"what": target, "to": {"Battlefield": {"counters": [["+1/+1", count]]}}}}],
            "otherwise": [{"Move": {"what": target, "to": {"Battlefield": {}}}}],
        }})]
    );
}

#[rstest]
#[case::not_remembered("Creature", "2", "")]
#[case::counted_number("Creature.IsRemembered", "X", "")]
#[case::more_after_the_return("Creature.IsRemembered", "2", " | SubAbility$ DBReturn")]
fn other_returns_stay_gaps(#[case] valid: &str, #[case] put: &str, #[case] back: &str) {
    assert_ne!(gaps(&returned(valid, put, back)), Vec::<String>::new());
}

#[test]
fn permanent_spells_keep_their_types() {
    let text = "Name:T\nTypes:Creature\nPT:1/1\nS:Mode$ CantBeCast | ValidCard$ Permanent | Caster$ You | Description$ X.\n";
    assert_eq!(gaps(text), Vec::<String>::new());
    let kinds: Vec<Value> = [
        "Artifact",
        "Battle",
        "Creature",
        "Enchantment",
        "Land",
        "Planeswalker",
    ]
    .iter()
    .map(|kind| json!({ "Type": kind }))
    .collect();
    assert_eq!(
        abilities(text)[0]["Rule"]["rule"]["Cant"]["action"],
        json!({"Cast": {"filter": [{"AnyOf": kinds}]}})
    );
}
