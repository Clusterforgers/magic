//! Goal: the keyword lines of round 5 builder g (Reinforce, Forecast, Escalate) are written only for the ability the keyword means.
use super::*;
use crate::cost::Cost;

fn reinforce(cost: &str, counters: &str, more: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":{cost},"effects":[{{"PutCounters":{{{counters}"counter":"+1/+1","on":{{"Target":0}}}}}}],{more}"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}]}}}}"#
    )
}

const HAND: &str = r#""from":"Hand","#;

#[rstest]
#[case::two(
    reinforce(r#"[{"Mana":"{1}{G}"},"DiscardSelf"]"#, r#""count":2,"#, HAND),
    "Reinforce 2\u{2014}{1}{G}",
    true
)]
#[case::one(
    reinforce(r#"[{"Mana":"{W}"},"DiscardSelf"]"#, "", HAND),
    "Reinforce 1\u{2014}{W}",
    true
)]
#[case::x(
    reinforce(r#"[{"Mana":"{X}{W}{W}"},"DiscardSelf"]"#, r#""count":"X","#, HAND),
    "Reinforce X\u{2014}{X}{W}{W}",
    true
)]
#[case::other_count(
    reinforce(r#"[{"Mana":"{1}{G}"},"DiscardSelf"]"#, r#""count":2,"#, HAND),
    "Reinforce 1\u{2014}{1}{G}",
    false
)]
#[case::other_mana(
    reinforce(r#"[{"Mana":"{1}{G}"},"DiscardSelf"]"#, r#""count":2,"#, HAND),
    "Reinforce 2\u{2014}{2}{G}",
    false
)]
#[case::from_the_battlefield(
    reinforce(r#"[{"Mana":"{1}{G}"},"DiscardSelf"]"#, r#""count":2,"#, ""),
    "Reinforce 2\u{2014}{1}{G}",
    false
)]
#[case::exiled_not_discarded(
    reinforce(r#"[{"Mana":"{1}{G}"},"ExileSelf"]"#, r#""count":2,"#, HAND),
    "Reinforce 2\u{2014}{1}{G}",
    false
)]
#[case::at_sorcery_speed(
    reinforce(
        r#"[{"Mana":"{1}{G}"},"DiscardSelf"]"#,
        r#""count":2,"#,
        r#""from":"Hand","timing":"Sorcery","#
    ),
    "Reinforce 2\u{2014}{1}{G}",
    false
)]
#[case::other_counter(reinforce(r#"[{"Mana":"{1}{G}"},"DiscardSelf"]"#, r#""count":2,"#, HAND).replace("+1/+1", "-1/-1"), "Reinforce 2\u{2014}{1}{G}", false)]
#[case::only_your_creatures(reinforce(r#"[{"Mana":"{1}{G}"},"DiscardSelf"]"#, r#""count":2,"#, HAND).replace(r#"{"Type":"Creature"}"#, r#"{"Type":"Creature"},{"ControlledBy":"You"}"#), "Reinforce 2\u{2014}{1}{G}", false)]
fn a_reinforce_line_is_written_for_the_ability_it_means(
    #[case] json: String,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    let line = crate::compile::reinforce_line(&ability(&json));
    assert_eq!(
        line.is_some_and(|line| same_text(&line, clause)),
        reads,
        "{json}"
    );
}

