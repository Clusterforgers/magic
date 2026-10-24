//! Goal: the writers give the oracle wording of the top card revealed and put onto the battlefield, cast or put on the bottom, of the owner of a permanent shuffling it away and revealing the card on top, of a free cast from the hand, of an untap of a few lands and of a token made by populate that gains a keyword and is sacrificed, and a wrong kind, limit, number or step never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const TOP: &str = r#"{"Bound":"top"}"#;
const BOTTOM: &str = r#"{"Library":{"place":"Bottom"}}"#;

fn creature_test(then: &str, otherwise: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"SpellCast":{{"what":[],"by":"Opponents"}}}},"effects":[{{"LookAtTop":{{"as":"top"}}}},{{"Reveal":{{"what":{TOP}}}}},{{"If":{{"condition":{{"Matches":{{"subject":{TOP},"selector":[{{"Type":"Creature"}}]}}}},"then":[{then}],"otherwise":[{otherwise}]}}}}]}}}}"#
    )
}

fn put_onto_the_battlefield() -> String {
    format!(r#"{{"Move":{{"what":{TOP},"to":{{"Battlefield":{{}}}}}}}}"#)
}

fn put_on_the_bottom() -> String {
    format!(r#"{{"Move":{{"what":{TOP},"to":{BOTTOM}}}}}"#)
}

fn may_put_on_the_bottom() -> String {
    format!(r#"{{"May":{{"effects":[{}]}}}}"#, put_on_the_bottom())
}

#[rstest]
#[case::a_may(
    may_put_on_the_bottom(),
    "Whenever an opponent casts a spell, reveal the top card of your library. If it's a creature card, put it onto the battlefield. Otherwise, you may put that card on the bottom of your library."
)]
#[case::certain(
    put_on_the_bottom(),
    "Whenever an opponent casts a spell, reveal the top card of your library. If it's a creature card, put it onto the battlefield. Otherwise, put it on the bottom of your library."
)]
fn the_top_card_revealed_and_put_onto_the_battlefield_or_the_bottom_is_written_as_the_oracle_words_it(
    #[case] otherwise: String,
    #[case] clause: &str,
) {
    assert!(
        written(
            &creature_test(&put_onto_the_battlefield(), &otherwise),
            clause
        ),
        "{clause}"
    );
}

#[rstest]
#[case::a_land(
    "Whenever an opponent casts a spell, reveal the top card of your library. If it's a land card, put it onto the battlefield. Otherwise, you may put that card on the bottom of your library."
)]
#[case::the_top_of_the_library(
    "Whenever an opponent casts a spell, reveal the top card of your library. If it's a creature card, put it onto the battlefield. Otherwise, you may put that card on top of your library."
)]
#[case::no_otherwise(
    "Whenever an opponent casts a spell, reveal the top card of your library. If it's a creature card, put it onto the battlefield."
)]
#[case::the_graveyard(
    "Whenever an opponent casts a spell, reveal the top card of your library. If it's a creature card, put it into your graveyard. Otherwise, you may put that card on the bottom of your library."
)]
#[case::a_player_who_casts_it(
    "Whenever you cast a spell, reveal the top card of your library. If it's a creature card, put it onto the battlefield. Otherwise, you may put that card on the bottom of your library."
)]
#[case::the_second_card(
    "Whenever an opponent casts a spell, reveal the top two cards of your library. If it's a creature card, put it onto the battlefield. Otherwise, you may put that card on the bottom of your library."
)]
fn the_top_card_revealed_does_not_read_as_another_kind_place_or_player(#[case] clause: &str) {
    let json = creature_test(&put_onto_the_battlefield(), &may_put_on_the_bottom());
    assert!(!written(&json, clause), "{clause}");
}

fn descendants(shared: &str, otherwise: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep","whose":"You"}}}},"effects":[{{"LookAtTop":{{"as":"top"}}}},{{"Reveal":{{"what":{TOP}}}}},{{"If":{{"condition":{{"Matches":{{"subject":{TOP},"selector":[{{"Type":"Creature"}},{{"SharesSubtype":{{"Each":[{shared}]}}}}]}}}},"then":[{{"CastFromZone":{{"what":{TOP},"free":true,"otherwise":[{}]}}}}],"otherwise":[{otherwise}]}}}}]}}}}"#,
        put_on_the_bottom()
    )
}

const A_CREATURE_YOU_CONTROL: &str = r#"{"Type":"Creature"},{"ControlledBy":"You"}"#;

#[test]
fn a_cast_of_the_top_card_or_a_move_to_the_bottom_is_written_as_the_oracle_words_it() {
    assert!(written(
        &descendants(A_CREATURE_YOU_CONTROL, &put_on_the_bottom()),
        "At the beginning of your upkeep, reveal the top card of your library. If it's a creature card that shares a creature type with a creature you control, you may cast it without paying its mana cost. If you don't cast it, put it on the bottom of your library."
    ));
}

