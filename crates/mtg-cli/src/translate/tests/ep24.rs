//! Goal: the pieces of ep24 translate as the card prints them: a change of one target to the source (`DefinedMagnet$ Self` with `ChangeSingleTarget$`) over spells and abilities or instant and sorcery spells with a single target; a magnet without a single change, another magnet or a magnet beside a chooser stays a gap. Unspent mana kept of one type or every type, and lost mana that becomes one type; another player, another type word or another replacement stays a gap. Mana spent as though of any color or type, for everything or for spells of one kind; white mana only, abilities or cards you do not own stay gaps. Mana of one color among objects on the battlefield, in your graveyard or of the card itself; an imprinted or exiled card and a count stay gaps. A draw limit of a number and opponents that can't make you sacrifice; a counted limit, costs or another kind of permanent stay gaps.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn activated(line: &str) -> String {
    format!("Name:T\nTypes:Artifact Creature Horror\nPT:0/4\nA:AB$ {line}\n")
}

fn effects(script: &str) -> Value {
    abilities(script)
        .first()
        .and_then(|found| found.get("Activated"))
        .and_then(|body| body.get("effects"))
        .cloned()
        .unwrap_or_default()
}

fn targets(script: &str) -> Value {
    abilities(script)
        .first()
        .and_then(|found| found.get("Activated"))
        .and_then(|body| body.get("targets"))
        .cloned()
        .unwrap_or_default()
}

#[rstest]
#[case::spell_or_ability(
    "ChangeTargets | Cost$ U | TargetType$ Spell,Activated,Triggered | ValidTgts$ Card,Emblem | DefinedMagnet$ Self | ChangeSingleTarget$ True",
    json!([{"AnyOf": ["Spell", {"Ability": "Activated"}, {"Ability": "Triggered"}]}])
)]
#[case::instant_or_sorcery_with_one_target(
    "ChangeTargets | Cost$ U | TargetType$ Instant.singleTarget,Sorcery.singleTarget | ValidTgts$ Card | DefinedMagnet$ Self | ChangeSingleTarget$ True",
    json!(["Spell", {"AnyOf": [{"Type": "Instant"}, {"Type": "Sorcery"}]}, "SingleTarget"])
)]
fn ep24_a_target_changes_to_the_source(#[case] line: &str, #[case] selector: Value) {
    let script = activated(line);
    assert_eq!(gaps(&script), Vec::<String>::new(), "{line}");
    assert_eq!(
        effects(&script),
        json!([{"ChangeTargets": {"what": {"Target": 0}, "to": "One", "onto": "This"}}])
    );
    assert_eq!(targets(&script), json!([{"what": {"Object": selector}}]));
}

#[rstest]
#[case::magnet_without_single_change(
    "ChangeTargets | Cost$ U | TargetType$ Spell | ValidTgts$ Card | DefinedMagnet$ Self"
)]
#[case::another_magnet(
    "ChangeTargets | Cost$ U | TargetType$ Spell | ValidTgts$ Card | DefinedMagnet$ Enchanted | ChangeSingleTarget$ True"
)]
#[case::magnet_beside_a_chooser(
    "ChangeTargets | Cost$ U | TargetType$ Spell | ValidTgts$ Card | DefinedMagnet$ Self | ChangeSingleTarget$ True | Optional$ True"
)]
fn ep24_other_magnets_stay_gaps(#[case] line: &str) {
    let found = gaps(&activated(line));
    assert!(
        found.iter().any(|gap| gap.contains("ChangeTargets")),
        "{line}: {found:?}"
    );
}

fn permanent(line: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact\n{line}\nSVar:ConvertMana:DB$ ReplaceMana | ReplaceType$ C\nSVar:Red:DB$ ReplaceMana | ReplaceType$ Red\nSVar:Twice:DB$ ReplaceMana | ReplaceAmount$ 2\n"
    )
}

#[rstest]
#[case::red_kept(
    "S:Mode$ UnspentMana | ValidPlayer$ You | ManaType$ Red | Description$ x",
    json!({"KeepUnspentMana": {"who": "You", "mana": "Red"}})
)]
#[case::every_player_keeps_all(
    "S:Mode$ UnspentMana | Description$ x",
    json!({"KeepUnspentMana": {"who": "EachPlayer"}})
)]
#[case::becomes_colorless(
    "R:Event$ LoseMana | ValidPlayer$ You | ReplacementResult$ Replaced | ReplaceWith$ ConvertMana | ActiveZones$ Battlefield | Description$ x",
    json!({"UnspentManaBecomes": {"who": "You", "into": "Colorless"}})
)]
#[case::becomes_red(
    "R:Event$ LoseMana | ValidPlayer$ You | ReplacementResult$ Replaced | ReplaceWith$ Red | ActiveZones$ Battlefield | Description$ x",
    json!({"UnspentManaBecomes": {"who": "You", "into": "Red"}})
)]
#[case::any_color(
    "S:Mode$ ManaConvert | ValidPlayer$ You | ManaConversion$ AnyType->AnyColor | Description$ x",
    json!({"SpendManaAs": {"who": "You", "spend_as": "AnyColor"}})
)]
#[case::any_type_for_creatures(
    "S:Mode$ ManaConvert | ValidPlayer$ You | ValidCard$ Creature.YouCtrl | ValidSA$ Spell | ManaConversion$ AnyType->AnyType | Description$ x",
    json!({"SpendManaAs": {"who": "You", "spend_as": "AnyType", "spells": [{"Type": "Creature"}]}})
)]
#[case::draw_limit(
    "S:Mode$ CantDraw | ValidPlayer$ Opponent | DrawLimit$ 1 | Description$ x",
    json!({"DrawLimit": {"who": "Opponents", "most": 1}})
)]
#[case::life_for_black(
    "S:Mode$ Continuous | Affected$ You | AddKeyword$ PayLifeInsteadOf:B | Description$ x",
    json!({"PayLifeForMana": {"who": "You", "color": "Black"}})
)]
#[case::spared_sacrifice(
    "S:Mode$ CantSacrifice | ValidCard$ Card.YouCtrl | ValidCause$ SpellAbility.OppCtrl | ForCost$ False | Description$ x",
    json!({"OpponentsCantMakeSacrifice": "You"})
)]
fn ep24_rules_translate(#[case] line: &str, #[case] rule: Value) {
    let script = permanent(line);
    assert_eq!(gaps(&script), Vec::<String>::new(), "{line}");
    assert_eq!(abilities(&script), vec![json!({"Rule": {"rule": rule}})]);
}

