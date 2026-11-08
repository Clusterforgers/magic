//! Goal: exiling from the top of a library until a card matches, with the card found and the rest exiled under names of their own, is written as the card says, and a different filter, permission, owner or place reads otherwise.
use rstest::rstest;

use super::*;

fn wordings(json: &str) -> Vec<String> {
    written_variants(&ability(json))
}

fn written(json: &str, clause: &str) -> bool {
    wordings(json)
        .iter()
        .any(|wording| same_text(wording, clause))
}

const LEGENDARY: &str = r#"[{"Supertype":"Legendary"}]"#;
const LESSER_NONLAND: &str = r#"[{"Not":{"Type":"Land"}},{"Supertype":"Legendary"},{"Compare":{"op":"Lt","stat":"ManaValue","to":{"Stat":{"of":"Triggering","stat":"ManaValue"}}}}]"#;
const LAND: &str = r#"[{"Type":"Land"},{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]}]"#;
const SPELLS: &str = r#"[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]"#;

const CAST: &str = r#"{"CastFromZone":{"free":true,"what":{"Bound":"found_moved3"}}}"#;
const CAST_PAID: &str = r#"{"CastFromZone":{"what":{"Bound":"found_moved3"}}}"#;
const CAST_TURN: &str = r#"{"Rule":{"duration":"UntilEndOfTurn","rule":{"MayCast":{"free":true,"from":"Exile","only":{"Bound":"found_moved3"},"what":[{"Not":{"Type":"Land"}}]}}}}"#;
const PLAY_WHILE: &str = r#"{"Rule":{"duration":"WhileSourceOnBattlefield","rule":{"MayCast":{"from":"Exile","only":{"Bound":"found_moved3"},"play":true}}}}"#;
const PLAY_TURN: &str = r#"{"Rule":{"duration":"UntilEndOfTurn","rule":{"MayCast":{"from":"Exile","only":{"Bound":"found_moved3"},"play":true}}}}"#;
const ONTO: &str = r#"{"Move":{"to":{"Battlefield":{}},"what":{"Bound":"found_moved3"}}}"#;
const ONTO_TAPPED: &str =
    r#"{"Move":{"to":{"Battlefield":{"tapped":true}},"what":{"Bound":"found_moved3"}}}"#;

const REST: &str = r#"{"Move":{"to":{"Library":{"order":"Random","place":"Bottom"}},"what":{"Each":[{"Is":{"Bound":"rest_moved4"}},{"In":"Exile"}]}}}"#;
const REST_BUT_FOUND: &str = r#"{"Move":{"to":{"Library":{"order":"Random","place":"Bottom"}},"what":{"Each":[{"Is":{"Bound":"rest_moved4"}},{"Not":{"Is":{"Bound":"found_moved3"}}},{"In":"Exile"}]}}}"#;
const REST_CHOSEN: &str = r#"{"Move":{"to":{"Library":{"order":"Chosen","place":"Bottom"}},"what":{"Each":[{"Is":{"Bound":"rest_moved4"}},{"In":"Exile"}]}}}"#;
const NOT_CAST: &str = r#"{"Move":{"to":{"Library":{"order":"Random","place":"Bottom"}},"what":{"Each":[{"AnyOf":[{"Is":{"Bound":"rest_moved4"}},{"Is":{"Bound":"found_moved3"}}]},{"In":"Exile"}]}}}"#;

