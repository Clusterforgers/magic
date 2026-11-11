//! Goal: the writers of ep27 write the card's words and nothing near them. "You may cast this spell as though it had flash if C." and the forms with the condition first ("As long as C, ...", "If C, ...") read as a `MayCast` rule of the card from its hand and write back; a permission from another zone, without flash, for other cards or without its condition does not. "If you would lose the game, instead ..." writes its steps as a spell would; another amount, fewer steps or another player does not. "You don't lose the game for having 0 or less life", hands played revealed, and damage lessened, capped or halved read and write back; every loss, other players, another source, recipient, amount or prevention does not.
use super::*;

const FAERIE: &str = r#"{"Compare":{"left":{"Count":[{"Subtype":"Faerie"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}"#;

fn self_flash(condition: &str, from: &str, permission: &str) -> Ability {
    ability(&format!(
        r#"{{"Rule":{{"rule":{{"MayCast":{permission}}},"condition":{condition},"from":"{from}"}}}}"#
    ))
}

const FLASH: &str = r#"{"what":["This"],"flash":true}"#;
const SPINNERS: &str = "You may cast ~ as though it had flash if you control a Faerie.";

#[rstest]
#[case::spinners(SPINNERS)]
#[case::condition_first("As long as you control a Faerie, you may cast ~ as though it had flash.")]
#[case::if_first("If you control a Faerie, you may cast ~ as though it had flash.")]
fn ep27_self_flash_reads_and_writes(#[case] text: &str) {
    let read = golden(text, ClauseKind::Static, false);
    assert_eq!(read, self_flash(FAERIE, "Hand", FLASH), "{text}");
}

