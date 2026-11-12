//! Goal: every card pack r30-7b made automatic reads back as its printed text clause by clause, and each new wording
//! is written only for its exact shape (near misses keep other words).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{ClauseState, Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn report(name: &str) -> mtg_cards::proposal::Report {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    let text = std::fs::read_to_string(&path).unwrap();
    check(&text, card, &tokens, &names(&cards), &Caps::everything())
}

/// Prints, for the cards named in `R307B_PROBE` (separated by `|`), each clause state and every wording of each
/// ability; a tool for writing rules files.
#[test]
fn probe() {
    let Ok(list) = std::env::var("R307B_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = report(name);
        println!("== {name}: {:?}", report.verdict());
        for problem in &report.problems {
            println!("  problem: {problem:?}");
        }
        for clause in &report.clauses {
            println!("  [{}] {} => {:?}", clause.face, clause.text, clause.state);
        }
        let path = common::cards_dir().join(format!("{}.json", slug(name)));
        let Ok(card) = mtg_cards::load::parse_json::<mtg_cards::card::Card>(
            "probe",
            &std::fs::read_to_string(&path).unwrap(),
        ) else {
            continue;
        };
        for face in &card.faces {
            for ability in &face.abilities {
                let mut seen = BTreeSet::new();
                for text in mtg_cards::compile::written_variants(ability) {
                    if seen.insert(text.clone()) {
                        println!("    > {text:?}");
                    }
                }
            }
        }
    }
}

/// Prints every wording of each ability of the JSON list in the file `R307B_ABILITIES`; a tool for writing writers.
#[test]
fn wordings() {
    let Ok(file) = std::env::var("R307B_ABILITIES") else {
        return;
    };
    let abilities: Vec<mtg_cards::ability::Ability> =
        mtg_cards::load::parse_json("abilities", &std::fs::read_to_string(file).unwrap()).unwrap();
    for ability in &abilities {
        println!("--");
        let mut seen = BTreeSet::new();
        for text in mtg_cards::compile::written_variants(ability) {
            if seen.insert(text.clone()) {
                println!("    > {text:?}");
            }
        }
    }
}

/// The cards of the pack that play whole.
const WHOLE: [&str; 8] = [
    "Archnemesis",
    "Combustion Man",
    "Crag Saurian",
    "Heartwood Storyteller",
    "Perrie, the Pulverizer",
    "Rufus Shinra",
    "Silver Surfer, Galactus's Herald",
    "Ultra Magnus, Tactician // Ultra Magnus, Armored Carrier",
];

#[test]
fn every_whole_card_reads_back_clause_by_clause() {
    for name in WHOLE {
        let report = report(name);
        assert_eq!(report.verdict(), Verdict::Accepted, "{name}: {report:?}");
        for clause in &report.clauses {
            assert!(
                matches!(clause.state, ClauseState::Reads { .. }),
                "{name}: {} => {:?}",
                clause.text,
                clause.state
            );
        }
    }
}

fn wordings_of(json: &str) -> Vec<String> {
    let ability: Ability = mtg_cards::load::parse_json("ability", json).unwrap();
    mtg_cards::compile::written_variants(&ability)
}

const OPPONENTS_DRAW: &str = r#"{"RepeatEach":{"over":{"PlayersWhere":{"who":"EachPlayer","test":{"Not":{"Among":"Triggering"}}}},"as":"player","effects":[{"May":{"who":{"Bound":"player"},"effects":[{"Draw":{"who":{"Bound":"player"}}}]}}]}}"#;
const GAINS_CONTROL: &str = r#"{"Continuous":{"affected":"This","mods":[{"SetController":{"ControllerOf":"Dealer"}}],"duration":"Forever"}}"#;
const KINDS: &str = r#"{"Distinct":{"of":[{"ControlledBy":"You"}],"stat":"CounterKinds"}}"#;

