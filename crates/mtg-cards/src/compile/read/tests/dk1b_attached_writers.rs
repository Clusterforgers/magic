//! Goal: cards put onto the battlefield attached to a permanent (a named creature, the card itself), from a graveyard or from a hand or a graveyard, are written back as the oracle wording; a changed host, zone, kind or an attachment that is not there never reads back.
use super::*;
use rstest::rstest;

/// Whether some wording the checker accepts for the rules is the clause.
fn dk1b_written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const DK1B_GRAVE_CREATURE: &str =
    r#"{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}"#;

/// The spell that returns a creature and then up to two attachments of `kinds` attached to it, with the destination of
/// the second move as given.
fn dk1b_business(kinds: &str, destination: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{DK1B_GRAVE_CREATURE},{{"what":{{"Object":[{kinds},{{"In":"Graveyard"}},{{"OwnedBy":"You"}}]}},"bounds":{{"min":0,"max":2}}}}],"effects":[{{"Move":{{"what":{{"Target":0}},"to":{{"Battlefield":{{}}}}}}}},{{"Move":{{"what":{{"Target":1}},"to":{destination}}}}}]}}}}"#
    )
}

const DK1B_ATTACHED_TO_CREATURE: &str = r#"{"BattlefieldAttached":{"to":{"Target":0}}}"#;
const DK1B_AURA_OR_EQUIPMENT: &str = r#"{"AnyOf":[{"Subtype":"Aura"},{"Subtype":"Equipment"}]}"#;

/// The trigger of a creature that puts a card of `zones` that is `kind` onto the battlefield, with this destination.
fn dk1b_enters(zones: &str, kind: &str, destination: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{{"May":{{"effects":[{{"Choose":{{"who":"You","from":{{"Each":[{zones},{{"OwnedBy":"You"}},{kind}]}},"as":"card"}}}},{{"Move":{{"what":{{"Bound":"card"}},"to":{destination}}}}}]}}}}]}}}}"#
    )
}

const DK1B_HAND_OR_GRAVEYARD: &str = r#"{"AnyOf":[{"In":"Hand"},{"In":"Graveyard"}]}"#;
const DK1B_HAND: &str = r#"{"In":"Hand"}"#;
const DK1B_ATTACHED_TO_THIS: &str = r#"{"BattlefieldAttached":{"to":"This"}}"#;

#[rstest]
#[case::auras_and_equipment(
    dk1b_business(DK1B_AURA_OR_EQUIPMENT, DK1B_ATTACHED_TO_CREATURE),
    "Return target creature card from your graveyard to the battlefield, then return up to two target Aura and/or Equipment cards from your graveyard to the battlefield attached to that creature."
)]
#[case::auras(
    dk1b_business(r#"{"Subtype":"Aura"}"#, DK1B_ATTACHED_TO_CREATURE),
    "Return target creature card from your graveyard to the battlefield, then return up to two target Aura cards from your graveyard to the battlefield attached to that creature."
)]
#[case::equipment(
    dk1b_business(r#"{"Subtype":"Equipment"}"#, DK1B_ATTACHED_TO_CREATURE),
    "Return target creature card from your graveyard to the battlefield, then return up to two target Equipment cards from your graveyard to the battlefield attached to that creature."
)]
#[case::hand_or_graveyard_to_itself(
    dk1b_enters(DK1B_HAND_OR_GRAVEYARD, DK1B_AURA_OR_EQUIPMENT, DK1B_ATTACHED_TO_THIS),
    "When ~ enters, you may put an Aura or Equipment card from your hand or graveyard onto the battlefield attached to ~."
)]
#[case::an_aura_from_the_hand(
    dk1b_enters(DK1B_HAND, r#"{"Subtype":"Aura"}"#, DK1B_ATTACHED_TO_THIS),
    "When ~ enters, you may put an Aura card from your hand onto the battlefield attached to ~."
)]
#[case::equipment_from_hand_or_graveyard(
    dk1b_enters(
        DK1B_HAND_OR_GRAVEYARD,
        r#"{"Subtype":"Equipment"}"#,
        DK1B_ATTACHED_TO_THIS
    ),
    "When ~ enters, you may put an Equipment card from your hand or graveyard onto the battlefield attached to ~."
)]
fn dk1b_a_card_put_onto_the_battlefield_attached_is_written_as_oracle_says(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(dk1b_written(&json, clause), "{clause}");
}

#[rstest]
#[case::another_host(
    dk1b_business(DK1B_AURA_OR_EQUIPMENT, DK1B_ATTACHED_TO_CREATURE),
    "Return target creature card from your graveyard to the battlefield, then return up to two target Aura and/or Equipment cards from your graveyard to the battlefield attached to that land."
)]
#[case::the_card_itself_as_host(
    dk1b_business(DK1B_AURA_OR_EQUIPMENT, DK1B_ATTACHED_TO_CREATURE),
    "Return target creature card from your graveyard to the battlefield, then return up to two target Aura and/or Equipment cards from your graveyard to the battlefield attached to ~."
)]
#[case::no_attachment_said(
    dk1b_business(DK1B_AURA_OR_EQUIPMENT, DK1B_ATTACHED_TO_CREATURE),
    "Return target creature card from your graveyard to the battlefield, then return up to two target Aura and/or Equipment cards from your graveyard to the battlefield."
)]
#[case::attached_said_for_a_plain_move(
    dk1b_business(DK1B_AURA_OR_EQUIPMENT, r#"{"Battlefield":{}}"#),
    "Return target creature card from your graveyard to the battlefield, then return up to two target Aura and/or Equipment cards from your graveyard to the battlefield attached to that creature."
)]
#[case::another_kind(
    dk1b_business(DK1B_AURA_OR_EQUIPMENT, DK1B_ATTACHED_TO_CREATURE),
    "Return target creature card from your graveyard to the battlefield, then return up to two target Aura and/or Vehicle cards from your graveyard to the battlefield attached to that creature."
)]
#[case::the_graveyard_said_for_a_hand(
    dk1b_enters(DK1B_HAND, DK1B_AURA_OR_EQUIPMENT, DK1B_ATTACHED_TO_THIS),
    "When ~ enters, you may put an Aura or Equipment card from your hand or graveyard onto the battlefield attached to ~."
)]
#[case::the_hand_said_for_a_hand_or_graveyard(
    dk1b_enters(DK1B_HAND_OR_GRAVEYARD, DK1B_AURA_OR_EQUIPMENT, DK1B_ATTACHED_TO_THIS),
    "When ~ enters, you may put an Aura or Equipment card from your hand onto the battlefield attached to ~."
)]
#[case::attached_to_a_target_said_for_itself(
    dk1b_enters(DK1B_HAND_OR_GRAVEYARD, DK1B_AURA_OR_EQUIPMENT, DK1B_ATTACHED_TO_THIS),
    "When ~ enters, you may put an Aura or Equipment card from your hand or graveyard onto the battlefield attached to target creature."
)]
#[case::unattached_move_said_attached(
    dk1b_enters(
        DK1B_HAND_OR_GRAVEYARD,
        DK1B_AURA_OR_EQUIPMENT,
        r#"{"Battlefield":{}}"#
    ),
    "When ~ enters, you may put an Aura or Equipment card from your hand or graveyard onto the battlefield attached to ~."
)]
fn dk1b_a_changed_attachment_never_reads_back(#[case] json: String, #[case] clause: &str) {
    assert!(!dk1b_written(&json, clause), "{clause}");
}
