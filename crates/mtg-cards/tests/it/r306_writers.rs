//! Goal: the r30-6 writers word "whenever you forage", an X held to a cap ("X can't be greater than ...") and the steps
//! added after this one as the cards print them, and refuse the near misses: another player, another action, another
//! amount for the cap, no cap, another comparison of hands, another step, a step added with no "first" test.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::compile::written_variants;
use rstest::rstest;

fn says(json: &str, text: &str) -> bool {
    let ability: Ability = serde_json::from_str(json).unwrap();
    written_variants(&ability).iter().any(|said| said == text)
}

const FORAGE: &str = "Whenever you forage, put a +1/+1 counter on ~.";
const SHANNA: &str = "At the beginning of your end step, you may pay {X}. If you do, draw X cards. X can't be greater than the amount of life you gained this turn.";
const SOKENZAN: &str = "At the beginning of your upkeep, if a player has more cards in hand than each other player, the player who has the most cards in hand gains control of this creature.";

fn foraged(who: &str, action: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"Did":{{"action":"{action}","who":"{who}"}}}},"effects":[{{"PutCounters":{{"on":"This","counter":"+1/+1"}}}}]}}}}"#
    )
}

fn shanna(cap: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"End","whose":"You"}}}},"effects":[{{"Pay":{{"cost":[{{"Mana":"{{X}}"}}{cap}],"paid":[{{"Draw":{{"count":"X"}}}}]}}}}]}}}}"#
    )
}

const GAINED: &str = r#",{"XAtMost":{"Tallied":{"who":"You","what":"LifeGainedTotal"}}}"#;

fn sokenzan(op: &str) -> String {
    let most = format!(
        r#"{{"PlayersWhere":{{"who":"EachPlayer","test":{{"Compare":{{"left":{{"Count":[{{"In":"Hand"}},{{"OwnedBy":"You"}}]}},"op":"{op}","right":{{"Largest":{{"who":"EachPlayer","of":[{{"In":"Hand"}},{{"OwnedBy":"You"}}]}}}}}}}}}}}}"#
    );
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep","whose":"You"}}}},"if":{{"Compare":{{"left":{{"Players":{most}}},"op":"Eq","right":1}}}},"effects":[{{"Continuous":{{"affected":"This","mods":[{{"SetController":{most}}}],"duration":"Forever"}}}}]}}}}"#
    )
}

const HAZE: &str = "At the beginning of enchanted player's first upkeep each turn, that player gets an additional upkeep step after this step.";
const YSHTOLA: &str = "At the beginning of your end step, exile target creature you control, then return it to the battlefield under its owner's control. Then if it's the first end step of the turn, there is an additional end step after this step.";

fn haze(condition: &str, step: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep","whose":"AttachedPlayer"}}}}{condition},"effects":[{{"AddStep":{{"step":"{step}"}}}}]}}}}"#
    )
}

fn yshtola(then: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"End","whose":"You"}}}},"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}}}}],"effects":[{{"Exile":{{"what":{{"Target":0}},"as":"exiled"}}}},{{"Move":{{"what":{{"Bound":"exiled"}},"to":{{"Battlefield":{{}}}}}}}},{then}]}}}}"#
    )
}

#[rstest]
#[case::haze(haze(r#","if":"FirstStep""#, "Upkeep"), HAZE, true)]
#[case::haze_every_upkeep(haze("", "Upkeep"), HAZE, false)]
#[case::haze_another_step(haze(r#","if":"FirstStep""#, "Draw"), HAZE, false)]
#[case::yshtola(
    yshtola(r#"{"If":{"condition":"FirstStep","then":[{"AddStep":{"step":"End"}}]}}"#),
    YSHTOLA,
    true
)]
#[case::yshtola_always(yshtola(r#"{"AddStep":{"step":"End"}}"#), YSHTOLA, false)]
#[case::yshtola_an_upkeep(
    yshtola(r#"{"If":{"condition":"FirstStep","then":[{"AddStep":{"step":"Upkeep"}}]}}"#),
    YSHTOLA,
    false
)]
fn r306_added_steps_read_as_printed(#[case] json: String, #[case] text: &str, #[case] reads: bool) {
    assert_eq!(says(&json, text), reads, "{json}");
}

#[rstest]
#[case::you_forage(foraged("You", "Foraged"), FORAGE, true)]
#[case::an_opponent_forages(foraged("Opponents", "Foraged"), FORAGE, false)]
#[case::opponent_words(
    foraged("Opponents", "Foraged"),
    "Whenever an opponent forages, put a +1/+1 counter on ~.",
    true
)]
#[case::any_player(foraged("EachPlayer", "Foraged"), FORAGE, false)]
#[case::evidence_instead(foraged("You", "CollectedEvidence"), FORAGE, false)]
#[case::shanna(shanna(GAINED), SHANNA, true)]
#[case::gains_counted_not_added(
    shanna(r#",{"XAtMost":{"Tallied":{"who":"You","what":"LifeGained"}}}"#),
    SHANNA,
    false
)]
#[case::life_lost_cap(
    shanna(r#",{"XAtMost":{"Tallied":{"who":"You","what":"LifeLost"}}}"#),
    SHANNA,
    false
)]
#[case::no_cap(shanna(""), SHANNA, false)]
#[case::sokenzan(sokenzan("Eq"), SOKENZAN, true)]
#[case::sokenzan_at_least(sokenzan("Ge"), SOKENZAN, false)]
fn r306_writers_word_the_cards_and_refuse_near_misses(
    #[case] json: String,
    #[case] text: &str,
    #[case] reads: bool,
) {
    assert_eq!(says(&json, text), reads, "{json}");
}
