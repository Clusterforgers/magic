//! Goal: the r37-9 wordings read and write as printed (a block either way this turn by any creature or by a kind, the
//! players finishing a vote and who voted with whom, the first spell of the game cast free, the greatest mana value among
//! the turn's instant and sorcery spells), and a rule of another meaning (another tally, another side of the vote, a block
//! one way only) never reads back as the clause.
use rstest::rstest;

use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const OPPONENTS_SAME: &str =
    r#"{"PlayersWhere":{"who":"Opponents","test":{"VotedAs":{"with":"Asker","same":true}}}}"#;
const OPPONENTS_APART: &str =
    r#"{"PlayersWhere":{"who":"Opponents","test":{"VotedAs":{"with":"Asker","same":false}}}}"#;

fn on_vote(effects: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"Did":{{"action":"FinishedVoting"}}}},"effects":[{effects}]}}}}"#
    )
}

#[rstest]
#[case::any_block("Destroy target creature that blocked or was blocked this turn.", r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"BlockedThisTurn":[]}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#)]
#[case::by_a_zombie("Exile target creature that blocked or was blocked by a Zombie this turn.", r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"BlockedThisTurn":[{"Subtype":"Zombie"}]}]}}],"effects":[{"Exile":{"what":{"Target":0}}}]}}"#)]
#[case::by_a_legend("Destroy target creature that blocked or was blocked by a legendary creature this turn.", r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"BlockedThisTurn":[{"Type":"Creature"},{"Supertype":"Legendary"}]}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#)]
fn r379_a_block_either_way_reads_as_printed(#[case] text: &str, #[case] json: &str) {
    assert_eq!(spell(text), Some(ability(json)), "{text}");
    assert!(written(json, text), "{text}");
}

#[rstest]
#[case::near_miss_only_blocked("Destroy target creature that blocked this turn.")]
#[case::near_miss_by_two(
    "Exile target creature that blocked or was blocked by two Zombies this turn."
)]
fn r379_a_block_of_another_shape_is_not_read(#[case] text: &str) {
    let read = spell(text);
    assert!(
        read.as_ref()
            .is_none_or(|ability| !format!("{ability:?}").contains("BlockedThisTurn")),
        "{text}: {read:?}"
    );
}

