//! Goal: the facts of round 10 pack g10c write as the card prints them: "the damage dealt this way", "the damage
//! already dealt to that player this turn", "you've committed a crime this turn" (also "~ can't be blocked if") and
//! "+1/+1 counters put this turn"; another amount, player, kind of counter or comparison does not read back.
use super::*;

fn spell_of(effects: &str, target: &str) -> String {
    format!(r#"{{"Spell":{{"effects":{effects},"targets":[{{"what":"{target}"}}]}}}}"#)
}

fn drain(gain: &str) -> String {
    spell_of(
        &format!(
            r#"[{{"DealDamage":{{"amount":3,"to":{{"Target":0}}}}}},{{"GainLife":{{"amount":{gain}}}}}]"#
        ),
        "AnyTarget",
    )
}

fn punish(who: &str) -> String {
    spell_of(
        &format!(
            r#"[{{"LoseLife":{{"amount":{{"Tallied":{{"what":"DamageTaken","who":{who}}}}},"who":{{"Target":0}}}}}}]"#
        ),
        "Player",
    )
}

const DRAIN: &str =
    "~ deals 3 damage to any target. You gain life equal to the damage dealt this way.";
const PUNISH: &str =
    "Target player loses life equal to the damage already dealt to that player this turn.";

#[rstest]
#[case::dealt_this_way(drain(r#""DamageDealtThisWay""#), DRAIN, true)]
#[case::life_lost_is_not_damage(drain(r#""LifeLostThisWay""#), DRAIN, false)]
#[case::that_players_damage(punish(r#"{"Target":0}"#), PUNISH, true)]
#[case::your_damage_is_not_theirs(punish(r#""You""#), PUNISH, false)]
fn g10c_spell_amounts_write_and_read_as_printed(
    #[case] json: String,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability(&json));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
    let read = clause(text, ClauseKind::Spell, false);
    assert_eq!(read.as_ref() == Some(&ability(&json)), reads, "{read:?}");
}

fn compare(left: &str, number: u32) -> String {
    format!(r#"{{"Compare":{{"left":{left},"op":"Ge","right":{number}}}}}"#)
}

fn crimes(who: &str) -> String {
    format!(r#"{{"Tallied":{{"what":"Crimes","who":"{who}"}}}}"#)
}

fn plus_one(event: &str, who: &str, filter: &str) -> String {
    format!(r#"{{"Tally":{{"event":"{event}","filter":{filter},"who":"{who}"}}}}"#)
}

fn static_of(condition: &str, mods: &str) -> String {
    format!(r#"{{"Static":{{"affected":"This","condition":{condition},"mods":{mods}}}}}"#)
}

fn end_step(condition: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"PutCounters":{{"counter":"+1/+1","on":"This"}}}}],"if":{condition},"when":{{"StepBegins":{{"step":"End"}}}}}}}}"#
    )
}

const PUMP: &str = r#"[{"ModifyPT":{"power":2,"toughness":0}}]"#;
const UNBLOCKED: &str = r#"["CantBeBlocked"]"#;
const TRAMPLE: &str = r#"[{"AddKeyword":"Trample"}]"#;
const CRIME_PUMP: &str = "As long as you've committed a crime this turn, ~ gets +2/+0.";
const CRIME_UNBLOCKED: &str = "~ can't be blocked if you've committed a crime this turn.";
const PUT_TRAMPLE: &str =
    "As long as you've put one or more +1/+1 counters on a creature this turn, ~ has trample.";
const PUT_YOURS: &str = "At the beginning of each end step, if a +1/+1 counter was put on a permanent under your control this turn, put a +1/+1 counter on ~.";
const CREATURE: &str = r#"[{"Type":"Creature"}]"#;
const YOURS: &str = r#"[{"ControlledBy":"You"}]"#;

