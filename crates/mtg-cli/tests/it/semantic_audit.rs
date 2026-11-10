//! Goal: each structural check of `semantic-audit` flags a clause whose rules lack the shape its cue asks for and stays
//! silent on the near miss that holds it; a Verified corpus card has no finding, a dropped ability is `unclaused`, the
//! translator's offered drafts are the whole ones nothing shadows, and the mutation score counts what each check catches.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use mtg_cards::card::Card;
use mtg_cli::draft_suspects::printed_of;
use mtg_cli::semantic_audit::checks::{CHECKS, clause_text, facts, run};
use mtg_cli::semantic_audit::{Entry, audit, offered, score};
use mtg_testkit::mutate::{MutationKind, mutant};
use rstest::rstest;
use serde_json::{Value, json};

/// The checks' findings on a clause and its rules.
fn found(clause: &str, rules: &Value) -> Vec<&'static str> {
    let text = clause_text(clause, &["Probe".to_owned()]);
    run(&text, &facts(&[rules]))
        .into_iter()
        .map(|(check, _)| check)
        .collect()
}

fn targets(max: i64, min: i64) -> Value {
    json!([{"bounds": {"min": min, "max": max}, "selector": ["Creature"]}])
}

#[rstest]
#[case::intervening_if(
    "When Probe enters, if you control a Forest, draw a card.",
    json!({"Triggered": {"effects": [{"Draw": {}}]}}),
    &["intervening-if"]
)]
#[case::intervening_if_near(
    "When Probe enters, if you control a Forest, draw a card.",
    json!({"Triggered": {"if": {"Compare": {}}, "effects": [{"Draw": {}}]}}),
    &[]
)]
#[case::for_each(
    "You gain 1 life for each creature you control.",
    json!({"GainLife": {"amount": 1}}),
    &["for-each"]
)]
#[case::for_each_near(
    "You gain 1 life for each creature you control.",
    json!({"GainLife": {"amount": {"Count": ["Creature"]}}}),
    &[]
)]
#[case::may("When Probe enters, you may draw a card.", json!({"Draw": {}}), &["may"])]
#[case::may_near("When Probe enters, you may draw a card.", json!({"May": {"effects": [{"Draw": {}}]}}), &[])]
#[case::may_permission("You may play lands from your graveyard.", json!({"Rule": {"rule": "PlayFrom"}}), &[])]
#[case::target("Destroy target creature.", json!({"Destroy": {"what": "This"}}), &["target"])]
#[case::target_near(
    "Destroy target creature.",
    json!({"Spell": {"targets": targets(1, 1), "effects": [{"Destroy": {"what": {"Target": 0}}}]}}),
    &[]
)]
#[case::target_of_a_spell("Whenever Probe becomes the target of a spell, draw a card.", json!({"Draw": {}}), &[])]
#[case::target_count(
    "Destroy up to two target creatures.",
    json!({"Spell": {"targets": targets(1, 0)}}),
    &["target-count"]
)]
#[case::target_count_up_to(
    "Tap up to two target creatures.",
    json!({"Spell": {"targets": targets(2, 2)}}),
    &["target-count"]
)]
#[case::target_count_near("Destroy up to two target creatures.", json!({"Spell": {"targets": targets(2, 0)}}), &[])]
#[case::duration(
    "Target creature gets +2/+2 until end of turn.",
    json!({"Spell": {"targets": targets(1, 1), "effects": [{"Pump": {"power": 2, "duration": "WhileSourceOnBattlefield"}}]}}),
    &["duration"]
)]
#[case::duration_near(
    "Target creature gets +2/+2 until end of turn.",
    json!({"Spell": {"targets": targets(1, 1), "effects": [{"Pump": {"power": 2, "duration": "UntilEndOfTurn"}}]}}),
    &[]
)]
#[case::each_opponent("Each opponent loses 2 life.", json!({"LoseLife": {"who": "You", "amount": 2}}), &["each-opponent"])]
#[case::each_opponent_near("Each opponent loses 2 life.", json!({"LoseLife": {"who": "Opponents", "amount": 2}}), &[])]
#[case::another(
    "Other creatures you control get +1/+1.",
    json!({"Static": {"affected": {"Each": ["Creature", {"ControlledBy": "You"}]}}}),
    &["another"]
)]
#[case::another_near(
    "Other creatures you control get +1/+1.",
    json!({"Static": {"affected": {"Each": ["Creature", "Other", {"ControlledBy": "You"}]}}}),
    &[]
)]
#[case::other_players("Each other player draws a card.", json!({"Draw": {"who": "Opponents"}}), &[])]
#[case::nontoken(
    "Whenever a nontoken creature you control dies, draw a card.",
    json!({"Triggered": {"when": {"Dies": [{"Type": "Creature"}]}}}),
    &["nontoken"]
)]
#[case::nontoken_near(
    "Whenever a nontoken creature you control dies, draw a card.",
    json!({"Triggered": {"when": {"Dies": [{"Type": "Creature"}, {"Not": "Token"}]}}}),
    &[]
)]
#[case::non_filter(
    "Destroy target nonland permanent.",
    json!({"Spell": {"targets": targets(1, 1)}}),
    &["non-filter"]
)]
#[case::non_filter_near(
    "Destroy target nonland permanent.",
    json!({"Spell": {"targets": [{"selector": [{"Not": {"Type": "Land"}}]}]}}),
    &[]
)]
#[case::number(
    "Probe deals 3 damage to any target.",
    json!({"Spell": {"targets": targets(1, 1), "effects": [{"DealDamage": {"amount": 2}}]}}),
    &["number"]
)]
#[case::number_near(
    "Probe deals 3 damage to any target.",
    json!({"Spell": {"targets": targets(1, 1), "effects": [{"DealDamage": {"amount": 3}}]}}),
    &[]
)]
#[case::cost_before_a_target(
    "{2}: Target creature gains wither until end of turn.",
    json!({"Activated": {"targets": targets(1, 1), "effects": [{"Continuous": {"duration": "UntilEndOfTurn"}}]}}),
    &[]
)]
#[case::each_target_beyond_the_first(
    "This spell costs {2}{R} more to cast for each target beyond the first.",
    json!({"CostModifier": {"per": {"TargetCount": 0}}}),
    &[]
)]
#[case::mana_kept_this_turn(
    "Add {R}. Until end of turn, you don't lose this mana as steps and phases end.",
    json!({"AddMana": {"lasts": "Turn"}}),
    &[]
)]
#[case::counter_kind_is_no_number(
    "Put a +2/+2 counter on target creature.",
    json!({"Spell": {"targets": targets(1, 1), "effects": [{"PutCounters": {"counter": {"Other": "+2/+2"}}}]}}),
    &[]
)]
#[case::if_as_a_filter(
    "When ~ dies, if it isn't a token, draw a card.",
    json!({"Triggered": {"when": {"ZoneChange": {"what": ["This", {"Not": "Token"}]}}}}),
    &[]
)]
#[case::union_with_another(
    "Whenever ~ or another Equipment you control enters, draw a card.",
    json!({"Triggered": {"when": {"ZoneChange": {"what": [{"Subtype": "Equipment"}]}}}}),
    &[]
)]
#[case::number_draw("Draw two cards.", json!({"Draw": {"count": 3}}), &["number"])]
#[case::number_draw_near("Draw two cards.", json!({"Draw": {"count": 2}}), &[])]
#[case::number_mana_list("{T}: Add six {G}.", json!({"AddMana": {"mana": {"Fixed": ["Green"]}}}), &[])]
#[case::number_piles("Separate them into two piles.", json!({"SeparatePiles": {}}), &[])]
#[case::may_opening_hand("You may reveal this card from your opening hand.", json!({"OpeningReveal": {}}), &[])]
#[case::for_each_colour_spent(
    "It enters with a +1/+1 counter for each color of mana spent to cast it.",
    json!({"WithCounters": {"count": "ColorsSpent"}}),
    &[]
)]
#[case::instead_shield(
    "If that spell would be put into your graveyard, exile it instead.",
    json!({"Shield": {"by": {"MoveTo": "Exile"}}}),
    &[]
)]
#[case::other_but_the_target(
    "Each other creature you control becomes a copy of that creature.",
    json!({"BecomeCopy": {"what": {"Each": [{"Type": "Creature"}, {"Not": {"Is": {"Target": 0}}}]}}}),
    &[]
)]
#[case::if_folded_into_the_trigger(
    "When ~ enters, if you cast it, draw a card.",
    json!({"Triggered": {"when": {"ZoneChange": {"what": ["This", {"WasCast": {}}]}}}}),
    &[]
)]
#[case::mana_symbols_are_no_number("{3}, {T}: Draw a card.", json!({"Activated": {"cost": "{3}"}}), &[])]
#[case::reminder_text_is_no_cue("Flying (This creature can't be blocked except by two target creatures.)", json!({"Keyword": "Flying"}), &[])]
#[case::instead("If a creature would die, exile it instead.", json!({"Exile": {}}), &["instead"])]
#[case::instead_near("If a creature would die, exile it instead.", json!({"Replacement": {"by": {"Instead": []}}}), &[])]
#[case::instead_modal("Choose one. If you control a commander, you may choose both instead.", json!({"Modes": {}}), &[])]
#[case::other_land_types("Each land is a Swamp in addition to its other land types.", json!({"Static": {}}), &[])]
#[case::as_another(
    "Whenever a creature enters, if it doesn't have the same name as another creature you control, draw a card.",
    json!({"Triggered": {"if": {}}}),
    &[]
)]
#[case::noncombat("Prevent all noncombat damage.", json!({"Replacement": {"by": "Prevent"}}), &[])]
#[case::alternative_cost("You may pay {B} rather than pay this spell's mana cost.", json!({"AlternativeCost": {}}), &[])]
#[case::modal_count(
    "Choose two — • Target player draws a card.",
    json!({"Spell": {"choose": 2, "targets": targets(1, 1)}}),
    &[]
)]
#[case::loyalty_cost(
    "\u{2212}2: Target creature you control fights target creature you don't control.",
    json!({"Loyalty": {"cost": -2, "targets": [{"selector": []}, {"selector": []}]}}),
    &[]
)]
#[case::mana_for_each_colour(
    "{T}: For each color among permanents you control, add one mana of that color.",
    json!({"AddMana": {"mana": {"EachColorAmong": []}}}),
    &[]
)]
#[case::until_end_of_turn_trigger(
    "Until end of turn, whenever a player taps an Island for mana, that player adds an additional {U}.",
    json!({"DelayedTrigger": {"this_turn": true}}),
    &[]
)]
fn a_check_flags_a_missing_shape_and_not_its_near_miss(
    #[case] clause: &str,
    #[case] rules: Value,
    #[case] expected: &[&str],
) {
    let mut got = found(clause, &rules);
    got.sort_unstable();
    let mut want = expected.to_vec();
    want.sort_unstable();
    assert_eq!(got, want, "{clause}");
}

