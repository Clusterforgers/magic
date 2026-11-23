//! Goal: the Star Trek wordings of r117-01 read and write back as the card's text: the top X cards of a library with the where clause between sentences (look, reveal, exile), "the other" of two cards looked at, a restriction under a trailing "if you control", a search or choice filter of two card phrases joined by "or" (a side of several qualities), "During your turn," before a subtype group, the card and a group as one subject, "Lands you control enter untapped.", and "it" for the card after an intervening "if" about it; a near miss stays with the table.
use super::*;

#[rstest]
#[case::look_top_x(
    ClauseKind::Triggered,
    "When ~ enters, look at the top X cards of your library, where X is the number of creature types among non-Borg creatures you control. Put one of them into your hand and the rest on the bottom of your library in a random order."
)]
#[case::reveal_top_x(
    ClauseKind::Spell,
    "Reveal the top X cards of your library, where X is the number of differently named lands you control. Put all permanent cards revealed this way onto the battlefield tapped and the rest on the bottom of your library in a random order."
)]
#[case::exile_top_x(
    ClauseKind::Triggered,
    "Whenever you attack, exile the top X cards of your library, where X is the number of opponents you attacked this turn. Until the end of your next turn, you may play those cards."
)]
#[case::the_other_of_two(
    ClauseKind::Spell,
    "Look at the top two cards of your library. Put one of them into your hand and the other into your graveyard."
)]
#[case::basic_or_planet(
    ClauseKind::Spell,
    "Search your library for a basic land card or Planet card, put it onto the battlefield tapped, then shuffle."
)]
#[case::basic_or_planet_to_hand(
    ClauseKind::Triggered,
    "When ~ enters, search your library for a basic land card or Planet card, reveal it, put it into your hand, then shuffle."
)]
#[case::subtype_or_legendary(
    ClauseKind::Spell,
    "Search your library for a Forest card or legendary creature card, put it into your hand, then shuffle."
)]
#[case::plains_or_permanent(
    ClauseKind::Spell,
    "Look at the top five cards of your library. You may reveal a Plains card or nonland permanent card with mana value 3 or less from among them and put it into your hand. Put the rest on the bottom in a random order."
)]
#[case::during_subtype(
    ClauseKind::Static,
    "During your turn, Spacecraft you control have hexproof."
)]
#[case::itself_and_group(
    ClauseKind::Static,
    "During your turn, ~ and creatures you control have hexproof."
)]
#[case::itself_and_group_plain(ClauseKind::Static, "~ and creatures you control have flying.")]
#[case::unblockable_if(ClauseKind::Static, "~ can't be blocked if you control a Spacecraft.")]
#[case::she_deals(
    ClauseKind::Triggered,
    "When ~ enters, she deals X damage to target attacking or blocking creature an opponent controls, where X is the number of creature types among non-Borg creatures you control."
)]
#[case::tap_and_stun(
    ClauseKind::Triggered,
    "When ~ enters, tap target creature an opponent controls and put a stun counter on it."
)]
#[case::untap_and_counter(
    ClauseKind::Triggered,
    "Whenever ~ attacks, untap target creature you control and put a +1/+1 counter on it."
)]
#[case::stun_counter(ClauseKind::Spell, "Put a stun counter on target creature.")]
#[case::finality_return(
    ClauseKind::Triggered,
    "Whenever ~ enters or deals combat damage to a player, return target creature card with mana value 4 or less from your graveyard to the battlefield with a finality counter on it."
)]
#[case::you_after_an_opponent(
    ClauseKind::Spell,
    "Target opponent draws a card. You draw two cards."
)]
#[case::labelled_modes(
    ClauseKind::Modal,
    "Choose one \u{2014}\n\u{2022} Battle Stations \u{2014} Put a +1/+1 counter on each creature you control.\n\u{2022} Begin Negotiations \u{2014} Target opponent draws a card. You draw two cards."
)]
#[case::up_to_three_lands(
    ClauseKind::Triggered,
    "When ~ enters, draw three cards. You may put up to three land cards from your hand onto the battlefield tapped."
)]
#[case::lands_enter_untapped(ClauseKind::Static, "Lands you control enter untapped.")]
#[case::it_after_if(
    ClauseKind::Triggered,
    "Whenever a creature dies, if ~ isn't a creature, put a charge counter on it."
)]
#[case::it_after_if_spell(
    ClauseKind::Triggered,
    "Whenever you cast a noncreature spell, if ~ isn't a creature, put two charge counters on it."
)]
fn r117_01_wordings_read_and_write_back(#[case] kind: ClauseKind, #[case] text: &str) {
    let read = clause(text, kind, false).unwrap_or_else(|| panic!("not read: {text}"));
    let written = written_variants(&read);
    assert!(
        written.iter().any(|one| same_text(one, text)),
        "{text}: {written:?}"
    );
}

#[rstest]
#[case::top_x_without_where(
    ClauseKind::Spell,
    "Look at the top X cards of your library. Put one of them into your hand and the rest on the bottom of your library in a random order."
)]
#[case::two_single_cards(
    ClauseKind::Spell,
    "Search your library for a Plains card or Island card, put it into your hand, then shuffle."
)]
#[case::the_other_of_three(
    ClauseKind::Spell,
    "Look at the top three cards of your library. Put one of them into your hand and the other into your graveyard."
)]
#[case::she_of_another_event(
    ClauseKind::Triggered,
    "Whenever a creature dies, she deals 1 damage to any target."
)]
#[case::tap_and_counter_on_another(
    ClauseKind::Triggered,
    "When ~ enters, tap target creature an opponent controls and put a stun counter on ~."
)]
#[case::if_not_a_control(ClauseKind::Static, "~ can't be blocked if it's your turn.")]
#[case::and_nothing(ClauseKind::Static, "~ and have flying.")]
#[case::lands_enter_sideways(ClauseKind::Static, "Lands you control enter sideways.")]
fn r117_01_near_misses_stay_with_the_table(#[case] kind: ClauseKind, #[case] text: &str) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}
