//! Goal: rules the w17 triage held back match Oracle and read back as it: a printed "you may have it deal" asks, a
//! script's opponent widens to the player the text names, and the read-back accepts the Oracle wordings of the same
//! rules (a toll before the exile, "If you do" before a delayed return, teamwork, "it deals", one damage sentence,
//! "That player", an attached permanent put into a graveyard, "that creature deal", "a permanent other than ~") while a
//! rule of another meaning is not accepted.
use std::collections::BTreeSet;

use rstest::rstest;
use serde_json::{Value, json};

use mtg_cards::load::Library;
use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::{PrintedCard, PrintedFace};

use super::{abilities, gaps};

#[rstest]
#[case::have_that_creature_deal(
    "Whenever a creature you control attacks, you may have that creature deal 1 damage to target player.",
    "TriggeredAttacker",
    true
)]
#[case::have_it_deal(
    "Whenever a creature you control attacks, you may have it deal 1 damage to target player.",
    "TriggeredAttacker",
    true
)]
#[case::it_deals(
    "Whenever a creature you control attacks, it deals 1 damage to target player.",
    "TriggeredAttacker",
    false
)]
fn a_printed_have_deal_asks(#[case] printed: &str, #[case] source: &str, #[case] asks: bool) {
    let script = format!(
        "Name:Test Forger\nTypes:Creature Shaman\nPT:2/2\nT:Mode$ Attacks | ValidCard$ Creature.YouCtrl | TriggerZones$ Battlefield | Execute$ TrigDamage | TriggerDescription$ {printed}\nSVar:TrigDamage:DB$ DealDamage | ValidTgts$ Player | NumDmg$ 1 | DamageSource$ {source}\nOracle:{printed}\n"
    );
    let found = json!(abilities(&script)).to_string();
    assert!(found.contains("Triggered"), "{found} {:?}", gaps(&script));
    assert_eq!(found.contains("\"May\""), asks, "{found}");
}

#[rstest]
#[case::printed_player("a player", "Player.Opponent", "Opponents", false)]
#[case::printed_opponent("an opponent", "Player.Opponent", "Opponents", true)]
#[case::printed_player_scripted_player("a player", "Player", "Opponents", false)]
fn a_damaged_player_is_the_one_the_text_names(
    #[case] printed: &str,
    #[case] valid: &str,
    #[case] narrowed: &str,
    #[case] opponents: bool,
) {
    let text = format!("Whenever CARDNAME deals combat damage to {printed}, you may draw a card.");
    let script = format!(
        "Name:Test Viper\nTypes:Creature Snake\nPT:1/1\nT:Mode$ DamageDone | ValidSource$ Card.Self | ValidTarget$ {valid} | CombatDamage$ True | OptionalDecider$ You | TriggerZones$ Battlefield | Execute$ TrigDraw | TriggerDescription$ {text}\nSVar:TrigDraw:DB$ Draw\nOracle:{text}\n"
    );
    let found = json!(abilities(&script)).to_string();
    assert!(found.contains("DealsDamage"), "{found} {:?}", gaps(&script));
    assert_eq!(
        found.contains(&format!("\"to\":\"{narrowed}\"")),
        opponents,
        "{found}"
    );
}

#[rstest]
#[case::printed_player("Target player discards two cards.", "Player")]
#[case::printed_opponent("Target opponent discards two cards.", "Opponent")]
#[case::printed_player_and_opponent(
    "Target player discards two cards. Each opponent loses 1 life.",
    "Opponent"
)]
fn a_targeted_opponent_is_the_player_the_text_names(#[case] oracle: &str, #[case] kind: &str) {
    let script = format!(
        "Name:Test Sabotage\nTypes:Sorcery\nA:SP$ Discard | ValidTgts$ Opponent | Mode$ TgtChoose | NumCards$ 2 | SpellDescription$ Target opponent discards two cards.\nOracle:{oracle}\n"
    );
    let found = json!(abilities(&script)).to_string();
    assert!(
        found.contains(&format!("\"targets\":[{{\"what\":\"{kind}\"}}]")),
        "{found} {:?}",
        gaps(&script)
    );
}

fn printed(type_line: &str, oracle: &str) -> PrintedCard {
    PrintedCard {
        oracle_id: "00000000-0000-4000-8000-0000000f2900".into(),
        name: "Alpha".into(),
        keywords: Vec::new(),
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: "Alpha".into(),
            mana_cost: "{2}{R}".into(),
            type_line: type_line.into(),
            oracle_text: oracle.into(),
            ..PrintedFace::default()
        }],
    }
}