#[test]
fn every_check_has_a_probe_row() {
    let probes = [
        "intervening-if",
        "for-each",
        "may",
        "target",
        "target-count",
        "duration",
        "each-opponent",
        "another",
        "nontoken",
        "non-filter",
        "number",
        "instead",
    ];
    let names: Vec<&str> = CHECKS.iter().map(|c| c.name).collect();
    assert_eq!(names, probes);
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// A corpus card as the audit reads it.
fn corpus_entry(slug: &str) -> Entry {
    let cards = root().join("crates/mtg-cards/cards");
    let text = std::fs::read_to_string(cards.join(format!("{slug}.json"))).unwrap();
    let card: Card = serde_json::from_str(&text).unwrap();
    let printed = printed_of(&root().join(format!(
        "crates/mtg-cards/tests/fixtures/corpus/{slug}.json"
    )))
    .unwrap();
    Entry {
        slug: slug.to_owned(),
        source: "corpus",
        card,
        printed,
    }
}

const VERIFIED: [&str; 4] = [
    "lightning_bolt",
    "llanowar_elves",
    "counterspell",
    "grizzly_bears",
];

#[rstest]
fn a_verified_corpus_card_has_no_finding(
    #[values("lightning_bolt", "llanowar_elves", "counterspell")] slug: &str,
) {
    let entry = corpus_entry(slug);
    assert_eq!(audit(&entry.card, &entry.printed), vec![], "{slug}");
}

#[test]
fn a_dropped_ability_is_unclaused() {
    let entry = corpus_entry("lightning_bolt");
    let mut card = entry.card.clone();
    card.faces[0].abilities.clear();
    let checks: Vec<&str> = audit(&card, &entry.printed)
        .iter()
        .map(|f| f.check)
        .collect();
    assert_eq!(checks, ["unclaused"]);
}

#[test]
fn offered_drafts_are_whole_and_unshadowed() {
    let tsv = "slug\tname\ttranslated\ttoday\toracle_matches\tgaps\toffered\n\
        a\tA\twhole\tnone\ttrue\t\ttrue\n\
        b\tB\twhole\tcorpus-verified\ttrue\t\ttrue\n\
        c\tC\tpartial\tnone\t\tx\t\n\
        d\tD\twhole\tcompiled-partial\ttrue\t\ttrue\n\
        e\tE\twhole\tnone\tfalse\t\tfalse\n";
    assert_eq!(offered(tsv).into_iter().collect::<Vec<_>>(), ["a", "d"]);
}

#[test]
fn the_mutation_score_counts_each_kind_and_its_catches() {
    let entries: Vec<Entry> = VERIFIED.iter().map(|slug| corpus_entry(slug)).collect();
    let result = score(&entries, 0);
    assert_eq!(result.cards, VERIFIED.len());
    let made: usize = VERIFIED
        .iter()
        .map(|slug| {
            let card = &corpus_entry(slug).card;
            MutationKind::ALL
                .iter()
                .filter(|k| mutant(card, **k, 0).is_some())
                .count()
        })
        .sum();
    assert_eq!(
        result.kinds.values().map(|t| t.mutants).sum::<usize>(),
        made
    );
    let number = &result.kinds[&MutationKind::Number];
    assert!(number.either >= number.audit.max(number.readback));
    assert!(
        number.readback > 0,
        "a changed number of a Verified card no longer reads back"
    );
    assert!(result.render().lines().last().unwrap().starts_with("all\t"));
}
