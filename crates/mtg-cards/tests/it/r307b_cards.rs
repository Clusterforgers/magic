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
const WHOLE: [&str; 18] = [
    "Archfiend of Spite",
    "Archnemesis",
    "Combustion Man",
    "Crag Saurian",
    "Deathmist Raptor",
    "Elbrus, the Binding Blade // Withengar Unbound",
    "Frodo, Adventurous Hobbit",
    "Heartwood Storyteller",
    "Perrie, the Pulverizer",
    "Raphael, Tag Team Tough",
    "Rufus Shinra",
    "Sam, Loyal Attendant",
    "Silver Surfer, Galactus's Herald",
    "Typhoid Mary, Fractured",
    "Ultima, Origin of Oblivion",
    "Ultra Magnus, Tactician // Ultra Magnus, Armored Carrier",
    "Void Mirror",
    "Yuna, Grand Summoner",
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
const YUNA: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["Other",{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"if":{"Compare":{"left":{"AnyCounters":"Triggering"},"op":"Ge","right":1}},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"May":{"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":{"AnyCounters":"Triggering"}}}]}}]}}"#;
const SPITE: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"ControlledBy":"Opponents"}],"to":"This"}},"effects":[{"Pay":{"who":{"ControllerOf":"Dealer"},"cost":[{"Sacrifice":{"filter":[],"count":"Triggering"}}],"unpaid":[{"LoseLife":{"who":{"ControllerOf":"Dealer"},"amount":"Triggering"}}]}}]}}"#;
const RAPHAEL: &str = r#"{"Triggered":{"when":{"Nth":{"event":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"nth":1}},"effects":[{"Untap":{"what":{"Each":[{"Type":"Creature"},"Attacking"]}}},{"AddPhase":{"phases":["Combat"]}}]}}"#;
const MARY: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Modal":{"at_random":true,"chosen_if":{"Compare":{"left":{"Tallied":{"who":"You","what":"Discarded"}},"op":"Ge","right":1}},"modes":[{"effects":[{"Draw":{}}]},{"effects":[{"GainLife":{"amount":2}}]}]}}]}}"#;
const MIRROR: &str = r#"{"Triggered":{"when":{"SpellCast":{}},"if":{"Compare":{"left":{"Stat":{"of":"Triggering","stat":"ColoredManaSpent"}},"op":"Lt","right":1}},"effects":[{"Counter":{"what":"Triggering"}}]}}"#;
const FRODO: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"RingTempts":{}},{"If":{"condition":{"All":[{"Matches":{"subject":"This","selector":["RingBearer"]}},{"Compare":{"left":{"RingTempted":"You"},"op":"Ge","right":2}}]},"then":[{"Draw":{}}]}}]}}"#;
const FOODS: &str = r#"{"CostModifier":{"applies_to":[{"Subtype":"Food"},{"ControlledBy":"You"}],"change":"Reduce","mana":"{1}","activating":"Every"}}"#;
const BLIGHT: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":{"Other":"blight"}}},{"Continuous":{"affected":{"Target":0},"mods":[{"SetLandTypes":[]},"LoseAbilities",{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}}],"duration":{"WhileHasCounter":{"Other":"blight"}}}}]}}"#;
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
#[case::that_number_of_counters(
    YUNA.to_owned(),
    "is put into a graveyard from the battlefield, if it had one or more counters on it, you may put that number of +1/+1 counters on target creature.",
    true
)]
#[case::that_number_of_one_kind(
    YUNA.replace(r#"{"AnyCounters":"Triggering"}}}"#, r#"{"Counters":{"on":"Triggering","counter":"+1/+1"}}}}"#),
    "that number of +1/+1 counters",
    false
)]
#[case::life_unless_sacrifice(
    SPITE.to_owned(),
    "Whenever a source an opponent controls deals damage to ~, that source's controller loses that much life unless they sacrifice that many permanents of their choice.",
    true
)]
#[case::life_unless_sacrifice_one(
    SPITE.replace(r#""count":"Triggering""#, r#""count":1"#),
    "unless they sacrifice that many permanents",
    false
)]
#[case::any_source(
    SPITE.replace(r#"[{"ControlledBy":"Opponents"}]"#, "[]"),
    "that source's controller loses that much life",
    false
)]
#[case::after_this_combat_phase(
    RAPHAEL.to_owned(),
    "untap all attacking creatures. After this combat phase, there is an additional combat phase.",
    true
)]
#[case::after_this_combat_phase_outside_combat(
    RAPHAEL.replace(r#""combat":true"#, r#""combat":false"#),
    "After this combat phase",
    false
)]
#[case::chosen_instead(
    MARY.to_owned(),
    "choose one at random. If you discarded a card this turn, you choose one instead.",
    true
)]
#[case::chosen_instead_of_two(
    MARY.replace(r#""left":{"Tallied":{"who":"You","what":"Discarded"}},"op":"Ge","right":1"#, r#""left":{"Tallied":{"who":"You","what":"Discarded"}},"op":"Ge","right":2"#),
    "If you discarded a card this turn",
    false
)]
#[case::picked_at_random_alone(
    MARY.replace(r#""chosen_if":{"Compare":{"left":{"Tallied":{"who":"You","what":"Discarded"}},"op":"Ge","right":1}},"#, ""),
    "instead",
    false
)]
#[case::no_colored_mana(MIRROR.to_owned(), "if no colored mana was spent to cast it, counter that spell", true)]
#[case::no_mana_at_all(MIRROR.replace("ColoredManaSpent", "ManaSpent"), "no colored mana", false)]
#[case::some_colored_mana(MIRROR.replace(r#""op":"Lt""#, r#""op":"Ge""#), "no colored mana", false)]
#[case::ring_tempted_twice(FRODO.to_owned(), "Then if ~ is your Ring-bearer and the Ring has tempted you two or more times this game, draw a card.", true)]
#[case::ring_tempted_five_times(FRODO.replace(r#""right":2"#, r#""right":5"#), "or more times this game", false)]
#[case::ring_tempted_without_bearer(FRODO.replace(r#"{"Matches":{"subject":"This","selector":["RingBearer"]}}"#, r#"{"Matches":{"subject":"This","selector":["Tapped"]}}"#), "your Ring-bearer", false)]
#[case::activated_abilities_of_foods(FOODS.to_owned(), "Activated abilities of Foods you control cost {1} less to activate.", true)]
#[case::equip_abilities_of_foods(FOODS.replace("Every", "Equip"), "Activated abilities of Foods", false)]
#[case::blighted_land(BLIGHT.to_owned(), "put a blight counter on target land. For as long as that land has a blight counter on it, it loses all land types and abilities and has", true)]
#[case::blighted_land_for_good(BLIGHT.replace(r#""duration":{"WhileHasCounter":{"Other":"blight"}}"#, r#""duration":"Forever""#), "For as long as that land has", false)]
#[case::blighted_land_keeps_abilities(BLIGHT.replace(r#""LoseAbilities","#, ""), "loses all land types and abilities", false)]
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

