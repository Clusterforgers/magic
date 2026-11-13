//! Goal: the r28-9 cards read back whole as their printed text and play Verified, and the same ability with one count, kind, player, duration or verb changed never reads back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;
use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::{PrintedCard, Support};
use rstest::rstest;

use common::{cards_dir, printed, token_sources};

fn names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

/// The verdict and state of `rules` checked against the corpus card `name`.
fn judged(name: &str, rules: &str) -> (Verdict, Option<Support>) {
    let cards = printed();
    let tokens = Library::from_sources(&token_sources()).unwrap();
    let card = cards.iter().find(|card| card.name == name).unwrap();
    let report = check(rules, card, &tokens, &names(&cards), &Caps::everything());
    (
        report.verdict(),
        report.support.map(|record| record.support),
    )
}

/// The corpus file of the card as compact JSON.
fn compact(name: &str) -> String {
    let text = std::fs::read_to_string(cards_dir().join(format!("{}.json", slug(name)))).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value.to_string()
}

#[rstest]
#[case::angel_skips_combat(
    "Blinding Angel",
    r#""action":"SkipCombat""#,
    r#""action":{"SkipStep":"Untap"}"#
)]
#[case::patriarch_white(
    "Revenant Patriarch",
    r#""ManaSpent":"White""#,
    r#""ManaSpent":"Black""#
)]
#[case::shisato_untap(
    "Shisato, Whispering Hunter",
    r#"{"SkipStep":"Untap"}"#,
    r#"{"SkipStep":"Draw"}"#
)]
#[case::firemaw_name(
    "Chandra's Firemaw",
    r#""Named":"Chandra, Flame's Catalyst""#,
    r#""Named":"Elspeth, Undaunted Hero""#
)]
#[case::devotee_graveyard("Elspeth's Devotee", r#""also":["Graveyard"],"#, "")]
#[case::stalwarts_reveal("Rowan's Stalwarts", r#","reveal":true"#, "")]
#[case::wavecaster_hand("Teferi's Wavecaster", r#""to":"Hand""#, r#""to":"Graveyard""#)]
#[case::resolve_anthem(
    "Gideon's Resolve",
    r#""power":1,"toughness":1"#,
    r#""power":1,"toughness":0"#
)]
#[case::war_axe_trample("Raven Clan War-Axe", r#""Trample""#, r#""Menace""#)]
#[case::visage_colors("Visage of Bolas", r#""Red""#, r#""Green""#)]
#[case::chandra_free_cast_duration(
    "Chandra, Flame's Catalyst",
    r#""duration":"UntilEndOfTurn","rule":{"MayCast""#,
    r#""duration":"UntilYourNextTurn","rule":{"MayCast""#
)]
#[case::chandra_seven(
    "Chandra, Flame's Catalyst",
    r#"{"Draw":{"count":7}}"#,
    r#"{"Draw":{"count":6}}"#
)]
#[case::elspeth_hoplite_battlefield(
    "Elspeth, Undaunted Hero",
    r#""to":{"Battlefield":{"under":"You"}}"#,
    r#""to":"Hand""#
)]
#[case::rowan_two_targets(
    "Rowan, Fearless Sparkmage",
    r#""max":2,"min":0"#,
    r#""max":3,"min":0"#
)]
#[case::rowan_cant_block("Rowan, Fearless Sparkmage", r#""CantBlock""#, r#""CantAttack""#)]
#[case::gideon_size(
    "Gideon, Martial Paragon",
    r#""power":5,"toughness":5"#,
    r#""power":4,"toughness":4"#
)]
#[case::gideon_prevent_to_him(
    "Gideon, Martial Paragon",
    r#"{"Damage":{"to":"This"}}"#,
    r#"{"Damage":{"source":["This"]}}"#
)]
#[case::eivor_equipment("Eivor, Battle-Ready", r#""Equipment""#, r#""Aura""#)]
#[case::transcendence_twenty("Transcendence", r#""right":20"#, r#""right":30"#)]
#[case::transcendence_twice("Transcendence", r#""by":2"#, r#""by":3"#)]
#[case::tomb_sacrifice("Lich's Tomb", r#"{"Sacrifice""#, r#"{"Exile""#)]
#[case::opal_size(
    "Opal Avenger",
    r#""power":3,"toughness":5"#,
    r#""power":5,"toughness":3"#
)]
#[case::opal_life("Opal Avenger", r#""op":"Le""#, r#""op":"Ge""#)]
#[case::acrobats_results("Atomwheel Acrobats", r#""results":[1,2]"#, r#""results":[1,3]"#)]
#[case::critical_natural("Critical Hit", r#""natural":true,"#, "")]
#[case::clerk_one("Complaints Clerk", r#""results":[1]"#, r#""results":[2]"#)]
#[case::dee_kay_six("Dee Kay, Finder of the Lost", r#""results":[6]"#, r#""results":[5]"#)]
#[case::dee_kay_may(
    "Dee Kay, Finder of the Lost",
    r#"{"May":{"effects":[{"TapOrUntap""#,
    r#"{"Pay":{"cost":"{1}","paid":[{"TapOrUntap""#
)]
#[case::hook_destroys(
    "Captain's Hook",
    r#"{"Destroy":{"what":"Triggering"}}"#,
    r#"{"Exile":{"what":"Triggering"}}"#
)]
#[case::exhumer_haunt("Exhumer Thrull", r#"{"HauntedBy":"This"}"#, r#""Other""#)]
#[case::belfry_two("Belfry Spirit", r#""count":2"#, r#""count":3"#)]
#[case::absolver_enchantment("Absolver Thrull", r#""Enchantment""#, r#""Artifact""#)]
#[case::blind_hunter_two(
    "Blind Hunter",
    r#"{"GainLife":{"amount":2}}"#,
    r#"{"GainLife":{"amount":3}}"#
)]
#[case::graven_other(
    "Graven Dominator",
    r#"{"Type":"Creature"},"Other"]"#,
    r#"{"Type":"Creature"}]"#
)]
#[case::pontiff_minus("Orzhov Pontiff", r#""power":-1"#, r#""power":-2"#)]
#[case::robot_third("Robot Domination", r#""number":3"#, r#""number":4"#)]
#[case::rewrite_fourth("Rewrite History", r#""number":4"#, r#""number":3"#)]
#[case::cube_seventh("Construct a Cosmic Cube", r#""number":7"#, r#""number":6"#)]
#[case::marshdrinker_swamp("Marshdrinker Giant", r#""Swamp""#, r#""Forest""#)]
#[case::seer_nonland(
    "Thought-Knot Seer",
    r#"{"Not":{"Type":"Land"}}"#,
    r#"{"Not":{"Type":"Creature"}}"#
)]
#[case::yore_tiller_attacking(
    "Yore-Tiller Nephilim",
    r#"{"JoinAttack":{"what":{"Target":0}}}"#,
    r#"{"Tap":{"what":{"Target":0}}}"#
)]
#[case::legion_name(
    "Legion Angel",
    r#""Named":"Legion Angel""#,
    r#""Named":"Serra Angel""#
)]
#[case::ogre_life("Singe-Mind Ogre", r#"{"LoseLife""#, r#"{"GainLife""#)]
#[case::toothy_draw(
    "Toothy, Imaginary Friend",
    r#"{"Drew":{"who":"You"}}"#,
    r#"{"Drew":{"who":"Opponents"}}"#
)]
#[case::kitnap_stun("Kitnap", r#""count":3"#, r#""count":2"#)]
#[case::motion_tapped("Motion Sickness", r#","Tapped"]"#, r"]")]
#[case::antelope_plains("Graceful Antelope", r#""Plains""#, r#""Island""#)]
#[case::amalia_twenty("Amalia Benavides Aguirre", r#""right":20"#, r#""right":10"#)]
#[case::veil_morph(
    "Master of the Veil",
    r#"{"HasKeywordNamed":"Morph"}"#,
    r#"{"HasKeywordNamed":"Disguise"}"#
)]
#[case::cerberus_hand("Underworld Cerberus", r#""to":"Hand""#, r#""to":"Exile""#)]
#[case::lammasu_three("Absolving Lammasu", r#""amount":3"#, r#""amount":2"#)]
#[case::monitor_four(
    "Surveillance Monitor",
    r#"{"CollectEvidence":4}"#,
    r#"{"CollectEvidence":3}"#
)]
#[case::overlord_tapped("Overlord of the Hauntwoods", r#""tapped":true,"#, "")]
#[case::bolas_three("Nicol Bolas, the Deceiver", r#""amount":3"#, r#""amount":2"#)]
fn r289_a_card_reads_back_and_a_near_miss_does_not(
    #[case] name: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let rules = compact(name);
    assert_eq!(
        judged(name, &rules),
        (Verdict::Accepted, Some(Support::Verified)),
        "{name}"
    );
    let changed = rules.replacen(from, to, 1);
    assert_ne!(changed, rules, "{name}: the mutation applies");
    assert_ne!(
        judged(name, &changed).0,
        Verdict::Accepted,
        "{name}: {to} must not read back"
    );
}

#[rstest]
#[case::adipose_emerge_sacrifice("Adipose Offspring", 1)]
#[case::aboleth_spawn_copies("Aboleth Spawn", 1)]
#[case::campsite_sacrifice_foods("Campsite Cuisine", 1)]
fn r289_a_partial_card_leaves_only_its_listed_clauses(#[case] name: &str, #[case] manual: usize) {
    let rules = compact(name);
    let (verdict, support) = judged(name, &rules);
    assert_eq!(
        (verdict, support),
        (Verdict::Accepted, Some(Support::Partial)),
        "{name}"
    );
    assert_eq!(rules.matches(r#""Manual""#).count(), manual, "{name}");
}