fn verdict(type_line: &str, oracle: &str, abilities: &Value) -> Verdict {
    let card = printed(type_line, oracle);
    let names = BTreeSet::from(["alpha".to_owned()]);
    let json = json!({"faces": [{"abilities": abilities}]}).to_string();
    check_exact(
        &json,
        &card,
        &Library::default(),
        &names,
        &mtg_engine::caps(),
    )
    .verdict()
}

const INSTANT: &str = "Instant";
const CREATURE: &str = "Creature \u{2014} Goblin";
const AURA: &str = "Enchantment \u{2014} Aura";
const LIES: &str = "Counter target spell unless its controller pays {3}. If that spell is countered this way, exile it instead of putting it into its owner's graveyard.";
const SLIDE: &str = "Whenever a player cycles a card, you may exile target creature. If you do, return that card to the battlefield under its owner's control at the beginning of the next end step.";
const BLAST: &str = "Teamwork 2 (As an additional cost to cast this spell, you may tap any number of creatures you control with total power 2 or more.)\nAlpha deals 5 damage to target creature. If this spell was cast using teamwork, it also deals 2 damage to that creature's controller.";
const TACTICS: &str = "Teamwork 1 (As an additional cost to cast this spell, you may tap any number of creatures you control with total power 1 or more.)\nTarget creature gains double strike until end of turn. If this spell was cast using teamwork, that creature also gains trample until end of turn.";
const CARNIVAL: &str = "Alpha deals 1 damage to target creature or planeswalker and 1 damage to that permanent's controller.";
const CARNAGE: &str = "Alpha deals 3 damage to target opponent. That player discards two cards.";
const GREMLIN: &str = "Enchant artifact\nWhen enchanted artifact is put into a graveyard, create a 2/2 red Gremlin creature token.";
const FORGER: &str = "Whenever a creature you control attacks, you may have that creature deal 1 damage to target player or planeswalker.";
const LAUGH: &str = "Whenever a permanent other than Alpha is put into a graveyard from the battlefield, Alpha deals 1 damage to each creature and each player.";

fn lies(cost: &str, both_in_toll: bool) -> Value {
    let shield = json!({"Shield": {"by": {"MoveTo": "Exile"}, "duration": "UntilEndOfTurn",
        "when": {"Countered": {"what": [{"Is": {"Target": 0}}, "Spell"]}}}});
    let counter = json!({"Counter": {"what": {"Target": 0}}});
    let who = json!({"ControllerOf": {"Target": 0}});
    let effects = if both_in_toll {
        json!([{"Pay": {"cost": cost, "unpaid": [shield, counter], "who": who}}])
    } else {
        json!([counter, {"Pay": {"cost": cost, "unpaid": [shield], "who": who}}])
    };
    json!([{"Spell": {"effects": effects, "targets": [{"what": {"Object": ["Spell"]}}]}}])
}

fn slide(return_in_choice: bool) -> Value {
    let exile = json!({"Exile": {"as": "exiled", "what": {"Target": 0}}});
    let back = json!({"DelayedTrigger": {"effects": [{"Move": {"to": {"Battlefield": {}}, "what": {"Bound": "exiled"}}}],
        "when": {"StepBegins": {"step": "End"}}}});
    let effects = if return_in_choice {
        json!([{"May": {"effects": [exile, back]}}])
    } else {
        json!([{"May": {"effects": [exile]}}, back])
    };
    json!([{"Triggered": {"effects": effects, "targets": [{"what": {"Object": [{"Type": "Creature"}]}}],
        "when": {"Cycled": {}}}}])
}

fn teamwork(amount: u32, effects: &Value) -> Value {
    json!([{"Keyword": {"Macro": {"amount": amount, "name": "teamwork"}}},
        {"Spell": {"effects": effects, "targets": [{"what": {"Object": [{"Type": "Creature"}]}}]}}])
}

fn blast(condition: &Value) -> Value {
    teamwork(
        2,
        &json!([{"DealDamage": {"amount": 5, "to": {"Target": 0}}},
        {"If": {"condition": condition, "then": [{"DealDamage": {"amount": 2, "to": {"ControllerOf": {"Target": 0}}}}]}}]),
    )
}

