//! Goal: the top cards looked at or revealed and the takes from among them (several takes, X cards, mixed, historic and non-Human filters, the count of lands, the rest to the hand, the graveyard, the bottom or shuffled in) read into the DSL and write back as the same text; a take of another shape stays unread.
use super::*;

const TOP_BOTTOM: &str =
    r#"{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Bottom","order":"Random"}}}}"#;

fn look(count: &str) -> String {
    format!(r#"{{"LookAtTop":{{"count":{count},"as":"top"}}}}"#)
}

fn kept(filter: &str, count: &str, name: &str) -> String {
    format!(
        r#"{{"Choose":{{"from":{{"Each":[{{"In":"Library"}},{{"Is":{{"Bound":"top"}}}},{filter}]}},"count":{count},"as":"{name}"}}}}"#
    )
}

fn put(name: &str, to: &str) -> String {
    format!(r#"{{"Move":{{"what":{{"Bound":"{name}"}},"to":{to}}}}}"#)
}

const MAY: &str = r#"{"min":0,"max":1}"#;
const REVEAL: &str = r#"{"Reveal":{"what":{"Bound":"kept"}}}"#;
const ANY: &str = r#"{"min":0,"max":null}"#;
const PERMANENT: &str = r#"{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"},{"Type":"Battle"}]}"#;

#[rstest]
#[case::mixed_list(
    "Look at the top four cards of your library. You may reveal a Human or artifact card from among them and put it into your hand. Put the rest on the bottom of your library in a random order.",
    ClauseKind::Spell,
    false,
    format!(
        r#"{{"Spell":{{"effects":[{},{},{REVEAL},{},{TOP_BOTTOM}]}}}}"#,
        look("4"),
        kept(r#"{"AnyOf":[{"Subtype":"Human"},{"Type":"Artifact"}]}"#, MAY, "kept"),
        put("kept", r#""Hand""#)
    )
)]
#[case::two_takes_in_one_sentence(
    "Reveal the top six cards of your library. You may put up to one land card from among them onto the battlefield tapped and up to one Elf card from among them into your hand. Put the rest on the bottom of your library in a random order.",
    ClauseKind::Spell,
    false,
    format!(
        r#"{{"Spell":{{"effects":[{},{{"Reveal":{{"what":{{"Bound":"top"}}}}}},{},{},{},{},{TOP_BOTTOM}]}}}}"#,
        look("6"),
        kept(r#"{"Type":"Land"}"#, MAY, "kept"),
        put("kept", r#"{"Battlefield":{"tapped":true}}"#),
        kept(r#"{"Subtype":"Elf"}"#, MAY, "kept_2"),
        put("kept_2", r#""Hand""#)
    )
)]
#[case::two_takes_in_two_sentences(
    "Reveal the top six cards of your library. You may put a permanent card from among them onto the battlefield with an indestructible counter on it. You may put a permanent card from among them into your hand. Put the rest into your graveyard.",
    ClauseKind::Spell,
    false,
    format!(
        r#"{{"Spell":{{"effects":[{},{{"Reveal":{{"what":{{"Bound":"top"}}}}}},{},{},{},{},{}]}}}}"#,
        look("6"),
        kept(PERMANENT, MAY, "kept"),
        put("kept", r#"{"Battlefield":{"counters":[[{"Other":"indestructible"},1]]}}"#),
        kept(PERMANENT, MAY, "kept_2"),
        put("kept_2", r#""Hand""#),
        put("top", r#""Graveyard""#)
    )
)]
#[case::x_cards_and_mana_value_x(
    "Reveal the top X cards of your library. You may put any number of permanent cards with mana value X or less from among them onto the battlefield. Then put all cards revealed this way that weren't put onto the battlefield into your graveyard.",
    ClauseKind::Spell,
    true,
    format!(
        r#"{{"Spell":{{"effects":[{},{{"Reveal":{{"what":{{"Bound":"top"}}}}}},{},{},{}]}}}}"#,
        look(r#""X""#),
        kept(&format!(r#"{PERMANENT},{{"Compare":{{"stat":"ManaValue","op":"Le","to":"X"}}}}"#), ANY, "kept"),
        put("kept", r#"{"Battlefield":{}}"#),
        put("top", r#""Graveyard""#)
    )
)]
#[case::rest_to_the_hand_in_the_sentence(
    "Look at the top five cards of your library. Put any number of permanent cards from among them onto the battlefield and the rest into your hand.",
    ClauseKind::Spell,
    false,
    format!(
        r#"{{"Spell":{{"effects":[{},{},{},{}]}}}}"#,
        look("5"),
        kept(PERMANENT, ANY, "kept"),
        put("kept", r#"{"Battlefield":{}}"#),
        put("top", r#""Hand""#)
    )
)]
#[case::historic_and_shuffled_in(
    "Look at the top four cards of your library. You may reveal a historic card from among them and put it into your hand. Then shuffle the rest into your library.",
    ClauseKind::Spell,
    false,
    format!(
        r#"{{"Spell":{{"effects":[{},{},{REVEAL},{},{}]}}}}"#,
        look("4"),
        kept(r#"{"AnyOf":[{"Type":"Artifact"},{"Supertype":"Legendary"},{"Subtype":"Saga"}]}"#, MAY, "kept"),
        put("kept", r#""Hand""#),
        put("top", r#"{"Library":{"place":"Shuffled"}}"#)
    )
)]
#[case::non_subtype(
    "{5}{G}{U}: Look at the top five cards of your library. You may put a non-Human creature card from among them onto the battlefield. Put the rest on the bottom of your library in a random order.",
    ClauseKind::Activated,
    false,
    format!(
        r#"{{"Activated":{{"cost":"{{5}}{{U}}{{G}}","effects":[{},{},{},{TOP_BOTTOM}]}}}}"#,
        look("5"),
        kept(r#"{"Not":{"Subtype":"Human"}},{"Type":"Creature"}"#, MAY, "kept"),
        put("kept", r#"{"Battlefield":{}}"#)
    )
)]
#[case::lands_you_control(
    "{4}{G}{G}, {T}: Look at the top six cards of your library. You may reveal a creature card with mana value less than or equal to the number of lands you control from among them and put it onto the battlefield. Put the rest on the bottom in a random order.",
    ClauseKind::Activated,
    false,
    format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{4}}{{G}}{{G}}"}},"Tap"],"effects":[{},{},{REVEAL},{},{TOP_BOTTOM}]}}}}"#,
        look("6"),
        kept(r#"{"Type":"Creature"},{"Compare":{"stat":"ManaValue","op":"Le","to":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}}"#, MAY, "kept"),
        put("kept", r#"{"Battlefield":{}}"#)
    )
)]
fn w20a_takes_from_among_the_top_cards_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] x_free: bool,
    #[case] written: String,
) {
    reads_as(text, kind, x_free, &written);
}

#[rstest]
#[case::creature_of_the_event(
    "Whenever equipped creature attacks, look at the top six cards of your library. You may reveal a card that shares a creature type with that creature from among them and put it into your hand. Put the rest on the bottom of your library in a random order.",
    ClauseKind::Triggered
)]
#[case::combat_damage_historic(
    "Whenever ~ deals combat damage to a player, look at the top five cards of your library. You may reveal a historic card from among them and put it into your hand. Put the rest on the bottom of your library in a random order.",
    ClauseKind::Triggered
)]
#[case::cast_trigger_with_x_and_shuffle(
    "When you cast this spell, reveal the top X cards of your library. You may put a nonland permanent card with mana value X or less from among them onto the battlefield. Then shuffle the rest into your library.",
    ClauseKind::Triggered
)]
#[case::mixed_list_of_three(
    "Look at the top four cards of your library. You may reveal a Knight, Equipment, or artifact card from among them and put it into your hand. Put the rest on the bottom of your library in a random order.",
    ClauseKind::Spell
)]
#[case::plus_one_counter(
    "Reveal the top four cards of your library. You may put a creature card from among them onto the battlefield with a +1/+1 counter on it. Put the rest into your graveyard.",
    ClauseKind::Spell
)]
#[case::rest_in_the_hand_after_an_exile(
    "Look at the top five cards of your library. Put any number of permanent cards from among them onto the battlefield and the rest into your hand. Exile ~.",
    ClauseKind::Spell
)]
fn w20a_other_places_and_wordings_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    golden(text, kind, true);
}