#[rstest]
#[case::crime(static_of(&compare(&crimes("You"), 1), PUMP), CRIME_PUMP, true)]
#[case::an_opponents_crime(static_of(&compare(&crimes("Opponents"), 1), PUMP), CRIME_PUMP, false)]
#[case::two_crimes(static_of(&compare(&crimes("You"), 2), PUMP), CRIME_PUMP, false)]
#[case::unblocked_if_crime(static_of(&compare(&crimes("You"), 1), UNBLOCKED), CRIME_UNBLOCKED, true)]
#[case::unblocked_if_their_crime(static_of(&compare(&crimes("Opponents"), 1), UNBLOCKED), CRIME_UNBLOCKED, false)]
#[case::counters_put(static_of(&compare(&plus_one("PlusOneCountersPut", "You", CREATURE), 1), TRAMPLE), PUT_TRAMPLE, true)]
#[case::any_counter_is_not_plus_one(static_of(&compare(&plus_one("CountersPut", "You", CREATURE), 1), TRAMPLE), PUT_TRAMPLE, false)]
#[case::put_by_anyone_is_not_you(static_of(&compare(&plus_one("PlusOneCountersPut", "EachPlayer", CREATURE), 1), TRAMPLE), PUT_TRAMPLE, false)]
#[case::put_on_yours_by_anyone(end_step(&compare(&plus_one("PlusOneCountersPut", "EachPlayer", YOURS), 1)), PUT_YOURS, true)]
#[case::put_on_yours_by_you(end_step(&compare(&plus_one("PlusOneCountersPut", "You", YOURS), 1)), PUT_YOURS, false)]
#[case::put_on_yours_twice(end_step(&compare(&plus_one("PlusOneCountersPut", "EachPlayer", YOURS), 2)), PUT_YOURS, false)]
fn g10c_turn_facts_write_as_printed(#[case] json: String, #[case] text: &str, #[case] reads: bool) {
    let found = written_variants(&ability(&json));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

fn dies(condition: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"Draw":{{"count":3}}}}],"if":{condition},"when":{{"ZoneChange":{{"from":"Battlefield","to":"Graveyard","what":["This"]}}}}}}}}"#
    )
}

fn dealt_to(who: &str) -> String {
    format!(r#"{{"Tallied":{{"what":"DamageDealtTo","who":{who}}}}}"#)
}

const ZUBERA: &str =
    "When ~ dies, if 4 or more damage was dealt to it this turn, draw three cards.";

#[rstest]
#[case::four_dealt_to_it(dies(&compare(&dealt_to(r#""This""#), 4)), true)]
#[case::three_is_not_four(dies(&compare(&dealt_to(r#""This""#), 3)), false)]
#[case::damage_you_took_is_not_its(dies(&compare(&dealt_to(r#""You""#), 4)), false)]
#[case::its_counters_are_not_damage(dies(&compare(r#"{"Tallied":{"what":"CountersPut","who":"This"}}"#, 4)), false)]
fn g10c_damage_dealt_to_it_writes_as_printed(#[case] json: String, #[case] reads: bool) {
    let found = written_variants(&ability(&json));
    assert_eq!(
        found.iter().any(|w| same_text(w, ZUBERA)),
        reads,
        "{found:?}"
    );
}

fn graved_end(what: &str, who: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"Draw":{{}}}}],"if":{},"when":{{"StepBegins":{{"step":"End"}}}}}}}}"#,
        compare(
            &format!(r#"{{"Tallied":{{"what":"{what}","who":"{who}"}}}}"#),
            1
        )
    )
}

const GRAVED: &str = "At the beginning of each end step, if a creature card was put into your graveyard from anywhere this turn, draw a card.";

#[rstest]
#[case::creature_card_to_yours(graved_end("CreatureCardsGraved", "You"), true)]
#[case::any_card_is_not_a_creature_card(graved_end("CardsGraved", "You"), false)]
#[case::an_opponents_graveyard(graved_end("CreatureCardsGraved", "Opponents"), false)]
#[case::died_is_not_from_anywhere(graved_end("CreaturesDied", "You"), false)]
fn g10c_creature_card_graved_writes_as_printed(#[case] json: String, #[case] reads: bool) {
    let found = written_variants(&ability(&json));
    assert_eq!(
        found.iter().any(|w| same_text(w, GRAVED)),
        reads,
        "{found:?}"
    );
}
