//! Goal: the checker repairs the slips of a model that the file and the printed text settle (ids and names of the card, the faces of the card, an unusable Manual span, an ability without effects, the half of a token's size the text prints, a target index shifted as a whole, `Triggering` for the one unused target, a target declared from the printed phrase, an invented `X`), leaves every slip with more than one reading alone, and never lets a repair turn a wrong proposal into an accepted one. The rules are trimmed from stored proposals of the local model.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::Library;
use mtg_cards::proposal::{RepairClass, Report, Verdict, check};
use mtg_cards::support::{PrintedCard, PrintedFace};
use rstest::rstest;

use RepairClass::{
    Faces, Identity, InventedX, ManualSpan, NoEffect, TargetDeclared, TargetIndex, TargetUnused,
    TokenSize, TriggeringTarget,
};

/// A card as imported: the faces are `(name, type line, oracle text)`.
fn card(name: &str, id: &str, faces: &[(&str, &str, &str)]) -> PrintedCard {
    PrintedCard {
        oracle_id: id.to_owned(),
        name: name.to_owned(),
        faces: faces
            .iter()
            .map(|(name, type_line, text)| PrintedFace {
                name: (*name).to_owned(),
                type_line: (*type_line).to_owned(),
                oracle_text: (*text).to_owned(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

fn judge(printed: &PrintedCard, rules: &str) -> Report {
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let names: BTreeSet<String> = std::iter::once(printed.name.to_lowercase())
        .chain(printed.faces.iter().map(|face| face.name.to_lowercase()))
        .collect();
    check(rules, printed, &tokens, &names, &Caps::everything())
}

fn classes(report: &Report) -> Vec<RepairClass> {
    report.repairs.iter().map(|repair| repair.class).collect()
}

fn has_problem(report: &Report, text: &str) -> bool {
    report.problems.iter().any(|p| p.message.contains(text))
}

fn cavern() -> PrintedCard {
    card(
        "Cavern of Souls",
        "89ca686a-7c72-4d8f-9290-e89635624a83",
        &[(
            "Cavern of Souls",
            "Land",
            "As this land enters, choose a creature type.\n{T}: Add {C}.\n{T}: Add one mana of any color. Spend this mana only to cast a creature spell of the chosen type, and that spell can't be countered.",
        )],
    )
}

fn mox_amber() -> PrintedCard {
    card(
        "Mox Amber",
        "7a43bd27-fdd8-41f0-9bc4-92568f3408f1",
        &[(
            "Mox Amber",
            "Legendary Artifact",
            "{T}: Add one mana of any color among legendary creatures and planeswalkers you control.",
        )],
    )
}

fn breach() -> PrintedCard {
    card(
        "Breach the Multiverse",
        "ebbd5b9c-07ff-4b42-81c2-ba09539f3a42",
        &[(
            "Breach the Multiverse",
            "Sorcery",
            "Each player mills ten cards. For each player, choose a creature or planeswalker card in that player's graveyard. Put those cards onto the battlefield under your control. Then each creature you control becomes a Phyrexian in addition to its other types.",
        )],
    )
}

fn clock() -> PrintedCard {
    card(
        "Midnight Clock",
        "c68faebc-b2cd-461b-b93e-e1fcd4816810",
        &[(
            "Midnight Clock",
            "Artifact",
            "{T}: Add {U}.\n{2}{U}: Put an hour counter on this artifact.\nAt the beginning of each upkeep, put an hour counter on this artifact.\nWhen the twelfth hour counter is put on this artifact, shuffle your hand and graveyard into your library, then draw seven cards. Exile this artifact.",
        )],
    )
}

fn malakir() -> PrintedCard {
    card(
        "Malakir Rebirth // Malakir Mire",
        "a731e87b-8d99-4b64-8ee3-8e540d652366",
        &[
            (
                "Malakir Rebirth",
                "Instant",
                "Choose target creature. You lose 2 life. Until end of turn, that creature gains \"When this creature dies, return it to the battlefield tapped under its owner's control.\"",
            ),
            (
                "Malakir Mire",
                "Land",
                "This land enters tapped.\n{T}: Add {B}.",
            ),
        ],
    )
}

fn tutor() -> PrintedCard {
    card(
        "Worldly Tutor",
        "e8863518-0bfa-49c3-8c6e-6c9116a81051",
        &[(
            "Worldly Tutor",
            "Instant",
            "Search your library for a creature card, reveal it, then shuffle and put the card on top.",
        )],
    )
}

fn saga() -> PrintedCard {
    card(
        "Urza's Saga",
        "4c6a0c30-b547-4eff-8ff4-0ca25803c076",
        &[(
            "Urza's Saga",
            "Enchantment Land — Urza's Saga",
            "(As this Saga enters and after your draw step, add a lore counter. Sacrifice after III.)\nI — This Saga gains \"{T}: Add {C}.\"\nII — This Saga gains \"{2}, {T}: Create a 0/0 colorless Construct artifact creature token with 'This token gets +1/+1 for each artifact you control.'\"\nIII — Search your library for an artifact card with mana cost {0} or {1}, put it onto the battlefield, then shuffle.",
        )],
    )
}

fn sword() -> PrintedCard {
    card(
        "Sword of Hearth and Home",
        "913e6182-706a-4872-8c8a-e146b0ae0738",
        &[(
            "Sword of Hearth and Home",
            "Artifact — Equipment",
            "Equipped creature gets +2/+2 and has protection from green and from white.\nWhenever equipped creature deals combat damage to a player, exile up to one target creature you own, then search your library for a basic land card. Put both cards onto the battlefield under your control, then shuffle.\nEquip {2}",
        )],
    )
}

fn speaker() -> PrintedCard {
    card(
        "Formidable Speaker",
        "5bd06f9e-9a42-455d-ad26-24bc04e51c7c",
        &[(
            "Formidable Speaker",
            "Creature — Elf Druid",
            "When this creature enters, you may discard a card. If you do, search your library for a creature card, reveal it, put it into your hand, then shuffle.\n{1}, {T}: Untap another target permanent.",
        )],
    )
}

fn drown() -> PrintedCard {
    card(
        "Drown in the Loch",
        "0f264e5b-264e-4e97-9a8d-8ae1d6a286ce",
        &[(
            "Drown in the Loch",
            "Instant",
            "Choose one —\n• Counter target spell with mana value less than or equal to the number of cards in its controller's graveyard.\n• Destroy target creature with mana value less than or equal to the number of cards in its controller's graveyard.",
        )],
    )
}

fn bolt() -> PrintedCard {
    card(
        "Lightning Bolt",
        "4457ed35-7c10-48c8-9776-456485fdf070",
        &[(
            "Lightning Bolt",
            "Instant",
            "Lightning Bolt deals 3 damage to any target.",
        )],
    )
}

const CAVERN: &str = r#"{"faces":[{"abilities":[{"Replacement":{"by":{"Choose":"CreatureType"},"when":{"Enters":{"what":["This"]}}}},{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}},{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1},"only":[{"Spell":{"types":["Creature"],"without":[]}}]} } ]}},{"Manual":{"reason":"that spell can't be countered","span":{"end":1,"start":1}}}]}] }"#;
const MOX_AMBER: &str = r#"{"faces":[{"abilities":[{"Manual":{"reason":"Add one mana of any color among legendary creatures and planeswalkers you control","span":{"end":99,"start":0}}}]}]}"#;
const BREACH: &str = r#"{"faces":[{"abilities":[{"Spell":{"effects":[{"Mill":{"who":"EachPlayer"}},{"Mill":{"who":"EachPlayer"}}]}},{"Manual":{"reason":"For each player, choose a creature or planeswalker card in that player's graveyard.","span":{"end":1,"start":0}}},{"Manual":{"reason":"Put those cards onto the battlefield under your control.","span":{"end":1,"start":0}}},{"Manual":{"reason":"Then each creature you control becomes a Phyrexian in addition to its other types.","span":{"end":1,"start":0}}}]}]}"#;
const CLOCK: &str = r#"{"faces":[{"name":"Midnight Clock","oracle_fingerprint":"5c5d6a1f-9b72-4d3c-9e73-1c7b8c2f5a3d"}],"impl_status":"Partial","notes":"Partial implementation: final condition written as Manual","oracle_id":"5c5d6a1f-9b72-4d3c-9e73-1c7b8c2f5a3d"}"#;
const MALAKIR: &str = r#"{"faces":[{"name":"Malakir Rebirth","oracle_fingerprint":"x"}],"name":"Malakir Rebirth","oracle_id":"..."}"#;
const TUTOR: &str = r#"{"faces":[{"abilities":[{"Spell":{"effects":[{"Search":{"as":"card","filter":[{"Type":"Creature"}],"reveal":true}},{"Shuffle":{}}]}}]},{"abilities":[{"Manual":{"reason":"put the card on top","span":{"end":0,"start":0}}}]}]}"#;
const SAGA: &str = r#"{"faces":[{"abilities":[{"Activated":{"cost":["Tap","Untap"],"effects":[{"CreateToken":{"token":{"Custom":{"name":"Construct","types":["Artifact","Creature"],"subtypes":["Construct"],"supertypes":[],"toughness":0}},"as":"token"}}]}}]}]}"#;
const SWORD: &str = r#"{"faces":[{"abilities":[{"Triggered":{"effects":[{"Exile":{"what":{"Target":1},"as":"creature"}},{"Search":{"as":"land","filter":[{"Supertype":"Basic"},{"Type":"Land"}]}},{"Move":{"to":{"Battlefield":{}},"what":{"Bound":"land"}}},{"Shuffle":{}}],"when":{"DealsDamage":{"source":[{"Is":"Attached"}],"to":"EachPlayer"}},"targets":[{"what":{"Object":[{"OwnedBy":"You"},{"Type":"Creature"}]},"bounds":{"min":0}}]}}]}]}"#;
const SPEAKER: &str = r#"{"faces":[{"abilities":[{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"Untap":{"what":"Triggering"}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"Type":"Artifact"},{"Type":"Enchantment"},{"Type":"Planeswalker"},{"Type":"Land"}]}}]}}]}]}"#;
const DROWN: &str = r#"{"faces":[{"abilities":[{"Spell":{"effects":[],"targets":[]}}]}]}"#;

#[rstest]
#[case::an_empty_span(cavern(), CAVERN, &[ManualSpan], "span is empty")]
#[case::a_span_past_the_text(mox_amber(), MOX_AMBER, &[ManualSpan], "span ends past")]
#[case::spans_over_one_another(breach(), BREACH, &[ManualSpan, ManualSpan], "overlaps an earlier")]
#[case::an_id_of_another_card(clock(), CLOCK, &[Identity], "oracle id")]
#[case::a_card_of_two_faces_with_one(malakir(), MALAKIR, &[Identity, Identity, Faces], "face(s)")]
#[case::faces_that_are_abilities(tutor(), TUTOR, &[Faces, ManualSpan], "missing field")]
#[case::a_creature_token_without_power(saga(), SAGA, &[TokenSize], "creature token needs")]
#[case::targets_counted_from_one(sword(), SWORD, &[TargetIndex], "does not exist")]
#[case::triggering_for_the_target(speaker(), SPEAKER, &[TriggeringTarget], "Triggering")]
#[case::an_ability_without_effects(drown(), DROWN, &[NoEffect], "at least one effect")]
fn a_slip_with_one_reading_is_repaired_before_the_file_is_judged(
    #[case] printed: PrintedCard,
    #[case] rules: &str,
    #[case] repaired: &[RepairClass],
    #[case] problem: &str,
) {
    let report = judge(&printed, rules);
    assert_eq!(classes(&report), repaired, "{}", report.render(false));
    assert!(!has_problem(&report, problem), "{}", report.render(false));
    assert!(report.rules.is_some());
}

#[test]
fn the_repaired_token_has_the_power_the_text_prints() {
    let report = judge(&saga(), SAGA);
    let power = report
        .rules
        .as_ref()
        .and_then(|rules| serde_json::to_value(rules).ok())
        .map(|value| value.to_string());
    assert!(
        power.is_some_and(|json| json.contains(r#""power":0"#)),
        "{}",
        report.render(false)
    );
}

#[test]
fn a_wrong_repair_ends_as_a_mismatch_never_accepted() {
    // The reason quotes the end of the mana clause that an ability also stands for: the repair drops the Manual and
    // the read-back judges that ability against the printed clause, which says more.
    let report = judge(&cavern(), CAVERN);
    assert_eq!(
        report.verdict(),
        Verdict::Mismatch,
        "{}",
        report.render(false)
    );
}

#[test]
fn the_report_lists_what_was_repaired() {
    let text = judge(&cavern(), CAVERN).render(false);
    assert!(text.contains("repaired  unusable Manual span"), "{text}");
}

#[test]
fn a_file_that_needs_nothing_is_not_repaired() {
    let report = judge(
        &bolt(),
        r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}]}]}"#,
    );
    assert_eq!(classes(&report), []);
    assert_eq!(report.verdict(), Verdict::Accepted);
}

const MANUAL_IN_TEXT: &str = r#"{"faces":[{"abilities":[{"Manual":{"reason":"choose a creature type","span":{"start":0,"end":45}}}]}]}"#;
const TWO_TARGETS_SKIPPED: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":1},"amount":3}},{"Draw":{"who":{"Target":2}}}]}}]}]}"#;
const TWO_UNUSED_TARGETS: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"},{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":"Triggering","amount":3}}]}}]}]}"#;
const TRIGGERED_WITH_UNUSED_TARGET: &str = r#"{"faces":[{"abilities":[{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Untap":{"what":"Triggering"}}],"targets":[{"what":"AnyTarget"}]}}]}]}"#;
const MODAL_SHIFTED: &str = r#"{"faces":[{"abilities":[{"Spell":{"effects":[{"Modal":{"choose":{"min":1,"max":1},"modes":[{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":1},"amount":3}}]},{"effects":[{"Draw":{}}]}]}}]}}]}]}"#;
const TOUGHNESS_NOT_PRINTED: &str = r#"{"faces":[{"abilities":[{"Activated":{"cost":["Tap"],"effects":[{"CreateToken":{"token":{"Custom":{"name":"Construct","types":["Artifact","Creature"],"subtypes":["Construct"],"toughness":5}}}}]}}]}]}"#;
const EXTRA_FACE_NAMED_OTHERWISE: &str =
    r#"{"faces":[{"abilities":[]},{"name":"Other Face","abilities":[]}]}"#;
