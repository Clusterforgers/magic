//! Goal: a target only a kicked spell chooses writes back as "If this spell was kicked, ... target ...", and "For each of
//! those permanents, its controller creates <token>" writes back after the sentences that name the targets; a target
//! also named outside the kicked sentence, a bound of another shape, a token for someone else, or a loop over
//! something else reads `Differs`.
use super::*;

/// Whether the ability writes the clause back, among its wordings.
fn writes(json: &str, clause: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, clause))
}

const KICKED: &str = r#"{"When":{"condition":"Kicked","otherwise":0,"then":1}}"#;

/// Probe as the translator writes it, with `bounds` on its target and `rest` after the draw.
fn probe(bounds: &str, rest: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Draw":{{"count":3}}}},{rest}],"targets":[{{"bounds":{bounds},"what":"Player"}}]}}}}"#
    )
}

const PROBE_TEXT: &str = "Draw three cards, then discard two cards. If this spell was kicked, target player discards two cards.";
const DISCARD_IF_KICKED: &str = r#"{"Discard":{"count":2}},{"If":{"condition":"Kicked","then":[{"Discard":{"count":2,"who":{"Target":0}}}]}}"#;

/// Unstable Footing as the translator writes it.
fn footing(bounds: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Rule":{{"duration":"UntilEndOfTurn","rule":{{"DamageCantBePrevented":{{}}}}}}}},{{"If":{{"condition":"Kicked","then":[{{"DealDamage":{{"amount":5,"to":{{"Target":0}},"unpreventable":true}}}}]}}}}],"targets":[{{"bounds":{bounds},"what":{{"Either":["Player",{{"Object":[{{"Type":"Planeswalker"}}]}}]}}}}]}}}}"#
    )
}

const FOOTING_TEXT: &str = "Damage can't be prevented this turn. If this spell was kicked, it deals 5 damage to target player or planeswalker.";

#[test]
fn fix16_kicked_only_targets_write_back() {
    let bounds = format!(r#"{{"min":{KICKED},"max":{KICKED}}}"#);
    assert!(writes(&probe(&bounds, DISCARD_IF_KICKED), PROBE_TEXT));
    assert!(writes(&footing(&bounds), FOOTING_TEXT));
}

#[rstest]
#[case::always_one_is_not_kicked_only(&format!(r#"{{"min":1,"max":{KICKED}}}"#), DISCARD_IF_KICKED)]
#[case::unkicked_target(
    r#"{"min":{"When":{"condition":"Kicked","otherwise":1,"then":0}},"max":{"When":{"condition":"Kicked","otherwise":1,"then":0}}}"#,
    DISCARD_IF_KICKED
)]
#[case::named_outside_the_kicked_sentence(
    &format!(r#"{{"min":{KICKED},"max":{KICKED}}}"#),
    r#"{"Discard":{"count":2,"who":{"Target":0}}},{"If":{"condition":"Kicked","then":[{"Discard":{"count":2,"who":{"Target":0}}}]}}"#
)]
#[case::other_condition(
    &format!(r#"{{"min":{KICKED},"max":{KICKED}}}"#),
    r#"{"Discard":{"count":2}},{"If":{"condition":"AdditionalCostPaid","then":[{"Discard":{"count":2,"who":{"Target":0}}}]}}"#
)]
#[case::with_an_otherwise(
    &format!(r#"{{"min":{KICKED},"max":{KICKED}}}"#),
    r#"{"Discard":{"count":2}},{"If":{"condition":"Kicked","then":[{"Discard":{"count":2,"who":{"Target":0}}}],"otherwise":[{"Draw":{"count":1}}]}}"#
)]
fn fix16_a_wrong_kicked_target_differs(#[case] bounds: &str, #[case] rest: &str) {
    assert!(!writes(&probe(bounds, rest), PROBE_TEXT), "{bounds} {rest}");
}

const ANGEL: &str = r#"{"Custom":{"abilities":[{"Keyword":"Flying"}],"colors":["White"],"name":"Angel","power":3,"subtypes":["Angel"],"toughness":3,"types":["Creature"]}}"#;

/// Soul of Emancipation as the translator writes it: `who` makes the token, `over` is what the loop runs over.
fn soul(who: &str, over: &str, min: u32) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"Destroy":{{"what":{{"Target":0}}}}}},{{"Together":{{"effects":[{{"RepeatEach":{{"as":"each","effects":[{{"CreateToken":{{"token":{ANGEL},"who":{who}}}}}],"over":{over}}}}}]}}}}],"targets":[{{"bounds":{{"max":3,"min":{min}}},"what":{{"Object":[{{"Not":{{"Type":"Land"}}}},"Other"]}}}}],"when":{{"ZoneChange":{{"to":"Battlefield","what":["This"]}}}}}}}}"#
    )
}

const SOUL_TEXT: &str = "When ~ enters, destroy up to three other target nonland permanents. For each of those permanents, its controller creates a 3/3 white Angel creature token with flying.";
const ITS_CONTROLLER: &str = r#"{"ControllerOf":{"Bound":"each"}}"#;

#[test]
fn fix16_for_each_of_those_targets_writes_back() {
    assert!(writes(
        &soul(ITS_CONTROLLER, r#"{"Target":0}"#, 0),
        SOUL_TEXT
    ));
}

#[rstest]
#[case::you_create(r#""You""#, r#"{"Target":0}"#, 0)]
#[case::owner_creates(r#"{"OwnerOf":{"Bound":"each"}}"#, r#"{"Target":0}"#, 0)]
#[case::over_each_creature(ITS_CONTROLLER, r#"{"Each":[{"Type":"Creature"}]}"#, 0)]
#[case::exactly_three_targets(ITS_CONTROLLER, r#"{"Target":0}"#, 3)]
fn fix16_a_wrong_token_loop_differs(#[case] who: &str, #[case] over: &str, #[case] min: u32) {
    assert!(
        !writes(&soul(who, over, min), SOUL_TEXT),
        "{who} {over} {min}"
    );
}
