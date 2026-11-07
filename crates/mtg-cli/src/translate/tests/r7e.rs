//! Goal: round 7 builder e gaps the scripts that name fewer spells than the card prints ("Whenever a player kicks a spell" scripted for the spells you control) and keeps the cast triggers whose script and card agree.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The gaps of an enchantment with the given trigger and a draw.
fn trigger_gaps(trigger: &str) -> (String, Vec<String>) {
    let text = format!(
        "Name:Subject\nTypes:Enchantment\nT:{trigger}\nSVar:TrigDraw:DB$ Draw | NumCards$ 1\n"
    );
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[rstest]
#[case::saproling_infestation(
    "Mode$ SpellCast | ValidCard$ Card.YouCtrl+kicked | TriggerZones$ Battlefield | Execute$ TrigDraw | TriggerDescription$ Whenever a player kicks a spell, draw a card.",
    true
)]
#[case::roost_of_drakes(
    "Mode$ SpellCast | ValidCard$ Card.YouCtrl+kicked | TriggerZones$ Battlefield | Execute$ TrigDraw | TriggerDescription$ Whenever you cast a kicked spell, draw a card.",
    false
)]
#[case::any_player_any_spell(
    "Mode$ SpellCast | ValidCard$ Card.kicked | TriggerZones$ Battlefield | Execute$ TrigDraw | TriggerDescription$ Whenever a player kicks a spell, draw a card.",
    false
)]
fn a_cast_trigger_for_your_spells_printed_for_any_player_is_a_gap(
    #[case] trigger: &str,
    #[case] gapped: bool,
) {
    let (json, found) = trigger_gaps(trigger);
    let named = found
        .iter()
        .any(|gap| gap.contains("spell-you-control-printed-any-player"));
    assert_eq!(named, gapped, "{found:?} {json}");
}

#[rstest]
// The printed "you may" over the one step asks already (r6a): no gap.
#[case::decree_of_savagery(
    "Mode$ Cycled | ValidCard$ Card.Self | Execute$ TrigDraw | TriggerDescription$ When you cycle CARDNAME, you may put a +1/+1 counter on it.",
    false
)]
#[case::with_a_decider(
    "Mode$ Cycled | ValidCard$ Card.Self | OptionalDecider$ You | Execute$ TrigDraw | TriggerDescription$ When you cycle CARDNAME, you may put a +1/+1 counter on it.",
    false
)]
#[case::forced(
    "Mode$ Cycled | ValidCard$ Card.Self | Execute$ TrigDraw | TriggerDescription$ When you cycle CARDNAME, put a +1/+1 counter on it.",
    false
)]
fn a_printed_may_with_no_choice_in_the_script_is_a_gap(
    #[case] trigger: &str,
    #[case] gapped: bool,
) {
    let text = format!(
        "Name:Subject\nTypes:Creature\nPT:1/1\nT:{}\nSVar:TrigPut:DB$ PutCounter | Defined$ Self | CounterType$ P1P1 | CounterNum$ 1\n",
        trigger.replace("TrigDraw", "TrigPut")
    );
    let (json, found) = (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    );
    let named = found
        .iter()
        .any(|gap| gap.contains("may-put-counters-without-decider"));
    assert_eq!(named, gapped, "{found:?} {json}");
}

#[rstest]
#[case::behold_the_beyond("Card", "Mandatory$ True | ", r#""min":3"#)]
#[case::a_stated_quality("Creature", "Mandatory$ True | ", r#""min":0"#)]
#[case::up_to("Card", "", r#""min":0"#)]
fn a_search_for_any_cards_finds_as_many_as_it_names(
    #[case] kind: &str,
    #[case] mandatory: &str,
    #[case] expected: &str,
) {
    let text = format!(
        "Name:Subject\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Library | Destination$ Hand | {mandatory}ChangeType$ {kind} | ChangeNum$ 3 | SpellDescription$ Search.\n"
    );
    let json = serde_json::to_string(&abilities(&text)).unwrap();
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    assert!(json.contains(expected), "{json}");
}

#[rstest]
#[case::rune_of_sustenance("Permanent:permanent", true)]
#[case::artifact_or_creature("Artifact,Creature", true)]
#[case::a_creature_aura("Creature", false)]
#[case::your_creature("Creature.YouCtrl", false)]
#[case::aerial_modification("Creature,Vehicle", false)]
fn a_keyword_for_the_creature_an_aura_of_any_permanent_enchants_is_a_gap(
    #[case] enchant: &str,
    #[case] gapped: bool,
) {
    let text = format!(
        "Name:Subject\nTypes:Enchantment Aura\nK:Enchant:{enchant}\nS:Mode$ Continuous | AffectedDefined$ Enchanted | Affected$ Creature | AddKeyword$ Lifelink | Description$ Lifelink.\n"
    );
    let named = gaps(&text)
        .iter()
        .any(|gap| gap.contains("creature-of-an-enchanted-permanent"));
    assert_eq!(named, gapped, "{:?}", gaps(&text));
}

/// A one-face creature card with the given Oracle text.
fn printed(text: &str) -> mtg_cards::support::PrintedCard {
    mtg_cards::support::PrintedCard {
        oracle_id: "00000000-0000-4000-8000-00000000e7e0".into(),
        name: "Alpha".into(),
        keywords: Vec::new(),
        layout: "normal".into(),
        faces: vec![mtg_cards::support::PrintedFace {
            name: "Alpha".into(),
            mana_cost: "{2}{B}".into(),
            type_line: "Creature — Skeleton".into(),
            oracle_text: text.into(),
            power: Some("3".into()),
            toughness: Some("1".into()),
            ..mtg_cards::support::PrintedFace::default()
        }],
    }
}

#[rstest]
#[case::charred_graverobber(
    "Escape—{3}{B}{B}, Exile four other cards from your graveyard.",
    r#"{"Keyword":{"Escape":[{"Mana":"{3}{B}"},{"ExileFromGraveyard":{"count":4,"filter":["Other"]}}]}}"#,
    true
)]
#[case::the_printed_cost(
    "Escape—{3}{B}{B}, Exile four other cards from your graveyard.",
    r#"{"Keyword":{"Escape":[{"Mana":"{3}{B}{B}"},{"ExileFromGraveyard":{"count":4,"filter":["Other"]}}]}}"#,
    false
)]
// The types of an enchant line in another order are the same rule, whatever the reader says of the order.
#[case::an_order_of_types(
    "Enchant artifact or creature",
    r#"{"Keyword":{"Enchant":[{"AnyOf":[{"Type":"Creature"},{"Type":"Artifact"}]}]}}"#,
    false
)]
fn a_keyword_at_another_mana_cost_is_not_offered(
    #[case] text: &str,
    #[case] ability: &str,
    #[case] refused: bool,
) {
    let card = printed(text);
    let library = mtg_cards::load::Library::default();
    let names = std::collections::BTreeSet::new();
    let caps = mtg_engine::caps();
    let json = format!(r#"{{"faces":[{{"abilities":[{ability}]}}]}}"#);
    let report = mtg_cards::proposal::check_exact(&json, &card, &library, &names, &caps);
    let refusal = crate::translate::stored::refusal(&report, &caps);
    assert_eq!(
        refusal.as_deref() == Some("offer:keyword-cost-differs"),
        refused,
        "{refusal:?} {:?}",
        report.clauses
    );
}