#[rstest]
#[case::grudge(
    &on_vote(&format!(r#"{{"LoseLife":{{"who":{OPPONENTS_APART},"amount":2}}}}"#)),
    "Whenever players finish voting, each opponent who voted for a choice you didn't vote for loses 2 life.",
    true
)]
#[case::grudge_near_miss_the_other_side(
    &on_vote(&format!(r#"{{"LoseLife":{{"who":{OPPONENTS_SAME},"amount":2}}}}"#)),
    "Whenever players finish voting, each opponent who voted for a choice you didn't vote for loses 2 life.",
    false
)]
#[case::erestor(
    &on_vote(&format!(r#"{{"CreateToken":{{"who":{OPPONENTS_SAME},"token":{{"Named":"treasure"}}}}}},{{"Scry":{{"count":{{"Players":{OPPONENTS_APART}}}}}}},{{"Draw":{{}}}}"#)),
    "Whenever players finish voting, each opponent who voted for a choice you voted for creates a Treasure token. You scry X, where X is the number of opponents who voted for a choice you didn't vote for. Draw a card.",
    true
)]
#[case::model_of_unity(
    &on_vote(&format!(r#"{{"RepeatEach":{{"over":{{"Union":["You",{OPPONENTS_SAME}]}},"as":"scrier","effects":[{{"May":{{"who":{{"Bound":"scrier"}},"effects":[{{"Scry":{{"who":{{"Bound":"scrier"}},"count":2}}}}]}}}}]}}}}"#)),
    "Whenever players finish voting, you and each opponent who voted for a choice you voted for may scry 2.",
    true
)]
#[case::first_spell(
    r#"{"FreeCast":{"condition":{"Compare":{"left":{"Tallied":{"who":"You","what":"SpellsCastThisGame"}},"op":"Eq","right":0}}}}"#,
    "If this spell is the first spell you've cast this game, you may cast it without paying its mana cost.",
    true
)]
#[case::first_spell_near_miss_this_turn(
    r#"{"FreeCast":{"condition":{"Compare":{"left":{"Tallied":{"who":"You","what":"SpellsCast"}},"op":"Eq","right":0}}}}"#,
    "If this spell is the first spell you've cast this game, you may cast it without paying its mana cost.",
    false
)]
#[case::rootha(
    r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"if":{"Compare":{"left":{"Tallied":{"who":"You","what":"InstantsAndSorceriesCast"}},"op":"Ge","right":1}},"effects":[{"CreateToken":{"token":{"Sized":{"size":{"GreatestCast":{"who":"You","of":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}],"stat":"ManaValue"}},"token":{"name":"Elemental","colors":["Blue","Red"],"types":["Creature"],"subtypes":["Elemental"],"abilities":[{"Keyword":"Flying"},{"Keyword":"Haste"}]}}}}}]}}"#,
    "At the beginning of combat on your turn, if you've cast an instant or sorcery spell this turn, create an X/X blue and red Elemental creature token with flying and haste, where X is the greatest mana value among instant and sorcery spells you've cast this turn.",
    true
)]
#[case::rootha_near_miss_any_spell(
    r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"if":{"Compare":{"left":{"Tallied":{"who":"You","what":"InstantsAndSorceriesCast"}},"op":"Ge","right":1}},"effects":[{"CreateToken":{"token":{"Sized":{"size":{"GreatestCast":{"who":"You","of":[],"stat":"ManaValue"}},"token":{"name":"Elemental","colors":["Blue","Red"],"types":["Creature"],"subtypes":["Elemental"],"abilities":[{"Keyword":"Flying"},{"Keyword":"Haste"}]}}}}}]}}"#,
    "At the beginning of combat on your turn, if you've cast an instant or sorcery spell this turn, create an X/X blue and red Elemental creature token with flying and haste, where X is the greatest mana value among instant and sorcery spells you've cast this turn.",
    false
)]
fn r379_a_rule_writes_as_printed(#[case] json: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(
        written(json, clause),
        reads,
        "{clause}: {:?}",
        written_variants(&ability(json))
    );
}

/// The abilities of a warp line: the `AltCast` and its riders.
fn r379_warp(cost: &str) -> Vec<Ability> {
    let mut all = vec![ability(&format!(
        r#"{{"AltCast":{{"way":"Warp","cost":{cost}}}}}"#
    ))];
    all.extend(crate::alt_riders::warp_riders());
    all
}

#[rstest]
#[case::with_life(
    r#"[{"Mana":"{B}"},{"PayLife":2}]"#,
    Some("Warp\u{2014}{B}, Pay 2 life")
)]
#[case::mana_alone(r#""{1}{W}""#, Some("Warp {1}{W}"))]
#[case::near_miss_no_mana(r#"[{"PayLife":2}]"#, None)]
fn r379_a_warp_line_names_its_other_parts(#[case] cost: &str, #[case] line: Option<&str>) {
    let found = crate::alt_riders::keyword_line(&r379_warp(cost)).map(|(line, _)| line);
    assert_eq!(found.as_deref(), line);
}

#[test]
fn r379_a_warp_from_the_graveyard_is_written_only_with_its_own_cost() {
    let culler =
        r#"{"AltCast":{"way":"Warp","cost":[{"Mana":"{B}"},{"PayLife":2}],"from":"Graveyard"}}"#;
    let other = r#"{"AltCast":{"way":"Warp","cost":"{B}","from":"Graveyard"}}"#;
    let text = "You may cast this card from your graveyard using its warp ability.";
    assert!(written(culler, text));
    assert!(!written(other, text));
}