/// A counted cost in an effect runs when its count reads the same for any payer: a stat of an object as damage and the
/// event's amount as a sacrifice; a count of the payer's own permanents stays out.
#[rstest]
#[case::damage_equal_to_power(r#"[{"DamageToYou":{"Stat":{"of":"This","stat":"Power"}}}]"#, true)]
#[case::sacrifice_that_many(r#"[{"Sacrifice":{"filter":[],"count":"Triggering"}}]"#, true)]
#[case::sacrifice_as_many_as_you_control(
    r#"[{"Sacrifice":{"filter":[],"count":{"Count":[{"ControlledBy":"You"}]}}}]"#,
    false
)]
fn a_counted_cost_another_player_pays_is_counted_in_the_effect(
    #[case] cost: &str,
    #[case] counted: bool,
) {
    let json = format!(
        r#"{{"Triggered":{{"when":{{"DealsDamage":{{"source":[],"to":"This"}}}},"effects":[{{"Pay":{{"who":{{"ControllerOf":"Dealer"}},"cost":{cost},"unpaid":[{{"LoseLife":{{"who":{{"ControllerOf":"Dealer"}},"amount":"Triggering"}}}}]}}}}]}}}}"#
    );
    let ability: Ability = mtg_cards::load::parse_json("ability", &json).unwrap();
    let used = mtg_cards::validate::used_primitives(&ability);
    assert_eq!(
        used.iter().any(|p| p == "Effect.Pay.Counted"),
        counted,
        "{used:?}"
    );
}

/// The first time each turn the source deals combat damage to a player is counted; damage of any kind, or by another
/// source, is not.
#[rstest]
#[case::combat_damage_by_this(r#"{"source":["This"],"to":"EachPlayer","combat":true}"#, true)]
#[case::any_damage_by_this(r#"{"source":["This"],"to":"EachPlayer"}"#, false)]
#[case::combat_damage_by_a_creature(
    r#"{"source":[{"Type":"Creature"}],"to":"EachPlayer","combat":true}"#,
    false
)]
#[case::combat_damage_to_you(r#"{"source":["This"],"to":"You","combat":true}"#, false)]
fn the_first_combat_damage_to_a_player_is_counted(#[case] event: &str, #[case] counted: bool) {
    let json = format!(
        r#"{{"Triggered":{{"when":{{"Nth":{{"event":{{"DealsDamage":{event}}},"nth":1}}}},"effects":[{{"Draw":{{}}}}]}}}}"#
    );
    let card = format!(
        r#"{{"oracle_id":"00000000-0000-4000-8000-000000000001","name":"Probe","impl_status":"Full","faces":[{{"name":"Probe","oracle_fingerprint":"","abilities":[{json}]}}]}}"#
    );
    let problems = Library::from_sources(&[mtg_cards::load::Source {
        path: "cards/probe.json".to_owned(),
        text: card,
    }])
    .err()
    .map(|e| format!("{e:?}"))
    .unwrap_or_default();
    assert_eq!(
        !problems.contains("counted in a turn"),
        counted,
        "{problems}"
    );
}

/// A choice instead of a pick at random needs the pick at random.
#[test]
fn a_choice_instead_needs_a_pick_at_random() {
    let json = MARY.replace(r#""at_random":true,"#, "");
    let card = format!(
        r#"{{"oracle_id":"00000000-0000-4000-8000-000000000002","name":"Probe","impl_status":"Full","faces":[{{"name":"Probe","oracle_fingerprint":"","abilities":[{json}]}}]}}"#
    );
    let problems = Library::from_sources(&[mtg_cards::load::Source {
        path: "cards/probe.json".to_owned(),
        text: card,
    }])
    .err()
    .map(|e| format!("{e:?}"))
    .unwrap_or_default();
    assert!(problems.contains("needs a pick at random"), "{problems}");
}