fn tactics(second: &str) -> Value {
    let gains = |keyword: &str| json!({"Continuous": {"affected": {"Target": 0}, "duration": "UntilEndOfTurn", "mods": [{"AddKeyword": keyword}]}});
    teamwork(
        1,
        &json!([gains("DoubleStrike"), {"If": {"condition": "AdditionalCostPaid", "then": [gains(second)]}}]),
    )
}

fn carnival(second: u32) -> Value {
    json!([{"Spell": {"effects": [
        {"DealDamage": {"amount": 1, "to": {"Target": 0}}},
        {"DealDamage": {"amount": second, "to": {"ControllerOf": {"Target": 0}}}}
    ], "targets": [{"what": {"Object": [{"AnyOf": [{"Type": "Creature"}, {"Type": "Planeswalker"}]}]}}]}}])
}

fn carnage(discarder: &Value) -> Value {
    json!([{"Spell": {"effects": [
        {"DealDamage": {"amount": 3, "to": {"Target": 0}}},
        {"Discard": {"count": 2, "who": discarder}}
    ], "targets": [{"what": "Opponent"}]}}])
}

fn gremlin(to: &str) -> Value {
    json!([{"Keyword": {"Enchant": [{"Type": "Artifact"}]}},
        {"Triggered": {"effects": [{"CreateToken": {"token": {"Custom": {"colors": ["Red"], "name": "Gremlin",
            "power": 2, "subtypes": ["Gremlin"], "toughness": 2, "types": ["Creature"]}}}}],
        "when": {"ZoneChange": {"from": "Battlefield", "to": to, "what": [{"Is": "Attached"}]}}}}])
}

fn forger(asks: bool) -> Value {
    let deal = json!({"DealDamage": {"amount": 1, "source": "Triggering", "to": {"Target": 0}}});
    let effects = if asks {
        json!([{"May": {"effects": [deal]}}])
    } else {
        json!([deal])
    };
    json!([{"Triggered": {"effects": effects,
        "targets": [{"what": {"Either": ["Player", {"Object": [{"Type": "Planeswalker"}]}]}}],
        "when": {"Attacks": {"what": [{"Type": "Creature"}, {"ControlledBy": "You"}]}}}}])
}

fn laugh(what: &Value) -> Value {
    json!([{"Triggered": {"effects": [
        {"DealDamage": {"amount": 1, "to": {"Each": [{"Type": "Creature"}]}}},
        {"DealDamage": {"amount": 1, "to": "EachPlayer"}}
    ], "when": {"ZoneChange": {"from": "Battlefield", "to": "Graveyard", "what": what}}}}])
}

#[rstest]
#[case::lies(INSTANT, LIES, lies("{3}", true), true)]
#[case::lies_four(INSTANT, LIES, lies("{4}", true), false)]
#[case::lies_counter_outside(INSTANT, LIES, lies("{3}", false), false)]
#[case::slide("Enchantment", SLIDE, slide(true), true)]
#[case::slide_return_outside("Enchantment", SLIDE, slide(false), false)]
#[case::blast("Sorcery", BLAST, blast(&json!("AdditionalCostPaid")), true)]
#[case::blast_kicked("Sorcery", BLAST, blast(&json!("Kicked")), false)]
#[case::tactics(INSTANT, TACTICS, tactics("Trample"), true)]
#[case::tactics_flying(INSTANT, TACTICS, tactics("Flying"), false)]
#[case::carnival("Sorcery", CARNIVAL, carnival(1), true)]
#[case::carnival_two("Sorcery", CARNIVAL, carnival(2), false)]
#[case::carnage("Sorcery", CARNAGE, carnage(&json!({"Target": 0})), true)]
#[case::carnage_you("Sorcery", CARNAGE, carnage(&json!("You")), false)]
#[case::gremlin(AURA, GREMLIN, gremlin("Graveyard"), true)]
#[case::gremlin_exile(AURA, GREMLIN, gremlin("Exile"), false)]
#[case::forger(CREATURE, FORGER, forger(true), true)]
#[case::forger_forced(CREATURE, FORGER, forger(false), false)]
#[case::laugh("Enchantment", LAUGH, laugh(&json!(["Other"])), true)]
#[case::laugh_any("Enchantment", LAUGH, laugh(&json!([])), false)]
fn the_oracle_wordings_read_back(
    #[case] type_line: &str,
    #[case] oracle: &str,
    #[case] abilities: Value,
    #[case] accepted: bool,
) {
    let found = verdict(type_line, oracle, &abilities);
    assert_eq!(found == Verdict::Accepted, accepted, "{found:?} {oracle}");
}
