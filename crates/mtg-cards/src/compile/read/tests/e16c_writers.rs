//! Goal: the writers of round 16 pack e16c give the oracle wording of a declaration of attackers at one player ("Whenever you attack a player", "Whenever one or more creatures attack you", "Whenever an opponent attacks you"), of a token "tapped and attacking that player" and of an attack at the player with the most life, and another player attacked, attacker, count, defender or change never reads back.
use super::super::tokens::same_written_text;
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_written_text(wording, clause))
}

const YOURS: &str = r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#;
const ANY: &str = r#"[{"Type":"Creature"}]"#;
const THEIRS: &str = r#"[{"Type":"Creature"},{"ControlledBy":"Opponents"}]"#;
const WARRIORS: &str = r#"[{"Subtype":"Warrior"},{"ControlledBy":"You"}]"#;
const EQUIPPED: &str =
    r#"[{"Type":"Creature"},{"ControlledBy":"You"},{"HasAttached":[{"Subtype":"Equipment"}]}]"#;
const DRAW: &str = r#"{"Draw":{"count":1}}"#;
const GLIMMER: &str = r#"{"CreateToken":{"token":{"Custom":{"name":"Glimmer","colors":["White"],"types":["Enchantment","Creature"],"subtypes":["Glimmer"],"power":1,"toughness":1}},"tapped":true,"attacking":true,"against":"Against"}}"#;
const WEAKER: &str = r#"{"Continuous":{"affected":"Triggering","duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":-1,"toughness":0}}]}}"#;

/// A trigger on attackers at one player.
fn at(what: &str, player: &str, at_least: u32, effect: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"GroupAttacksAt":{{"what":{what},"at":"{player}","at_least":{at_least}}}}},"effects":[{effect}]}}}}"#
    )
}

const LIGHTBRINGER: &str = "Whenever you attack a player, create a 1/1 white Glimmer enchantment creature token that's tapped and attacking that player.";

#[rstest]
#[case::you_attack_a_player(
    at(YOURS, "EachPlayer", 1, DRAW),
    "Whenever you attack a player, draw a card."
)]
#[case::that_player(at(YOURS, "EachPlayer", 1, GLIMMER), LIGHTBRINGER)]
#[case::creatures_attack_you(
    at(ANY, "You", 1, DRAW),
    "Whenever one or more creatures attack you, draw a card."
)]
#[case::a_player_attacks_you(
    at(ANY, "You", 1, DRAW),
    "Whenever a player attacks you, draw a card."
)]
#[case::an_opponent_attacks_you(
    at(THEIRS, "You", 1, DRAW),
    "Whenever an opponent attacks you, draw a card."
)]
#[case::one_of_your_opponents(
    at(ANY, "Opponents", 1, DRAW),
    "Whenever one or more creatures attack one of your opponents, draw a card."
)]
#[case::warriors(
    at(WARRIORS, "EachPlayer", 1, DRAW),
    "Whenever one or more Warriors you control attack a player, draw a card."
)]
#[case::yours_at_an_opponent(
    at(WARRIORS, "Opponents", 1, DRAW),
    "Whenever one or more Warriors you control attack a player, draw a card."
)]
#[case::two_or_more(
    at(YOURS, "EachPlayer", 2, DRAW),
    "Whenever two or more creatures you control attack a player, draw a card."
)]
#[case::with_equipped(
    at(EQUIPPED, "EachPlayer", 1, DRAW),
    "Whenever you attack a player with one or more equipped creatures, draw a card."
)]
#[case::those_creatures(
    at(ANY, "You", 1, WEAKER),
    "Whenever one or more creatures attack you, those creatures get -1/-0 until end of turn."
)]
fn e16c_an_attack_at_one_player_reads_back(#[case] json: String, #[case] clause: &str) {
    assert!(
        written(&json, clause),
        "{:?}",
        written_variants(&ability(&json))
    );
}

#[rstest]
#[case::another_player(
    at(YOURS, "Opponents", 1, DRAW),
    "Whenever you attack a player, draw a card."
)]
#[case::any_attacker(
    at(ANY, "EachPlayer", 1, DRAW),
    "Whenever you attack a player, draw a card."
)]
#[case::not_you(
    at(ANY, "Opponents", 1, DRAW),
    "Whenever one or more creatures attack you, draw a card."
)]
#[case::at_you(
    at(ANY, "You", 1, DRAW),
    "Whenever a player attacks one of your opponents, draw a card."
)]
#[case::count(
    at(YOURS, "EachPlayer", 3, DRAW),
    "Whenever two or more creatures you control attack a player, draw a card."
)]
#[case::not_yours(
    at(THEIRS, "You", 2, DRAW),
    "Whenever an opponent attacks you, draw a card."
)]
#[case::no_defender(at(YOURS, "EachPlayer", 1, &GLIMMER.replace(r#","against":"Against""#, "")), LIGHTBRINGER)]
#[case::not_attacking(at(YOURS, "EachPlayer", 1, &GLIMMER.replace(r#""attacking":true,"#, "")), LIGHTBRINGER)]
#[case::other_defender(at(YOURS, "EachPlayer", 1, &GLIMMER.replace(r#""against":"Against""#, r#""against":"You""#)), LIGHTBRINGER)]
#[case::those_for_good(at(ANY, "You", 1, &WEAKER.replace("UntilEndOfTurn", "Forever")), "Whenever one or more creatures attack you, those creatures get -1/-0 until end of turn.")]
#[case::those_are_others(at(ANY, "You", 1, &WEAKER.replace("Triggering", "This")), "Whenever one or more creatures attack you, those creatures get -1/-0 until end of turn.")]
fn e16c_another_attack_shape_differs(#[case] json: String, #[case] clause: &str) {
    assert!(
        !written(&json, clause),
        "{:?}",
        written_variants(&ability(&json))
    );
}

const GREATSWORD: &str = "Whenever equipped creature attacks the player with the most life or tied for most life, create a 4/4 white Angel creature token with flying that's tapped and attacking that player.";
const ANGEL: &str = r#"{"CreateToken":{"token":{"Custom":{"name":"Angel","colors":["White"],"types":["Creature"],"subtypes":["Angel"],"power":4,"toughness":4,"abilities":[{"Keyword":"Flying"}]}},"tapped":true,"attacking":true,"against":"Defending"}}"#;

#[rstest]
#[case::the_most_life("MostLife", ANGEL.to_owned(), true)]
#[case::another_player("Opponents", ANGEL.to_owned(), false)]
#[case::chosen_defender("MostLife", ANGEL.replace(r#","against":"Defending""#, ""), false)]
fn e16c_an_attack_at_the_most_life_reads_back(
    #[case] player: &str,
    #[case] effect: String,
    #[case] same: bool,
) {
    let json = format!(
        r#"{{"Triggered":{{"when":{{"AttacksPlayer":{{"what":[{{"Is":"Attached"}}],"at":"{player}"}}}},"effects":[{effect}]}}}}"#
    );
    assert_eq!(
        written(&json, GREATSWORD),
        same,
        "{:?}",
        written_variants(&ability(&json))
    );
}