#[rstest]
#[case::rest_on_top(
    "Look at the top four cards of your library. You may reveal a Human or artifact card from among them and put it into your hand. Put the rest on top of your library in any order."
)]
#[case::mixed_list_without_the_comma(
    "Look at the top four cards of your library. You may reveal a Human, artifact or land card from among them and put it into your hand. Put the rest into your graveyard."
)]
#[case::tapped_and_attacking(
    "Look at the top six cards of your library. You may put a creature card from among them onto the battlefield tapped and attacking. Put the rest on the bottom of your library in a random order."
)]
#[case::chosen_type(
    "Choose a creature type. Look at the top six cards of your library. You may reveal any number of cards of the chosen type from among them and put the revealed cards into your hand. Put the rest on the bottom of your library in a random order."
)]
#[case::creature_of_no_event(
    "Look at the top six cards of your library. You may reveal a card that shares a creature type with that creature from among them and put it into your hand. Put the rest on the bottom of your library in a random order."
)]
#[case::a_take_without_from_among(
    "Look at the top four cards of your library. You may reveal a Human card and put it into your hand. Put the rest into your graveyard."
)]
#[case::mana_value_compared_to_a_number_word(
    "Look at the top four cards of your library. You may put a creature card with mana value less than or equal to three from among them onto the battlefield. Put the rest into your graveyard."
)]
#[case::counter_that_is_no_counter(
    "Reveal the top four cards of your library. You may put a permanent card from among them onto the battlefield with an indestructible on it. Put the rest into your graveyard."
)]
#[case::no_take_at_all(
    "Look at the top four cards of your library. Put the rest into your graveyard."
)]
fn w20a_near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
