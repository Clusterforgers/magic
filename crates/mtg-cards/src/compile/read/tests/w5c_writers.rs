//! Goal: the writers of the step leftovers give the oracle wording of a trigger that triggers an additional time, a delayed trigger said after its effect, a damage trigger of at least an amount from a source you control, cost increases for spells and abilities that target, and a permission to cast as long as a count holds; a wrong count, kind, player, zone, step or condition never reads back.
use super::*;

/// The wordings the checker accepts for the rules, for the message of a failed golden.
fn wordings(json: &str) -> Vec<String> {
    written_variants(&ability(json))
}

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    wordings(json)
        .iter()
        .any(|wording| same_text(wording, clause))
}

const FELIX: &str = r#"{"Rule":{"rule":{"TriggerAgain":{"cause":{"DealsDamage":{"source":[{"Type":"Creature"},{"ControlledBy":"You"}],"to":"EachPlayer","combat":true}}}}}}"#;
const VEYRAN: &str = r#"{"Rule":{"rule":{"TriggerAgain":{"cause":{"AnyOf":[{"SpellCast":{"what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}],"by":"You"}},{"SpellCopied":{"what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}],"by":"You"}}]}}}}}"#;
const OHRAN: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":{"Each":[{"Type":"Creature"}]},"combat":true}},"effects":[{"DelayedTrigger":{"when":{"StepBegins":{"step":"EndCombat"}},"effects":[{"Destroy":{"what":"Triggering"}}]}}]}}"#;
const DRAGONBORN: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"ControlledBy":"You"}],"to":"EachPlayer"}},"if":{"Compare":{"left":"Triggering","op":"Ge","right":5}},"effects":[{"Draw":{}}]}}"#;
const KOPALA_SPELLS: &str = r#"{"CostModifier":{"applies_to":["Spell",{"ControlledBy":"Opponents"},{"Targeting":[{"Subtype":"Merfolk"},{"ControlledBy":"You"}]}],"change":"Increase","mana":"{2}"}}"#;
const KOPALA_ABILITIES: &str = r#"{"CostModifier":{"applies_to":[{"Ability":"Activated"},{"ControlledBy":"Opponents"},{"Targeting":[{"Subtype":"Merfolk"},{"ControlledBy":"You"}]}],"change":"Increase","mana":"{2}"}}"#;
const INDOMITABLE: &str = r#"{"Rule":{"rule":{"MayCast":{"what":["This"],"from":"Graveyard"}},"condition":{"Compare":{"left":{"Count":[{"AnyOf":[{"Subtype":"Pirate"},{"Subtype":"Vehicle"}]},"Tapped",{"ControlledBy":"You"}]},"op":"Ge","right":3}}}}"#;

#[rstest]
#[case::felix(
    FELIX,
    "If a creature you control dealing combat damage to a player causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time."
)]
#[case::veyran(
    VEYRAN,
    "If you casting or copying an instant or sorcery spell causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time."
)]
#[case::ohran(
    OHRAN,
    "Whenever ~ deals combat damage to a creature, destroy that creature at end of combat."
)]
#[case::dragonborn(
    DRAGONBORN,
    "Whenever a source you control deals 5 or more damage to a player, draw a card."
)]
#[case::kopala_spells(
    KOPALA_SPELLS,
    "Spells your opponents cast that target a Merfolk you control cost {2} more to cast."
)]
#[case::kopala_abilities(
    KOPALA_ABILITIES,
    "Abilities your opponents activate that target a Merfolk you control cost {2} more to activate."
)]
#[case::indomitable(
    INDOMITABLE,
    "You may cast this card from your graveyard as long as you control three or more tapped Pirates and/or Vehicles."
)]
fn writes_the_oracle_wording(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}: {:?}", wordings(json));
}

#[rstest]
#[case::felix_noncombat(
    FELIX,
    "If a creature you control dealing noncombat damage to a player causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time."
)]
#[case::felix_opponent(
    FELIX,
    "If a creature an opponent controls dealing combat damage to a player causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time."
)]
#[case::veyran_creature(
    VEYRAN,
    "If you casting or copying a creature spell causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time."
)]
#[case::ohran_step(
    OHRAN,
    "Whenever ~ deals combat damage to a creature, destroy that creature at the beginning of the next end step."
)]
#[case::ohran_kind(
    OHRAN,
    "Whenever ~ deals combat damage to an artifact, destroy that creature at end of combat."
)]
#[case::dragonborn_count(
    DRAGONBORN,
    "Whenever a source you control deals 4 or more damage to a player, draw a card."
)]
#[case::dragonborn_player(
    DRAGONBORN,
    "Whenever a source an opponent controls deals 5 or more damage to a player, draw a card."
)]
#[case::dragonborn_combat(
    DRAGONBORN,
    "Whenever a source you control deals 5 or more combat damage to a player, draw a card."
)]
#[case::kopala_kind(
    KOPALA_SPELLS,
    "Spells your opponents cast that target an Elf you control cost {2} more to cast."
)]
#[case::kopala_player(
    KOPALA_SPELLS,
    "Spells you cast that target a Merfolk you control cost {2} more to cast."
)]
#[case::kopala_amount(
    KOPALA_ABILITIES,
    "Abilities your opponents activate that target a Merfolk you control cost {1} more to activate."
)]
#[case::kopala_spells_as_abilities(
    KOPALA_SPELLS,
    "Abilities your opponents activate that target a Merfolk you control cost {2} more to activate."
)]
#[case::indomitable_count(
    INDOMITABLE,
    "You may cast this card from your graveyard as long as you control two or more tapped Pirates and/or Vehicles."
)]
#[case::indomitable_zone(
    INDOMITABLE,
    "You may cast this card from exile as long as you control three or more tapped Pirates and/or Vehicles."
)]
#[case::indomitable_untapped(
    INDOMITABLE,
    "You may cast this card from your graveyard as long as you control three or more Pirates and/or Vehicles."
)]
fn a_changed_clause_does_not_read_back(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

#[test]
fn a_kind_of_damage_keeps_the_permanent_wording() {
    let typed = r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"Type":"Creature"},{"ControlledBy":"You"}],"to":"EachPlayer"}},"if":{"Compare":{"left":"Triggering","op":"Ge","right":3}},"effects":[{"Draw":{}}]}}"#;
    assert!(written(
        typed,
        "Whenever a creature you control deals 3 or more damage to a player, draw a card."
    ));
    assert!(!written(
        typed,
        "Whenever a source you control deals 3 or more damage to a player, draw a card."
    ));
}
