//! Goal: forage reads as the cost part `Forage` wherever a cost is read (an activation, a choice of additional costs, a
//! payment after "you may") and is written back as the card words it; a near miss reads as something else or not at all.
use super::*;

/// Whether one of the wordings of the ability is the text.
fn writes(json: &str, text: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

const CAMELLIA: &str = r#"{"Activated":{"cost":[{"Mana":"{2}"},"Forage"],"effects":[{"PutCounters":{"on":{"Each":[{"Subtype":"Squirrel"},"Other",{"ControlledBy":"You"}]},"counter":"+1/+1"}}]}}"#;
const FORAGER: &str = r#"{"Activated":{"cost":["Tap","Forage"],"effects":[{"AddMana":{"mana":{"AnyCombination":{"count":2}}}}]}}"#;
const FEED: &str = r#"{"AdditionalCost":{"cost":[{"OneOf":[["Forage"],"{B}"]}],"required":true}}"#;
const BODYGUARD: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Pay":{"cost":["Forage"],"paid":[{"PutCounters":{"on":"This","counter":"+1/+1","count":2}}]}}]}}"#;

#[rstest]
#[case::camellia(
    "{2}, Forage: Put a +1/+1 counter on each other Squirrel you control.",
    ClauseKind::Activated,
    CAMELLIA
)]
#[case::thornvault_forager(
    "{T}, Forage: Add two mana in any combination of colors.",
    ClauseKind::Activated,
    FORAGER
)]
#[case::feed_the_cycle(
    "As an additional cost to cast this spell, forage or pay {B}.",
    ClauseKind::Static,
    FEED
)]
#[case::bushy_bodyguard(
    "When this creature enters, you may forage. If you do, put two +1/+1 counters on it.",
    ClauseKind::Triggered,
    BODYGUARD
)]
fn forage_reads_as_a_cost_part(#[case] text: &str, #[case] kind: ClauseKind, #[case] json: &str) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::exile_instead(
    CAMELLIA,
    "{2}, Exile three cards from your graveyard: Put a +1/+1 counter on each other Squirrel you control."
)]
#[case::sacrifice_a_food(
    FORAGER,
    "{T}, Sacrifice a Food: Add two mana in any combination of colors."
)]
#[case::forage_and_pay(FEED, "As an additional cost to cast this spell, forage and pay {B}.")]
#[case::optional_choice(
    FEED,
    "As an additional cost to cast this spell, you may forage or pay {B}."
)]
#[case::must_forage(
    BODYGUARD,
    "When this creature enters, forage. If you do, put two +1/+1 counters on it."
)]
#[case::one_counter(
    BODYGUARD,
    "When this creature enters, you may forage. If you do, put a +1/+1 counter on it."
)]
fn a_near_miss_is_not_written_as_forage(#[case] json: &str, #[case] text: &str) {
    assert!(!writes(json, text), "{text}");
}

#[rstest]
#[case::forages("{2}, Forages: Draw a card.")]
#[case::forage_twice("When this creature enters, you may forage twice. If you do, draw a card.")]
fn a_near_miss_of_the_wording_stays_unread(#[case] text: &str) {
    let kind = if text.starts_with("When") {
        ClauseKind::Triggered
    } else {
        ClauseKind::Activated
    };
    assert_eq!(clause(text, kind, false), None, "{text}");
}