const EXTRA_FACE_WITH_NOTES: &str = r#"{"faces":[{"abilities":[]},{"abilities":[],"notes":"x"}]}"#;

fn knights() -> PrintedCard {
    card(
        "Twin Knights",
        "00000000-0000-4000-8000-000000000001",
        &[(
            "Twin Knights",
            "Sorcery",
            "Create a 2/2 white Knight creature token. Create a 3/2 black Knight creature token.",
        )],
    )
}

#[rstest]
#[case::a_manual_with_a_usable_span(cavern(), MANUAL_IN_TEXT, ManualSpan, "")]
#[case::targets_skipped_in_the_middle(bolt(), TWO_TARGETS_SKIPPED, TargetIndex, "does not exist")]
#[case::two_unused_targets_for_triggering(
    bolt(),
    TWO_UNUSED_TARGETS,
    TriggeringTarget,
    "Triggering"
)]
#[case::triggering_in_a_triggered_ability(
    speaker(),
    TRIGGERED_WITH_UNUSED_TARGET,
    TriggeringTarget,
    ""
)]
#[case::targets_of_a_mode(bolt(), MODAL_SHIFTED, TargetIndex, "")]
#[case::a_toughness_the_text_never_prints(
    saga(),
    TOUGHNESS_NOT_PRINTED,
    TokenSize,
    "creature token needs"
)]
#[case::two_powers_for_one_toughness(knights(), r#"{"faces":[{"abilities":[{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Knight","types":["Creature"],"subtypes":["Knight"],"toughness":2}}}}]}}]}]}"#, TokenSize, "creature token needs")]
#[case::an_extra_face_with_another_name(
    tutor(),
    EXTRA_FACE_NAMED_OTHERWISE,
    Faces,
    "face names joined"
)]
#[case::an_extra_face_with_other_keys(tutor(), EXTRA_FACE_WITH_NOTES, Faces, "unknown field")]
#[case::three_faces_for_a_card_of_two(
    malakir(),
    r#"{"faces":[{"abilities":[]},{"abilities":[]},{"abilities":[]}]}"#,
    Faces,
    "missing field"
)]
fn a_slip_with_no_single_reading_is_left_alone(
    #[case] printed: PrintedCard,
    #[case] rules: &str,
    #[case] class: RepairClass,
    #[case] still: &str,
) {
    let report = judge(&printed, rules);
    assert!(
        !classes(&report).contains(&class),
        "{}",
        report.render(false)
    );
    assert!(
        still.is_empty() || has_problem(&report, still),
        "{}",
        report.render(false)
    );
}