#[rstest]
#[case::any_number("A deck can have any number of cards named ~.", true)]
#[case::up_to_seven("A deck can have up to seven cards named ~.", false)]
#[case::another_name("A deck can have any number of cards named Island.", false)]
fn the_deck_rule_reads_and_writes_back(#[case] text: &str, #[case] reads: bool) {
    let read = clause(text, ClauseKind::Static, false);
    let rule = ability(r#"{"Rule":{"rule":"AnyNumberInDeck"}}"#);
    assert_eq!(read.as_ref() == Some(&rule), reads, "{read:?}");
    if reads {
        assert!(
            written_variants(&rule)
                .iter()
                .any(|written| same_text(written, text))
        );
    }
}

fn forecast(condition: &str, cost: &str, more: &str) -> String {
    format!(
        r#"{{"Activated":{{"condition":{condition},"cost":{cost},"effects":[{{"Continuous":{{"affected":{{"Target":0}},"duration":"UntilEndOfTurn","mods":[{{"ModifyPT":{{"power":1,"toughness":1}}}}]}}}}],{more}"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}]}}}}"#
    )
}

const UPKEEP: &str = r#"{"All":["YourTurn",{"Step":"Upkeep"}]}"#;
const REVEAL: &str = r#"[{"Mana":"{W}"},{"RevealFromHand":{"filter":["This"]}}]"#;
const ONCE_FROM_HAND: &str = r#""from":"Hand","limit":"OncePerTurn","#;
const STANCE: &str = "Forecast \u{2014} {W}, Reveal this card from your hand: Target creature gets +1/+1 until end of turn.";

#[rstest]
#[case::steeling_stance(forecast(UPKEEP, REVEAL, ONCE_FROM_HAND), STANCE, true)]
#[case::other_effect(forecast(UPKEEP, REVEAL, ONCE_FROM_HAND), &STANCE.replace("+1/+1", "+2/+2"), false)]
#[case::other_mana(forecast(UPKEEP, REVEAL, ONCE_FROM_HAND), &STANCE.replace("{W}", "{1}{W}"), false)]
#[case::without_the_limit(forecast(UPKEEP, REVEAL, r#""from":"Hand","#), STANCE, false)]
#[case::during_any_turn(forecast(r#""YourTurn""#, REVEAL, ONCE_FROM_HAND), STANCE, false)]
#[case::from_the_battlefield(forecast(UPKEEP, REVEAL, r#""limit":"OncePerTurn","#), STANCE, false)]
#[case::revealing_another_card(
    forecast(
        UPKEEP,
        r#"[{"Mana":"{W}"},{"RevealFromHand":{"filter":[{"Type":"Creature"}]}}]"#,
        ONCE_FROM_HAND
    ),
    STANCE,
    false
)]
#[case::the_reveal_alone(
    forecast(UPKEEP, r#"[{"RevealFromHand":{"filter":["This"]}}]"#, ONCE_FROM_HAND),
    "Forecast \u{2014} Reveal this card from your hand: Target creature gets +1/+1 until end of turn.",
    false
)]
fn a_forecast_line_is_written_for_the_ability_it_means(
    #[case] json: String,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    let lines = crate::compile::forecast_lines(&ability(&json), written_variants);
    assert_eq!(
        lines.iter().any(|line| same_text(line, clause)),
        reads,
        "{lines:?}"
    );
}

#[rstest]
#[case::mana(r#""{2}""#, "Escalate {2}", true)]
#[case::other_mana(r#""{2}""#, "Escalate {1}", false)]
#[case::a_discard(r#"[{"Discard":{}}]"#, "Escalate\u{2014}Discard a card.", true)]
#[case::two_discards(
    r#"[{"Discard":{"count":2}}]"#,
    "Escalate\u{2014}Discard a card.",
    false
)]
#[case::a_tapped_creature(
    r#"[{"TapPermanents":{"filter":[{"Type":"Creature"}]}}]"#,
    "Escalate\u{2014}Tap an untapped creature you control.",
    true
)]
#[case::another_keyword(r#""{2}""#, "Entwine {2}", false)]
fn an_escalate_line_reads_as_exactly_its_cost(
    #[case] cost: &str,
    #[case] line: &str,
    #[case] reads: bool,
) {
    let cost: Cost = serde_json::from_str(cost).unwrap();
    assert_eq!(crate::compile::escalate_reads(line, &cost), reads);
}

#[test]
fn escalate_belongs_to_a_spell_of_modes_alone() {
    let spell = ability(
        r#"{"Spell":{"effects":[{"Modal":{"choose":{"max":2,"min":1},"extra_mode_cost":"{1}","modes":[{"effects":[{"Draw":{}}]},{"effects":[{"GainLife":{"amount":2}}]}]}}]}}"#,
    );
    assert!(crate::compile::escalate_cost(&spell).is_some());
    let plain = ability(
        r#"{"Spell":{"effects":[{"Modal":{"choose":{"max":2,"min":1},"modes":[{"effects":[{"Draw":{}}]},{"effects":[{"GainLife":{"amount":2}}]}]}}]}}"#,
    );
    assert!(crate::compile::escalate_cost(&plain).is_none());
}