/// Each wording appears for its exact shape and disappears when one part changes.
#[rstest]
#[case::opponents_of_the_caster(
    format!(r#"{{"Triggered":{{"when":{{"SpellCast":{{"what":[{{"Not":{{"Type":"Creature"}}}}]}}}},"effects":[{OPPONENTS_DRAW}]}}}}"#),
    "each of that player's opponents may draw a card",
    true
)]
#[case::opponents_of_your_own_cast(
    format!(r#"{{"Triggered":{{"when":{{"SpellCast":{{"by":"You"}}}},"effects":[{OPPONENTS_DRAW}]}}}}"#),
    "each of that player's opponents",
    false
)]
#[case::source_controller_gains_control(
    format!(r#"{{"Triggered":{{"when":{{"DealsDamage":{{"source":[],"to":"This"}}}},"effects":[{GAINS_CONTROL}]}}}}"#),
    "that source's controller gains control of ~",
    true
)]
#[case::only_combat_damage_is_another_head(
    format!(r#"{{"Triggered":{{"when":{{"DealsDamage":{{"source":[],"to":"This","combat":true}}}},"effects":[{GAINS_CONTROL}]}}}}"#),
    "that source's controller gains control",
    false
)]
#[case::control_until_end_of_turn(
    r#"{"Triggered":{"when":{"DealsDamage":{"source":[],"to":"This"}},"effects":[{"Continuous":{"affected":"This","mods":[{"SetController":{"ControllerOf":"Dealer"}}],"duration":"UntilEndOfTurn"}}]}}"#.to_owned(),
    "that source's controller gains control",
    false
)]
#[case::damage_equal_to_power(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[]}}],"effects":[{"Pay":{"who":{"ControllerOf":{"Target":0}},"cost":[{"DamageToYou":{"Stat":{"of":"This","stat":"Power"}}}],"unpaid":[{"Destroy":{"what":{"Target":0}}}]}}]}}"#.to_owned(),
    "deal damage to them equal to his power",
    true
)]
#[case::damage_equal_to_toughness(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[]}}],"effects":[{"Pay":{"who":{"ControllerOf":{"Target":0}},"cost":[{"DamageToYou":{"Stat":{"of":"This","stat":"Toughness"}}}],"unpaid":[{"Destroy":{"what":{"Target":0}}}]}}]}}"#.to_owned(),
    "equal to his power",
    false
)]
#[case::the_damage_paid_by_you(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[]}}],"effects":[{"Pay":{"cost":[{"DamageToYou":{"Stat":{"of":"This","stat":"Power"}}}],"unpaid":[{"Destroy":{"what":{"Target":0}}}]}}]}}"#.to_owned(),
    "unless its controller has",
    false
)]
#[case::different_kinds(
    format!(r#"{{"Triggered":{{"when":{{"Attacks":{{"what":["This"]}}}},"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}}}}],"effects":[{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"AddKeyword":"Trample"}},{{"ModifyPT":{{"power":{KINDS},"toughness":{KINDS}}}}}],"duration":"UntilEndOfTurn"}}}}]}}}}"#),
    "where X is the number of different kinds of counters among permanents you control",
    true
)]
#[case::card_types_are_no_kinds_of_counters(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":{"CardTypes":[{"ControlledBy":"You"}]},"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#.to_owned(),
    "different kinds",
    false
)]
#[case::attach_to_the_attacker(
    r#"{"Triggered":{"when":{"PlayerAttacks":{"at":"You"}},"effects":[{"May":{"effects":[{"Attach":{"what":"This","to":"Triggering"}}]}}]}}"#.to_owned(),
    "Whenever a player attacks you, you may attach this Aura to that player.",
    true
)]
#[case::attach_when_an_opponent_attacks_anyone(
    r#"{"Triggered":{"when":{"PlayerAttacks":{"at":"Opponents"}},"effects":[{"May":{"effects":[{"Attach":{"what":"This","to":"Triggering"}}]}}]}}"#.to_owned(),
    "attach this Aura to that player",
    false
)]
#[case::attach_by_force(
    r#"{"Triggered":{"when":{"PlayerAttacks":{"at":"You"}},"effects":[{"Attach":{"what":"This","to":"Triggering"}}]}}"#.to_owned(),
    "attach this Aura to that player",
    false
)]
#[case::drain_the_enchanted_player(
    r#"{"Triggered":{"when":{"PlayerAttacks":{"by":"You","at":"AttachedPlayer"}},"effects":[{"LoseLife":{"who":"AttachedPlayer","amount":2}},{"Draw":{}},{"GainLife":{"amount":2}}]}}"#.to_owned(),
    "Whenever you attack enchanted player, that player loses 2 life. You draw a card and gain 2 life.",
    true
)]
#[case::drain_each_opponent(
    r#"{"Triggered":{"when":{"PlayerAttacks":{"by":"You","at":"AttachedPlayer"}},"effects":[{"LoseLife":{"who":"Opponents","amount":2}},{"Draw":{}},{"GainLife":{"amount":2}}]}}"#.to_owned(),
    "that player loses 2 life",
    false
)]
#[case::attacks_that_player(
    r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Rule":{"rule":{"MustAttackPlayers":{"who":{"Target":0},"players":"Triggering"}},"duration":"UntilEndOfYourNextTurn"}}]}}"#.to_owned(),
    "until the end of your next turn, target creature attacks that player each combat if able",
    true
)]
#[case::attacks_that_player_this_turn(
    r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Rule":{"rule":{"MustAttackPlayers":{"who":{"Target":0},"players":"Triggering"}},"duration":"UntilEndOfTurn"}}]}}"#.to_owned(),
    "until the end of your next turn",
    false
)]
#[case::put_attacking_then_convert(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"},{"Type":"Artifact"},{"Type":"Creature"}]},"as":"c"}},{"Move":{"what":{"Bound":"c"},"to":{"Battlefield":{"tapped":true}}}},{"JoinAttack":{"what":{"Bound":"c"}}}]}},{"If":{"condition":"Performed","then":[{"DelayedTrigger":{"when":{"StepBegins":{"step":"EndCombat"}},"effects":[{"Transform":{"what":"This"}}]}}]}}]}}"#.to_owned(),
    "you may put an artifact creature card from your hand onto the battlefield tapped and attacking. If you do, convert ~ at end of combat.",
    true
)]
#[case::put_untapped(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"},{"Type":"Artifact"},{"Type":"Creature"}]},"as":"c"}},{"Move":{"what":{"Bound":"c"},"to":{"Battlefield":{}}}},{"JoinAttack":{"what":{"Bound":"c"}}}]}},{"If":{"condition":"Performed","then":[{"DelayedTrigger":{"when":{"StepBegins":{"step":"EndCombat"}},"effects":[{"Transform":{"what":"This"}}]}}]}}]}}"#.to_owned(),
    "convert ~ at end of combat",
    false
)]
#[case::put_from_the_graveyard(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Artifact"},{"Type":"Creature"}]},"as":"c"}},{"Move":{"what":{"Bound":"c"},"to":{"Battlefield":{"tapped":true}}}},{"JoinAttack":{"what":{"Bound":"c"}}}]}},{"If":{"condition":"Performed","then":[{"DelayedTrigger":{"when":{"StepBegins":{"step":"EndCombat"}},"effects":[{"Transform":{"what":"This"}}]}}]}}]}}"#.to_owned(),
    "convert ~ at end of combat",
    false
)]
fn a_wording_is_written_for_its_shape_alone(
    #[case] json: String,
    #[case] words: &str,
    #[case] written: bool,
) {
    let texts = wordings_of(&json);
    assert_eq!(
        texts.iter().any(|text| text.contains(words)),
        written,
        "{words:?} in {texts:#?}"
    );
}