fn spell(who: &str, filter: &str, steps: &[&str]) -> String {
    let who = if who == "You" {
        String::new()
    } else {
        format!(r#","who":"{who}""#)
    };
    format!(
        r#"{{"Spell":{{"effects":[{{"RevealUntil":{{"as":"found","filter":{filter},"revealed_as":"revealed2"{who}}}}},{{"Exile":{{"as":"found_moved3","what":{{"Bound":"found"}}}}}},{{"Exile":{{"as":"rest_moved4","what":{{"Bound":"revealed2"}}}}}},{}]}}}}"#,
        steps.join(",")
    )
}

fn grima(cast_filter: &str) -> String {
    let cast_filter = &cast_filter[1..cast_filter.len() - 1];
    let choose = format!(
        r#"{{"Choose":{{"as":"picked5","count":{{"max":1,"min":0}},"from":{{"Each":[{cast_filter},{{"Is":{{"Bound":"found_moved3"}}}},{{"In":"Exile"}}]}}}}}},{{"CastFromZone":{{"free":true,"what":{{"Bound":"picked5"}}}}}}"#
    );
    spell("Triggering", SPELLS, &[&choose, NOT_CAST])
}

const JODAH: &str = "Exile cards from the top of your library until you exile a legendary nonland card with lesser mana value. You may cast that card without paying its mana cost. Put the rest on the bottom of your library in a random order.";
const CODIE: &str = "Exile cards from the top of your library until you exile an instant or sorcery card with lesser mana value. Until end of turn, you may cast that card without paying its mana cost. Put each other card exiled this way on the bottom of your library in a random order.";
const DOCTOR: &str = "Exile cards from the top of your library until you exile a legendary card. You may play that card for as long as ~ remains on the battlefield. Put the rest of those exiled cards on the bottom of your library in a random order.";
const VENTURE: &str = "Exile cards from the top of your library until you exile a land card. Put that card onto the battlefield and the rest on the bottom of your library in a random order.";
const GRIMA: &str = "That player exiles cards from the top of their library until they exile an instant or sorcery card. You may cast that card without paying its mana cost. Then that player puts the exiled cards that weren't cast this way on the bottom of their library in a random order.";

#[rstest]
#[case::cast_free(spell("You", LESSER_NONLAND, &[CAST, REST]), JODAH, true)]
#[case::cast_paid(spell("You", LESSER_NONLAND, &[CAST_PAID, REST]), JODAH, false)]
#[case::other_filter(spell("You", LEGENDARY, &[CAST, REST]), JODAH, false)]
#[case::rest_in_chosen_order(spell("You", LESSER_NONLAND, &[CAST, REST_CHOSEN]), JODAH, false)]
#[case::cast_this_turn(spell("You", r#"[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"Compare":{"op":"Lt","stat":"ManaValue","to":{"Stat":{"of":"Triggering","stat":"ManaValue"}}}}]"#, &[CAST_TURN, REST]), CODIE, true)]
#[case::play_while(spell("You", LEGENDARY, &[PLAY_WHILE, REST_BUT_FOUND]), DOCTOR, true)]
#[case::play_this_turn(spell("You", LEGENDARY, &[PLAY_TURN, REST_BUT_FOUND]), DOCTOR, false)]
#[case::onto(spell("You", LAND, &[ONTO, REST]), VENTURE, true)]
#[case::onto_tapped(spell("You", LAND, &[ONTO_TAPPED, REST]), VENTURE, false)]
#[case::that_player(grima(SPELLS), GRIMA, true)]
#[case::that_player_casts_another_kind(grima(r#"[{"Type":"Instant"}]"#), GRIMA, false)]
#[case::you_not_that_player(spell("You", SPELLS, &[CAST, NOT_CAST]), GRIMA, false)]
fn exiled_until_a_card_matches(#[case] json: String, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(written(&json, clause), same, "{:?}", wordings(&json));
}

fn returned(kind: &str, counters: &str, plain_to: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"If":{{"condition":{{"CountOf":{{"op":"Ge","right":1,"selector":[{kind}],"subject":{{"Target":0}}}}}},"otherwise":[{{"Move":{{"to":{{"Battlefield":{plain_to}}},"what":{{"Target":0}}}}}}],"then":[{{"Move":{{"to":{{"Battlefield":{{"counters":{counters}}}}},"what":{{"Target":0}}}}}}]}}}}],"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"OwnedBy":"You"}},{{"In":"Graveyard"}}]}}}}]}}}}"#
    )
}

const HERO: &str = r#"{"Subtype":"Hero"}"#;
const CREATURE: &str = r#"{"Type":"Creature"}"#;
const TWO: &str = r#"[["+1/+1",2]]"#;
const ONE: &str = r#"[["+1/+1",1]]"#;
const RETURN: &str = "Return target creature card from your graveyard to the battlefield.";

#[rstest]
#[case::hero(
    returned(HERO, TWO, "{}"),
    "If a Hero enters this way, it enters with two additional +1/+1 counters on it.",
    true
)]
#[case::creature_after(
    returned(CREATURE, TWO, "{}"),
    "It enters with two additional +1/+1 counters on it if it's a creature.",
    true
)]
#[case::one_counter(
    returned(CREATURE, ONE, "{}"),
    "If a creature enters this way, it enters with an additional +1/+1 counter on it.",
    true
)]
#[case::other_kind(
    returned(CREATURE, TWO, "{}"),
    "If a Hero enters this way, it enters with two additional +1/+1 counters on it.",
    false
)]
#[case::other_count(
    returned(HERO, ONE, "{}"),
    "If a Hero enters this way, it enters with two additional +1/+1 counters on it.",
    false
)]
#[case::plain_tapped(
    returned(HERO, TWO, r#"{"tapped":true}"#),
    "If a Hero enters this way, it enters with two additional +1/+1 counters on it.",
    false
)]
fn returned_with_counters_of_a_kind(
    #[case] json: String,
    #[case] sentence: &str,
    #[case] same: bool,
) {
    let clause = format!("{RETURN} {sentence}");
    assert_eq!(written(&json, &clause), same, "{:?}", wordings(&json));
}