#[rstest]
#[case::another_creature(
    "At the beginning of your upkeep, reveal the top card of your library. If it's a creature card that shares a creature type with another creature you control, you may cast it without paying its mana cost. If you don't cast it, put it on the bottom of your library."
)]
#[case::an_opponent_s_creature(
    "At the beginning of your upkeep, reveal the top card of your library. If it's a creature card that shares a creature type with a creature an opponent controls, you may cast it without paying its mana cost. If you don't cast it, put it on the bottom of your library."
)]
#[case::no_bottom(
    "At the beginning of your upkeep, reveal the top card of your library. If it's a creature card that shares a creature type with a creature you control, you may cast it without paying its mana cost."
)]
#[case::an_otherwise(
    "At the beginning of your upkeep, reveal the top card of your library. If it's a creature card that shares a creature type with a creature you control, you may cast it without paying its mana cost. If you don't cast it, put it on the bottom of your library. Otherwise, put it on the bottom of your library."
)]
fn a_cast_of_the_top_card_does_not_read_as_another_group_or_ending(#[case] clause: &str) {
    assert!(
        !written(
            &descendants(A_CREATURE_YOU_CONTROL, &put_on_the_bottom()),
            clause
        ),
        "{clause}"
    );
}

#[test]
fn a_cast_with_no_move_for_the_card_that_fails_the_test_is_not_the_card_that_goes_to_the_bottom() {
    let json = descendants(A_CREATURE_YOU_CONTROL, &may_put_on_the_bottom());
    let clause = "At the beginning of your upkeep, reveal the top card of your library. If it's a creature card that shares a creature type with a creature you control, you may cast it without paying its mana cost. If you don't cast it, put it on the bottom of your library.";
    assert!(!written(&json, clause));
}

const OWNER: &str = r#"{"OwnerOf":{"Target":0}}"#;

fn warp(kinds: &str, under: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[]}}}}],"effects":[{{"Move":{{"what":{{"Target":0}},"to":{{"Library":{{"place":"Shuffled"}}}}}}}},{{"LookAtTop":{{"who":{OWNER},"as":"top"}}}},{{"Reveal":{{"what":{TOP}}}}},{{"If":{{"condition":{{"Matches":{{"subject":{TOP},"selector":[{kinds}]}}}},"then":[{{"Move":{{"what":{TOP},"to":{{"Battlefield":{{"under":{under}}}}}}}}}]}}}}]}}}}"#
    )
}

const PERMANENTS: &str = r#"{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"},{"Type":"Battle"}]}"#;

#[test]
fn a_permanent_shuffled_away_and_the_top_card_of_its_owner_is_written_as_the_oracle_words_it() {
    assert!(written(
        &warp(PERMANENTS, OWNER),
        "The owner of target permanent shuffles it into their library, then reveals the top card of their library. If it's a permanent card, they put it onto the battlefield."
    ));
}

#[rstest]
#[case::a_creature(
    r#"{"Type":"Creature"}"#,
    "The owner of target permanent shuffles it into their library, then reveals the top card of their library. If it's a permanent card, they put it onto the battlefield."
)]
#[case::a_land_card(
    PERMANENTS,
    "The owner of target permanent shuffles it into their library, then reveals the top card of their library. If it's a land card, they put it onto the battlefield."
)]
#[case::its_controller(
    PERMANENTS,
    "Target permanent's controller shuffles it into their library, then reveals the top card of their library. If it's a permanent card, they put it onto the battlefield."
)]
fn a_permanent_shuffled_away_does_not_read_as_another_kind_or_player(
    #[case] kinds: &str,
    #[case] clause: &str,
) {
    assert!(!written(&warp(kinds, OWNER), clause), "{clause}");
}

#[test]
fn a_permanent_put_onto_the_battlefield_under_you_is_not_put_there_by_its_owner() {
    assert!(!written(
        &warp(PERMANENTS, r#""You""#),
        "The owner of target permanent shuffles it into their library, then reveals the top card of their library. If it's a permanent card, they put it onto the battlefield."
    ));
}

fn free_cast(limit: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":"AnyTarget"}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":"X"}}}},{{"Choose":{{"from":{{"Each":[{{"In":"Hand"}},{{"OwnedBy":"You"}},{{"Not":{{"Type":"Land"}}}}{limit}]}},"count":{{"min":0,"max":1}},"as":"spell"}}}},{{"CastFromZone":{{"what":{{"Bound":"spell"}},"free":true}}}}]}}}}"#
    )
}

const X_OR_LESS: &str = r#",{"Compare":{"stat":"ManaValue","op":"Le","to":"X"}}"#;

#[rstest]
#[case::x(
    X_OR_LESS,
    "~ deals X damage to any target. You may cast a spell with mana value X or less from your hand without paying its mana cost."
)]
#[case::three(
    r#",{"Compare":{"stat":"ManaValue","op":"Le","to":3}}"#,
    "~ deals X damage to any target. You may cast a spell with mana value 3 or less from your hand without paying its mana cost."
)]
#[case::no_limit(
    "",
    "~ deals X damage to any target. You may cast a spell from your hand without paying its mana cost."
)]
fn a_free_cast_from_the_hand_is_written_as_the_oracle_words_it(
    #[case] limit: &str,
    #[case] clause: &str,
) {
    assert!(written(&free_cast(limit), clause), "{clause}");
}

