//! Goal: an attack trigger that counts the attackers of a player's planeswalkers with theirs ("attack one of your opponents or a planeswalker they control") writes back as Frontier Warmonger's text, and the same trigger without them, or about another player, does not.
use super::*;

fn r5502_warmonger(walkers: bool, at: &str) -> Ability {
    ability(&format!(
        r#"{{"Triggered":{{"when":{{"GroupAttacksAt":{{"what":[{{"Type":"Creature"}}],"at":"{at}","walkers":{walkers}}}}},"effects":[{{"Continuous":{{"affected":"Triggering","mods":[{{"AddKeyword":"Menace"}}],"duration":"UntilEndOfTurn"}}}}]}}}}"#
    ))
}

const WARMONGER: &str = "Whenever one or more creatures attack one of your opponents or a planeswalker they control, those creatures gain menace until end of turn.";

#[rstest]
#[case::the_card(true, "Opponents", true)]
#[case::without_planeswalkers(false, "Opponents", false)]
#[case::another_player(true, "You", false)]
fn r5502_the_warmonger_trigger_writes_back(
    #[case] walkers: bool,
    #[case] at: &str,
    #[case] reads: bool,
) {
    let written = written_variants(&r5502_warmonger(walkers, at));
    assert_eq!(
        written.iter().any(|text| same_text(text, WARMONGER)),
        reads,
        "{written:?}"
    );
}
