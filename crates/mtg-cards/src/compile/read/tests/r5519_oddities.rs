//! Goal: the r55-19 oddities read into the DSL and write back as the printed text (a rule that stops a player's own
//! spells from searching their library, ...), and a near miss (another group, another zone, another verb) stays with
//! the table.
use super::*;

#[rstest]
#[case::own_search(
    "Spells and abilities your opponents control can't cause their controller to search their library.",
    ClauseKind::Static,
    r#"{"Rule":{"rule":{"OwnSpellsCantSearch":"Opponents"}}}"#
)]
#[case::commanders_home(
    "Target player returns each commander they control from the battlefield to the command zone.\nDraw a card.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Move":{"what":{"Each":["Commander",{"ControlledBy":{"Target":0}}]},"to":"Command"}},{"Draw":{}}]}}"#
)]
#[case::tomik_targets(
    "Lands on the battlefield and land cards in graveyards can't be the targets of spells or abilities your opponents control.",
    ClauseKind::Static,
    r#"{"Rule":{"rule":{"Cant":{"who":{"Each":[{"Type":"Land"},{"AnyOf":[{"In":"Battlefield"},{"In":"Graveyard"}]}]},"action":{"BeTargeted":[{"ControlledBy":"Opponents"}]}}}}}"#
)]
#[case::tomik_plays(
    "Your opponents can't play land cards from graveyards.",
    ClauseKind::Static,
    r#"{"Rule":{"rule":{"Cant":{"who":"Opponents","action":{"PlayLands":[{"Type":"Land"},{"In":"Graveyard"}]}}}}}"#
)]
fn r5519_oddities_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    let read = clause(text, kind, false).unwrap_or_else(|| panic!("not read: {text}"));
    assert_eq!(read, ability(json), "{text}");
    let written = crate::compile::written_variants(&read);
    assert!(
        written.iter().any(|wording| same_text(wording, text)),
        "{text}: {written:?}"
    );
}

#[rstest]
#[case::every_player(
    "Spells and abilities players control can't cause their controller to search their library.",
    ClauseKind::Static
)]
#[case::any_library(
    "Spells and abilities your opponents control can't cause their controller to search libraries.",
    ClauseKind::Static
)]
#[case::draw_not_search(
    "Spells and abilities your opponents control can't cause their controller to draw cards.",
    ClauseKind::Static
)]
#[case::to_the_hand(
    "Target player returns each commander they control from the battlefield to their hand.",
    ClauseKind::Spell
)]
#[case::creatures_not_commanders(
    "Target player returns each creature they control from the battlefield to the command zone.",
    ClauseKind::Spell
)]
#[case::anyones_targets(
    "Lands on the battlefield and land cards in graveyards can't be the targets of spells or abilities.",
    ClauseKind::Static
)]
#[case::creature_cards(
    "Lands on the battlefield and creature cards in graveyards can't be the targets of spells or abilities your opponents control.",
    ClauseKind::Static
)]
#[case::from_exile("Your opponents can't play land cards from exile.", ClauseKind::Static)]
#[case::three_counters(
    "Return target creature card from your graveyard to the battlefield. If a Hero enters this way, it enters with three additional +1/+1 counters on it.",
    ClauseKind::Spell
)]
#[case::a_soldier(
    "Return target creature card from your graveyard to the battlefield. If a Soldier enters this way, it enters with two additional +1/+1 counters on it.",
    ClauseKind::Spell
)]
#[case::all_two(
    "At the beginning of your end step, if you control an Employee, a Performer, or a Robot, you may get {TK} or create a Treasure token. If you control all two, you may put a sticker on a nonland permanent you own.",
    ClauseKind::Triggered
)]
#[case::upkeep_not_end_step(
    "At the beginning of your upkeep, if you control an Employee, a Performer, or a Robot, you may get {TK} or create a Treasure token. If you control all three, you may put a sticker on a nonland permanent you own.",
    ClauseKind::Triggered
)]
fn r5519_near_misses_stay_with_the_table(#[case] text: &str, #[case] kind: ClauseKind) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}
