//! Goal: combat measured from a creature's owner ("can't attack its owner", "can't be blocked unless it's attacking its owner or a permanent its owner controls", "deals combat damage to its owner") and "untap ~ and draw a card" read and write back as printed, and a near-miss stays with the table.
use super::*;

const BEAST: &str =
    "Whenever a creature deals combat damage to its owner, untap ~ and draw a card.";
const BEAST_JSON: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"Type":"Creature"}],"to":{"OwnerOf":"Dealer"},"combat":true}},"effects":[{"Untap":{"what":"This"}},{"Draw":{}}]}}"#;
const PILOT: &str = "Enchanted creature gets +2/+2 and can't be blocked unless it's attacking its owner or a permanent its owner controls.";
const PILOT_JSON: &str = r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":2,"toughness":2}},"CantBeBlockedUnlessAttackingOwner"]}}"#;
const ALONE_JSON: &str =
    r#"{"Static":{"affected":"Attached","mods":["CantBeBlockedUnlessAttackingOwner"]}}"#;
const MUST_ATTACK: &str = r#"{"Rule":{"rule":{"MustAttack":"This"}}}"#;

fn owner_static(attacked: &str) -> String {
    format!(r#"{{"Static":{{"affected":"This","mods":[{{"CantAttackOwner":"{attacked}"}}]}}}}"#)
}

#[rstest]
#[case::damage_to_its_owner(BEAST, ClauseKind::Triggered, BEAST_JSON)]
#[case::untap_and_draw(
    "Whenever ~ attacks, tap ~ and draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Tap":{"what":"This"}},{"Draw":{}}]}}"#
)]
#[case::counter_and_draw(
    "Whenever ~ attacks, put a +1/+1 counter on ~ and draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}},{"Draw":{}}]}}"#
)]
#[case::bonus_and_unblockable_unless(PILOT, ClauseKind::Static, PILOT_JSON)]
#[case::unblockable_unless_alone(
    "Enchanted creature can't be blocked unless it's attacking its owner or a permanent its owner controls.",
    ClauseKind::Static,
    ALONE_JSON
)]
#[case::cant_attack_its_owner(
    "Enchanted creature can't attack its owner.",
    ClauseKind::Static,
    r#"{"Static":{"affected":"Attached","mods":[{"CantAttackOwner":"Player"}]}}"#
)]
fn r371_owner_wordings_read_back(#[case] text: &str, #[case] kind: ClauseKind, #[case] json: &str) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::both(
    "Both",
    "~ attacks each combat if able and can't attack its owner or planeswalkers its owner controls."
)]
#[case::player("Player", "~ attacks each combat if able and can't attack its owner.")]
#[case::planeswalkers(
    "Planeswalkers",
    "~ attacks each combat if able and can't attack planeswalkers its owner controls."
)]
fn r371_attacks_and_cant_attack_its_owner_read_as_two_abilities(
    #[case] attacked: &str,
    #[case] text: &str,
) {
    let read = static_pair(text, false, &Own::default()).expect("read");
    assert_eq!(
        read,
        vec![ability(MUST_ATTACK), ability(&owner_static(attacked))]
    );
    let written = super::super::static_pairs::texts(&read[0], &read[1]);
    assert!(written.iter().any(|w| same_text(w, text)), "{written:?}");
}

#[rstest]
#[case::the_player_alone_is_not_the_owner(
    "~ attacks each combat if able and can't attack you.",
    &owner_static("Player")
)]
#[case::another_rule(
    "~ blocks each combat if able and can't attack its owner.",
    &owner_static("Player")
)]
fn r371_owner_pair_near_misses(#[case] text: &str, #[case] restriction: &str) {
    let read = static_pair(text, false, &Own::default());
    assert_ne!(
        read,
        Some(vec![ability(MUST_ATTACK), ability(restriction)]),
        "{text}"
    );
    let written = super::super::static_pairs::texts(&ability(MUST_ATTACK), &ability(restriction));
    assert!(!written.iter().any(|w| same_text(w, text)), "{written:?}");
}

#[rstest]
#[case::to_its_controller(
    "Whenever a creature deals combat damage to its controller, untap ~ and draw a card."
)]
#[case::unless_attacking_you(
    "Enchanted creature can't be blocked unless it's attacking you or a permanent you control."
)]
#[case::and_then_another_verb("Whenever ~ attacks, tap ~ and draw a card and discard a card.")]
#[case::after_a_players_step(
    "Whenever ~ attacks, you gain 3 life and draw a card and discard a card."
)]
fn r371_owner_near_misses_stay_by_hand(#[case] text: &str) {
    let kind = if text.starts_with("Whenever") {
        ClauseKind::Triggered
    } else {
        ClauseKind::Static
    };
    let read = clause(text, kind, false).map(|read| serde_json::to_string(&read).unwrap());
    assert!(
        !read
            .as_deref()
            .is_some_and(|json| json.contains("Owner") || json.contains("Draw")),
        "{text}: {read:?}"
    );
}

#[rstest]
#[case::its_owner(
    "~ attacks each combat if able, can't be sacrificed, and can't attack its owner.",
    "Player"
)]
#[case::and_its_planeswalkers(
    "~ attacks each combat if able, can't be sacrificed, and can't attack its owner or planeswalkers its owner controls.",
    "Both"
)]
fn r371_attacks_unsacrificed_reads_as_three_abilities(#[case] text: &str, #[case] attacked: &str) {
    let unsacrificed = ability(r#"{"Rule":{"rule":{"Cant":{"who":"This","action":"Sacrifice"}}}}"#);
    let read = static_pair(text, false, &Own::default()).expect("read");
    assert_eq!(
        read,
        vec![
            ability(MUST_ATTACK),
            unsacrificed.clone(),
            ability(&owner_static(attacked))
        ]
    );
    let written = super::super::r371_owner::r371_three_texts(&read[0], &read[1], &read[2]);
    assert!(written.iter().any(|w| same_text(w, text)), "{written:?}");
    let other = ability(r#"{"Rule":{"rule":{"Cant":{"who":"This","action":"Discard"}}}}"#);
    assert_eq!(
        super::super::r371_owner::r371_three_texts(&read[0], &other, &read[2]),
        Vec::<String>::new()
    );
    assert_eq!(
        static_pair(
            "~ attacks each combat if able, can't be sacrificed, and can't attack you.",
            false,
            &Own::default()
        ),
        None
    );
}