#[rstest]
#[case::another_limit(
    "~ deals X damage to any target. You may cast a spell with mana value 3 or less from your hand without paying its mana cost."
)]
#[case::a_graveyard(
    "~ deals X damage to any target. You may cast a spell with mana value X or less from your graveyard without paying its mana cost."
)]
#[case::a_creature_spell(
    "~ deals X damage to any target. You may cast a creature spell with mana value X or less from your hand without paying its mana cost."
)]
#[case::the_mana_cost_paid(
    "~ deals X damage to any target. You may cast a spell with mana value X or less from your hand."
)]
#[case::at_least(
    "~ deals X damage to any target. You may cast a spell with mana value X or greater from your hand without paying its mana cost."
)]
fn a_free_cast_from_the_hand_does_not_read_as_another_limit_zone_or_cost(#[case] clause: &str) {
    assert!(!written(&free_cast(X_OR_LESS), clause), "{clause}");
}

fn frantic(min: u32, max: u32, from: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Draw":{{"count":2}}}},{{"Discard":{{"count":2}}}},{{"Choose":{{"from":{{"Each":[{from}]}},"count":{{"min":{min},"max":{max}}},"as":"lands"}}}},{{"Untap":{{"what":{{"Bound":"lands"}}}}}}]}}}}"#
    )
}

#[rstest]
#[case::three_lands(
    0,
    3,
    r#"{"Type":"Land"}"#,
    "Draw two cards, then discard two cards. Untap up to three lands."
)]
#[case::two_lands(
    0,
    2,
    r#"{"Type":"Land"}"#,
    "Draw two cards, then discard two cards. Untap up to two lands."
)]
#[case::creatures(
    0,
    3,
    r#"{"Type":"Creature"}"#,
    "Draw two cards, then discard two cards. Untap up to three creatures."
)]
fn an_untap_of_a_few_permanents_is_written_as_the_oracle_words_it(
    #[case] min: u32,
    #[case] max: u32,
    #[case] from: &str,
    #[case] clause: &str,
) {
    assert!(written(&frantic(min, max, from), clause), "{clause}");
}

#[rstest]
#[case::another_number("Draw two cards, then discard two cards. Untap up to two lands.")]
#[case::another_kind("Draw two cards, then discard two cards. Untap up to three creatures.")]
#[case::all_of_them("Draw two cards, then discard two cards. Untap three lands.")]
#[case::a_target("Draw two cards, then discard two cards. Untap target land.")]
fn an_untap_of_a_few_lands_does_not_read_as_another_number_kind_or_choice(#[case] clause: &str) {
    assert!(
        !written(&frantic(0, 3, r#"{"Type":"Land"}"#), clause),
        "{clause}"
    );
}

#[test]
fn an_untap_of_exactly_three_lands_is_not_up_to_three() {
    assert!(!written(
        &frantic(3, 3, r#"{"Type":"Land"}"#),
        "Draw two cards, then discard two cards. Untap up to three lands."
    ));
}

fn iteration(keyword: &str, later: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"BeginCombat","whose":"You"}}}},"effects":[{{"Choose":{{"from":{{"Each":[{{"Type":"Creature"}},"Token",{{"ControlledBy":"You"}}]}},"as":"token"}}}},{{"CreateToken":{{"token":{{"CopyOf":{{"of":{{"Bound":"token"}}}}}},"as":"tokens"}}}},{{"Continuous":{{"affected":{{"Bound":"tokens"}},"mods":[{{"AddKeyword":"{keyword}"}}],"duration":"Forever"}}}},{{"DelayedTrigger":{{"when":{{"StepBegins":{{"step":"End"}}}},"effects":[{later}]}}}}]}}}}"#
    )
}

const SACRIFICE_IT: &str = r#"{"Sacrifice":{"filter":[{"Is":{"Bound":"tokens"}}]}}"#;

#[test]
fn populate_with_a_keyword_and_a_sacrifice_at_the_end_step_is_written_as_the_oracle_words_it() {
    assert!(written(
        &iteration("Haste", SACRIFICE_IT),
        "At the beginning of combat on your turn, populate. The token created this way gains haste. Sacrifice it at the beginning of the next end step."
    ));
}

#[rstest]
#[case::another_keyword(
    "At the beginning of combat on your turn, populate. The token created this way gains flying. Sacrifice it at the beginning of the next end step."
)]
#[case::an_exile(
    "At the beginning of combat on your turn, populate. The token created this way gains haste. Exile it at the beginning of the next end step."
)]
#[case::no_end_of_it(
    "At the beginning of combat on your turn, populate. The token created this way gains haste."
)]
#[case::until_end_of_turn(
    "At the beginning of combat on your turn, populate. The token created this way gains haste until end of turn. Sacrifice it at the beginning of the next end step."
)]
fn populate_with_a_keyword_does_not_read_as_another_keyword_or_ending(#[case] clause: &str) {
    assert!(
        !written(&iteration("Haste", SACRIFICE_IT), clause),
        "{clause}"
    );
}