const CODIE_RULE: &str = r#"{"Rule":{"rule":{"Cant":{"action":{"Cast":{"filter":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]}]}},"who":"You"}}}}"#;
const NOT_LANDS_RULE: &str = r#"{"Rule":{"rule":{"Cant":{"action":{"Cast":{"filter":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Planeswalker"}]}]}},"who":"You"}}}}"#;
const ATTACKED: &str = r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"AttackingPlayer":"You"}]},"op":"Ge","right":1}},"mana":"{2}"}}"#;
const ATTACKED_BY_TWO: &str = r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"AttackingPlayer":"You"}]},"op":"Ge","right":2}},"mana":"{2}"}}"#;
const JODAH_WHEN: &str = r#"{"Triggered":{"effects":[{"Draw":{"count":1}}],"when":{"SpellCast":{"by":"You","what":[{"Supertype":"Legendary"},{"WasCast":{"by":"You","from":"Hand"}},{"OwnedBy":"You"}]}}}}"#;
const ANY_HAND_WHEN: &str = r#"{"Triggered":{"effects":[{"Draw":{"count":1}}],"when":{"SpellCast":{"by":"You","what":[{"Supertype":"Legendary"},{"WasCast":{"by":"You","from":"Hand"}}]}}}}"#;

#[rstest]
#[case::permanent_spells(CODIE_RULE, "You can't cast permanent spells.", true)]
#[case::not_every_permanent_type(NOT_LANDS_RULE, "You can't cast permanent spells.", false)]
#[case::creature_attacking_you(
    ATTACKED,
    "This spell costs {2} less to cast if a creature is attacking you.",
    true
)]
#[case::two_attacking(
    ATTACKED_BY_TWO,
    "This spell costs {2} less to cast if a creature is attacking you.",
    false
)]
#[case::legendary_from_your_hand(
    JODAH_WHEN,
    "Whenever you cast a legendary spell from your hand, draw a card.",
    true
)]
#[case::from_a_hand_you_do_not_own(
    ANY_HAND_WHEN,
    "Whenever you cast a legendary spell from your hand, draw a card.",
    false
)]
fn rows_for_the_cards_of_the_pack(#[case] json: &str, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(written(json, clause), same, "{:?}", wordings(json));
}

fn next_cast(once: bool) -> String {
    let once = if once { "" } else { r#""once":false,"# };
    format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"DelayedTrigger":{{{once}"effects":[{{"Draw":{{"count":1}}}}],"this_turn":true,"when":{{"SpellCast":{{"by":"You"}}}}}}}}]}}}}"#
    )
}

#[rstest]
#[case::once(
    next_cast(true),
    "{T}: When you next cast a spell this turn, draw a card.",
    true
)]
#[case::each_time(
    next_cast(false),
    "{T}: When you next cast a spell this turn, draw a card.",
    false
)]
fn the_next_spell_cast_this_turn(#[case] json: String, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(written(&json, clause), same, "{:?}", wordings(&json));
}

fn doctors(max: u32, amount: u32, second: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"Choose":{{"as":"card","count":{{"max":{max},"min":0}},"from":{{"Each":[{{"Subtype":"Doctor"}}]}}}}}},{{"Either":{{"first":[{{"Exile":{{"what":{{"Each":[{{"Type":"Creature"}},{{"Not":{{"Is":{{"Bound":"card"}}}}}}]}}}}}},{{"DealDamage":{{"amount":{amount},"to":"You"}}}}],"second":{second}}}}}],"when":{{"Chapter":{{"number":4}}}}}}}}"#
    )
}

const DOCTORS: &str = "IV — Choose up to three Doctors. You may exile all other creatures. If you do, ~ deals 13 damage to you.";

#[rstest]
#[case::as_printed(doctors(3, 13, "[]"), true)]
#[case::other_number(doctors(2, 13, "[]"), false)]
#[case::other_damage(doctors(3, 12, "[]"), false)]
#[case::something_instead(doctors(3, 13, r#"[{"Draw":{"count":1}}]"#), false)]
fn chosen_ones_spared(#[case] json: String, #[case] same: bool) {
    assert_eq!(written(&json, DOCTORS), same, "{:?}", wordings(&json));
}