#[rstest]
#[case::spinners(self_flash(FAERIE, "Hand", FLASH), SPINNERS, true)]
#[case::from_the_graveyard(self_flash(FAERIE, "Graveyard", FLASH), SPINNERS, false)]
#[case::without_flash(self_flash(FAERIE, "Hand", r#"{"what":["This"]}"#), SPINNERS, false)]
#[case::other_cards(
    self_flash(FAERIE, "Hand", r#"{"what":[{"Type":"Creature"}],"flash":true}"#),
    SPINNERS,
    false
)]
#[case::free(
    self_flash(FAERIE, "Hand", r#"{"what":["This"],"flash":true,"free":true}"#),
    SPINNERS,
    false
)]
#[case::another_condition(
    self_flash(&FAERIE.replace("Faerie", "Human"), "Hand", FLASH),
    SPINNERS,
    false
)]
#[case::without_a_condition(
    ability(r#"{"Rule":{"rule":{"MayCast":{"what":["This"],"flash":true}},"from":"Hand"}}"#),
    "You may cast ~ as though it had flash.",
    false
)]
fn ep27_self_flash_near_misses_differ(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

#[rstest]
#[case::players("Players play with their hands revealed.", "EachPlayer")]
#[case::opponents("Your opponents play with their hands revealed.", "Opponents")]
#[case::you("Play with your hand revealed.", "You")]
fn ep27_hands_revealed_read_and_write(#[case] text: &str, #[case] who: &str) {
    let read = golden(text, ClauseKind::Static, false);
    assert_eq!(
        read,
        ability(&format!(
            r#"{{"Rule":{{"rule":{{"RevealHand":"{who}"}}}}}}"#
        ))
    );
}

#[rstest]
#[case::players("EachPlayer", "Players play with their hands revealed.", true)]
#[case::opponents_for_players("Opponents", "Players play with their hands revealed.", false)]
#[case::you_for_opponents("You", "Your opponents play with their hands revealed.", false)]
#[case::the_active_player("ActivePlayer", "Players play with their hands revealed.", false)]
fn ep27_hands_revealed_near_misses_differ(
    #[case] who: &str,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let rule = format!(r#"{{"Rule":{{"rule":{{"RevealHand":"{who}"}}}}}}"#);
    let found = written_variants(&ability(&rule));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

#[test]
fn ep27_the_top_card_revealed_is_no_revealed_hand() {
    let top = ability(r#"{"Rule":{"rule":{"RevealTop":"You"}}}"#);
    let found = written_variants(&top);
    assert!(
        !found
            .iter()
            .any(|w| same_text(w, "Play with your hand revealed."))
    );
}

fn lessened(when: &str, by: &str) -> Ability {
    ability(&format!(
        r#"{{"Replacement":{{"when":{{"Damage":{when}}},"by":{by}}}}}"#
    ))
}

const UNICORN: &str = "If a spell would deal damage to a permanent or player, it deals that much damage minus 1 to that permanent or player instead.";
const BARRIER: &str = "If a source would deal damage to a creature you control, it deals that much damage minus 1 to that creature instead.";
const PRESENCE: &str = "If a source would deal 4 or more damage to a permanent or player, that source deals 3 damage to that permanent or player instead.";
const GHOSTS: &str = "If a source would deal damage to a permanent or player, it deals half that damage, rounded down, to that permanent or player instead.";
const MINE: &str = r#"{"to":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#;

#[rstest]
#[case::unicorn(
    lessened(r#"{"source":["Spell"]}"#, r#"{"Subtract":1}"#),
    UNICORN,
    true
)]
#[case::unicorn_minus_two(
    lessened(r#"{"source":["Spell"]}"#, r#"{"Subtract":2}"#),
    UNICORN,
    false
)]
#[case::unicorn_any_source(lessened("{}", r#"{"Subtract":1}"#), UNICORN, false)]
#[case::unicorn_plus(lessened(r#"{"source":["Spell"]}"#, r#"{"Add":1}"#), UNICORN, false)]
#[case::unicorn_prevented(
    lessened(r#"{"source":["Spell"]}"#, r#"{"PreventUpTo":1}"#),
    UNICORN,
    false
)]
#[case::barrier(lessened(MINE, r#"{"Subtract":1}"#), BARRIER, true)]
#[case::barrier_any_creature(
    lessened(r#"{"to":{"Each":[{"Type":"Creature"}]}}"#, r#"{"Subtract":1}"#),
    BARRIER,
    false
)]
#[case::presence(lessened("{}", r#"{"Cap":3}"#), PRESENCE, true)]
#[case::amulet(
    lessened(
        r#"{"to":"You","source":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}"#,
        r#"{"Cap":2}"#
    ),
    "If an instant or sorcery source would deal 3 or more damage to you, it deals 2 damage to you instead.",
    true
)]
#[case::amulet_any_source(
    lessened(r#"{"to":"You"}"#, r#"{"Cap":2}"#),
    "If an instant or sorcery source would deal 3 or more damage to you, it deals 2 damage to you instead.",
    false
)]
#[case::presence_cap_four(lessened("{}", r#"{"Cap":4}"#), PRESENCE, false)]
#[case::presence_combat_only(lessened(r#"{"combat":true}"#, r#"{"Cap":3}"#), PRESENCE, false)]
#[case::ghosts(lessened("{}", r#""Half""#), GHOSTS, true)]
#[case::ghosts_prevented_half(lessened("{}", r#""PreventHalf""#), GHOSTS, false)]
fn ep27_lessened_damage_writes_as_printed(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

fn instead_of_losing(who: &str, by: &str) -> Ability {
    ability(&format!(
        r#"{{"Replacement":{{"when":{{"LoseGame":{who}}},"by":{by}}}}}"#
    ))
}

const THRONE_STEPS: &str = r#"{"Instead":[{"Exile":{"what":"This"}},{"SetLife":{"amount":1}}]}"#;
const THRONE: &str = "If you would lose the game, instead exile ~ and your life total becomes 1.";

#[rstest]
#[case::throne(instead_of_losing("{}", THRONE_STEPS), THRONE, true)]
#[case::another_life(
    instead_of_losing("{}", &THRONE_STEPS.replace("\"amount\":1", "\"amount\":2")),
    THRONE,
    false
)]
#[case::one_step(
    instead_of_losing("{}", r#"{"Instead":[{"Exile":{"what":"This"}}]}"#),
    THRONE,
    false
)]
#[case::an_opponent(
    instead_of_losing(r#"{"who":"Opponents"}"#, THRONE_STEPS),
    THRONE,
    false
)]
#[case::archangel(
    instead_of_losing(
        "{}",
        r#"{"Instead":[{"Exile":{"what":"This"}},{"SetLife":{"amount":{"StartingLife":"You"}}}]}"#
    ),
    "If you would lose the game, instead exile ~ and your life total becomes equal to your starting life total.",
    true
)]
#[case::an_opponents_life(
    instead_of_losing(
        "{}",
        r#"{"Instead":[{"Exile":{"what":"This"}},{"SetLife":{"who":"Opponents","amount":1}}]}"#
    ),
    THRONE,
    false
)]
fn ep27_instead_of_losing_writes_as_printed(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

const NO_LIFE_LOSS: &str = "You don't lose the game for having 0 or less life.";

#[test]
fn ep27_no_loss_for_life_reads_and_writes() {
    let read = golden(NO_LIFE_LOSS, ClauseKind::Static, false);
    assert_eq!(
        read,
        ability(r#"{"Rule":{"rule":{"CantLoseGame":{"who":"You","life_only":true}}}}"#)
    );
}

#[rstest]
#[case::life_only(
    r#"{"CantLoseGame":{"who":"You","life_only":true}}"#,
    NO_LIFE_LOSS,
    true
)]
#[case::every_loss(r#"{"CantLoseGame":{"who":"You"}}"#, NO_LIFE_LOSS, false)]
#[case::life_only_is_not_every_loss(
    r#"{"CantLoseGame":{"who":"You","life_only":true}}"#,
    "You can't lose the game.",
    false
)]
#[case::for_opponents(
    r#"{"CantLoseGame":{"who":"Opponents","life_only":true}}"#,
    NO_LIFE_LOSS,
    false
)]
fn ep27_no_loss_for_life_near_misses_differ(
    #[case] rule: &str,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability(&format!(r#"{{"Rule":{{"rule":{rule}}}}}"#)));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}
