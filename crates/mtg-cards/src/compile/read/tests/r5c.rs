//! Goal: a selector whose parts come in another order than the readers give writes back as the same group (a
//! conjunction), and a permanent held down through its controller's next untap step writes back with its subject and
//! its step; another zone, owner, controller, group or step reads `Differs`, never the clause.
use super::*;

/// Whether one wording of the ability is the clause.
fn writes_as(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, clause))
}

const KNIGHT: &str = r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":{"Count":[{"Type":"Land"},{"OwnedBy":"You"},{"In":"Graveyard"}]},"toughness":{"Count":[{"Type":"Land"},{"OwnedBy":"You"},{"In":"Graveyard"}]}}}]}}"#;
const RECLAIM: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"Move":{"to":"Hand","what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"OwnedBy":"You"},{"In":"Graveyard"}]}}]}}"#;
const CHOSEN: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"ChosenSubtype",{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#;
const CAGE: &str = r#"{"Activated":{"cost":"{3}","effects":[{"Continuous":{"affected":{"Target":0},"duration":"NextUntapStep","mods":["DoesntUntap"]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const RECKONING: &str = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"}]}}},{"Continuous":{"affected":{"Each":[{"Type":"Land"},{"ControlledBy":"You"}]},"duration":"NextUntapStep","mods":["DoesntUntap"]}}]}}"#;
const APES: &str = r#"{"Triggered":{"effects":[{"Continuous":{"affected":"This","duration":"NextUntapStep","mods":["DoesntUntap"]}}],"when":{"Attacks":{"what":["This"]}}}}"#;
const CLERIC: &str = r#"{"Triggered":{"effects":[{"Continuous":{"affected":"Triggering","duration":"NextUntapStep","mods":["DoesntUntap"]}}],"when":{"Attacks":{"what":[{"Type":"Creature"}]}}}}"#;

#[rstest]
#[case::graveyard_count(KNIGHT, "~ gets +1/+1 for each land card in your graveyard.")]
#[case::graveyard_target(
    RECLAIM,
    "{T}: Return target creature card from your graveyard to your hand."
)]
#[case::chosen_type_last(CHOSEN, "Creatures you control of the chosen type get +1/+1.")]
#[case::target_held(
    CAGE,
    "{3}: Target creature doesn't untap during its controller's next untap step."
)]
#[case::your_lands_held(
    RECKONING,
    "Destroy all creatures. Lands you control don't untap during your next untap step."
)]
#[case::itself_held(
    APES,
    "Whenever ~ attacks, it doesn't untap during its controller's next untap step."
)]
#[case::blocked_creature_held(
    CLERIC,
    "Whenever a creature attacks, that creature doesn't untap during its controller's next untap step."
)]
fn r5c_a_rule_writes_back_as_its_clause(#[case] json: &str, #[case] clause: &str) {
    assert!(
        writes_as(json, clause),
        "{clause}: {:?}",
        written_variants(&ability(json))
    );
}

#[rstest]
#[case::other_zone(KNIGHT, "In\":\"Graveyard", "In\":\"Hand")]
#[case::other_owner(KNIGHT, "OwnedBy\":\"You\"},{\"In", "OwnedBy\":\"Opponents\"},{\"In")]
#[case::target_from_exile(RECLAIM, "In\":\"Graveyard", "In\":\"Exile")]
#[case::anyones_creatures(CHOSEN, ",{\"ControlledBy\":\"You\"}", "")]
#[case::opponents_creatures(CHOSEN, "ControlledBy\":\"You", "ControlledBy\":\"Opponents")]
#[case::until_end_of_turn(CAGE, "NextUntapStep", "UntilEndOfTurn")]
#[case::all_lands(RECKONING, ",{\"ControlledBy\":\"You\"}", "")]
#[case::target_not_itself(APES, "\"affected\":\"This\"", "\"affected\":\"Triggering\"")]
#[case::itself_not_the_attacker(CLERIC, "\"affected\":\"Triggering\"", "\"affected\":\"This\"")]
fn r5c_a_wrong_rule_differs(#[case] json: &str, #[case] from: &str, #[case] to: &str) {
    let clause = match json {
        KNIGHT => "~ gets +1/+1 for each land card in your graveyard.",
        RECLAIM => "{T}: Return target creature card from your graveyard to your hand.",
        CHOSEN => "Creatures you control of the chosen type get +1/+1.",
        CAGE => "{3}: Target creature doesn't untap during its controller's next untap step.",
        RECKONING => {
            "Destroy all creatures. Lands you control don't untap during your next untap step."
        }
        APES => "Whenever ~ attacks, it doesn't untap during its controller's next untap step.",
        _ => {
            "Whenever a creature attacks, that creature doesn't untap during its controller's next untap step."
        }
    };
    let wrong = json.replacen(from, to, 1);
    assert_ne!(wrong, json, "{from}");
    assert!(!writes_as(&wrong, clause), "{wrong}");
}

#[test]
fn r5c_a_zone_named_inside_another_part_keeps_its_order() {
    let json = r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":{"Count":[{"OwnedBy":"You"},{"AnyOf":[{"In":"Hand"},{"In":"Graveyard"}]},{"In":"Exile"}]},"toughness":0}}]}}"#;
    let Ability::Static { mods, .. } = ability(json) else {
        panic!("not a static");
    };
    let reordered = super::super::r5c_order::r5c_reordered(&ability(json));
    assert!(reordered.is_empty(), "{mods:?}");
}