#[rstest]
#[case::an_opponent_keeps(
    "S:Mode$ UnspentMana | ValidPlayer$ Opponent | ManaType$ Red | Description$ x"
)]
#[case::another_type_word(
    "S:Mode$ UnspentMana | ValidPlayer$ You | ManaType$ Chosen | Description$ x"
)]
#[case::an_opponent_s_lost_mana(
    "R:Event$ LoseMana | ValidPlayer$ Opponent | ReplacementResult$ Replaced | ReplaceWith$ ConvertMana | ActiveZones$ Battlefield | Description$ x"
)]
#[case::another_replacement(
    "R:Event$ LoseMana | ValidPlayer$ You | ReplacementResult$ Replaced | ReplaceWith$ Twice | ActiveZones$ Battlefield | Description$ x"
)]
#[case::white_only(
    "S:Mode$ ManaConvert | ValidPlayer$ You | ManaConversion$ White->AnyColor | Description$ x"
)]
#[case::abilities_of_creatures(
    "S:Mode$ ManaConvert | ValidPlayer$ You | ValidCard$ Creature.YouCtrl | ValidSA$ Activated | ManaConversion$ AnyType->AnyColor | Description$ x"
)]
#[case::cards_you_dont_own(
    "S:Mode$ ManaConvert | ValidPlayer$ You | ValidCard$ Card.YouDontOwn+YouCtrl | ValidSA$ Spell | ManaConversion$ AnyType->AnyColor | Description$ x"
)]
#[case::life_for_an_opponent_s_mana(
    "S:Mode$ Continuous | Affected$ Opponent | AddKeyword$ PayLifeInsteadOf:B | Description$ x"
)]
#[case::draw_limit_of_a_count(
    "S:Mode$ CantDraw | ValidPlayer$ Opponent | DrawLimit$ X | Description$ x"
)]
#[case::sacrifice_for_costs(
    "S:Mode$ CantSacrifice | ValidCard$ Card.YouCtrl | ValidCause$ SpellAbility.OppCtrl | ForCost$ True | Description$ x"
)]
#[case::sacrifice_of_creatures(
    "S:Mode$ CantSacrifice | ValidCard$ Creature.YouCtrl | ValidCause$ SpellAbility.OppCtrl | ForCost$ False | Description$ x"
)]
fn ep24_other_rules_stay_gaps(#[case] line: &str) {
    assert!(!gaps(&permanent(line)).is_empty(), "{line}");
}

fn tapped(line: &str) -> String {
    format!(
        "Name:T\nTypes:Land\nA:AB$ ManaReflected | Cost$ T | ColorOrType$ Color | ReflectProperty$ Is | {line}\n"
    )
}

#[rstest]
#[case::legendary_permanents(
    "Valid$ Permanent.Legendary+YouCtrl",
    json!([{"Supertype": "Legendary"}, {"ControlledBy": "You"}])
)]
#[case::own_graveyard("Valid$ Defined.ValidGraveyard Card.YouOwn", json!([{"OwnedBy": "You"}, {"In": "Graveyard"}]))]
#[case::itself("Valid$ Defined.Self", json!(["This"]))]
fn ep24_mana_of_a_color_among_translates(#[case] line: &str, #[case] among: Value) {
    let script = tapped(line);
    assert_eq!(gaps(&script), Vec::<String>::new(), "{line}");
    let found = abilities(&script);
    let effects = found
        .first()
        .and_then(|a| a.get("Activated"))
        .and_then(|a| a.get("effects"))
        .cloned();
    assert_eq!(
        effects,
        Some(json!([{"AddMana": {"mana": {"AnyColorAmong": among}}}]))
    );
}

#[rstest]
#[case::imprinted("Valid$ Defined.Imprinted")]
#[case::exiled_with("Valid$ Defined.ExiledWith")]
#[case::counted("Valid$ Permanent.YouCtrl | Amount$ 2")]
fn ep24_other_reflected_colors_stay_gaps(#[case] line: &str) {
    assert!(!gaps(&tapped(line)).is_empty(), "{line}");
}