const BOLT_UNDECLARED: &str = r#"{"faces":[{"abilities":[{"Spell":{"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}]}]}"#;
const BOLT_UNDECLARED_FOUR: &str = r#"{"faces":[{"abilities":[{"Spell":{"effects":[{"DealDamage":{"to":{"Target":0},"amount":4}}]}}]}]}"#;
const BOLT_X: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"X"}}]}}]}]}"#;
const BOLT_UNUSED_TARGET: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":"You","amount":3}}]}}]}]}"#;

#[rstest]
#[case::undeclared_target(BOLT_UNDECLARED, TargetDeclared)]
#[case::invented_x(BOLT_X, InventedX)]
fn a_repaired_proposal_is_judged_like_any_other(#[case] rules: &str, #[case] class: RepairClass) {
    let report = judge(&bolt(), rules);
    assert_eq!(classes(&report), [class]);
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{}",
        report.render(false)
    );
}

#[test]
fn a_repair_of_the_target_never_turns_a_wrong_amount_into_an_accepted_card() {
    let report = judge(&bolt(), BOLT_UNDECLARED_FOUR);
    assert_eq!(classes(&report), [TargetDeclared]);
    assert_eq!(
        report.verdict(),
        Verdict::Mismatch,
        "{}",
        report.render(false)
    );
}

#[test]
fn a_target_the_text_names_is_not_dropped_for_being_unused() {
    let report = judge(&bolt(), BOLT_UNUSED_TARGET);
    assert!(!classes(&report).contains(&TargetUnused));
    assert_eq!(report.verdict(), Verdict::Invalid);
    assert!(has_problem(&report, "is never used"));
}
