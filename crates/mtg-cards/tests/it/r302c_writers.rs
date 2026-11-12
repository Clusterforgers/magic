//! Goal: the writers pack r30-2c adds write the card's words and nothing near them: an exchange of two targets that share a card type, a life auction for control of a target, one exile per player paid with a cloak, a look at the top cards that plots one, a shield doubling a target's damage, a spell that can't be countered under a reveal or under X, kicked split second and a look at the top X cards; each near miss (another kind, number, duration or shape) reads as other text.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::compile::{same_written_text, static_pair_texts, written_variants};
use rstest::rstest;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

fn writes(json: &str, text: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|written| same_written_text(written, text))
}

fn exchange(base: &str, second: &str, duration: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":{base}}}}},{{"what":{{"Another":{{"what":{{"Object":{second}}},"earlier":[0]}}}}}}],"effects":[{{"ExchangeControl":{{"what":[{{"Target":0}},{{"Target":1}}],"duration":"{duration}"}}}}]}}}}"#
    )
}

const SHARES: &str = r#"{"SharesType":{"Target":0}}"#;
const NONLAND: &str = r#"{"Not":{"Type":"Land"}}"#;
const TWO: &str = "Exchange control of two target permanents that share a card type.";
const TWO_NONLAND: &str =
    "Exchange control of two target nonland permanents that share a card type.";

#[rstest]
#[case::permanents(exchange("[]", &format!("[{SHARES}]"), "Forever"), TWO, true)]
#[case::nonland(exchange(&format!("[{NONLAND}]"), &format!("[{NONLAND},{SHARES}]"), "Forever"), TWO_NONLAND, true)]
#[case::nonland_is_not_any(exchange(&format!("[{NONLAND}]"), &format!("[{NONLAND},{SHARES}]"), "Forever"), TWO, false)]
#[case::until_end_of_turn(exchange("[]", &format!("[{SHARES}]"), "UntilEndOfTurn"), TWO, false)]
#[case::a_color_is_not_a_type(
    exchange("[]", r#"[{"SharesColor":{"Target":0}}]"#, "Forever"),
    TWO,
    false
)]
#[case::bases_differ(exchange("[]", &format!("[{NONLAND},{SHARES}]"), "Forever"), TWO_NONLAND, false)]
fn r302c_two_targets_that_share_a_card_type(
    #[case] json: String,
    #[case] text: &str,
    #[case] reads: bool,
) {
    assert_eq!(writes(&json, text), reads);
}

fn auction(start: u32, what: &str, duration: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"{what}"}}]}}}}],"effects":[{{"BidLife":{{"start":{start},"as":"bid","winner":"bidder"}}}},{{"LoseLife":{{"who":{{"Bound":"bidder"}},"amount":{{"ChosenAs":"bid"}}}}}},{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"SetController":{{"Bound":"bidder"}}}}],"duration":"{duration}"}}}}]}}}}"#
    )
}

const AUCTION: &str = "Each player may bid life for control of target creature. You start the bidding with a bid of 0. In turn order, each player may top the high bid. The bidding ends if the high bid stands. The high bidder loses life equal to the high bid and gains control of the creature.";

#[rstest]
#[case::illicit_auction(auction(0, "Creature", "Forever"), true)]
#[case::another_opening(auction(1, "Creature", "Forever"), false)]
#[case::an_artifact(auction(0, "Artifact", "Forever"), false)]
#[case::for_a_turn(auction(0, "Creature", "UntilEndOfTurn"), false)]
fn r302c_a_life_auction_for_control(#[case] json: String, #[case] reads: bool) {
    assert_eq!(writes(&json, AUCTION), reads);
}

fn absence(what: &str, max: u32, cloaker: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"PerPlayer":{{"players":"EachPlayer","what":{what}}}}},"bounds":{{"min":0,"max":{max}}}}}],"effects":[{{"Exile":{{"what":{{"Target":0}},"as":"exiled"}}}},{{"RepeatEach":{{"over":{{"Bound":"exiled"}},"as":"card","effects":[{{"Cloak":{{"who":{cloaker}}}}}]}}}}]}}}}"#
    )
}

const ABSENCE: &str = "For each player, exile up to one target nonland permanent that player controls. For each permanent exiled this way, its controller cloaks the top card of their library.";
const ITS_CONTROLLER: &str = r#"{"ControllerOf":{"Departed":{"Bound":"card"}}}"#;

#[rstest]
#[case::unexplained_absence(absence(&format!("[{NONLAND}]"), 1, ITS_CONTROLLER), true)]
#[case::any_permanent(absence("[]", 1, ITS_CONTROLLER), false)]
#[case::up_to_two(absence(&format!("[{NONLAND}]"), 2, ITS_CONTROLLER), false)]
#[case::you_cloak(absence(&format!("[{NONLAND}]"), 1, r#""You""#), false)]
fn r302c_one_exile_per_player_and_a_cloak(#[case] json: String, #[case] reads: bool) {
    assert_eq!(writes(&json, ABSENCE), reads);
}

fn plot(count: u32, filter: &str, rest: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"LookAtTop":{{"count":{count},"as":"top"}}}},{{"Choose":{{"from":{{"Each":[{{"In":"Library"}},{{"Is":{{"Bound":"top"}}}},{filter}]}},"count":{{"min":0,"max":1}},"as":"kept"}}}},{{"Plot":{{"what":{{"Bound":"kept"}}}}}},{{"Move":{{"what":{{"Bound":"top"}},"to":"{rest}"}}}}]}}}}"#
    )
}

const LUCK: &str = "Look at the top three cards of your library. You may exile a nonland card from among them. If you do, it becomes plotted. Put the rest into your hand.";

#[rstest]
#[case::make_your_own_luck(plot(3, NONLAND, "Hand"), true)]
#[case::four_cards(plot(4, NONLAND, "Hand"), false)]
#[case::a_creature(plot(3, r#"{"Type":"Creature"}"#, "Hand"), false)]
#[case::rest_to_graveyard(plot(3, NONLAND, "Graveyard"), false)]
fn r302c_a_look_that_plots(#[case] json: String, #[case] reads: bool) {
    assert_eq!(writes(&json, LUCK), reads);
}

fn doubling(by: &str, duration: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[]}}}}],"effects":[{{"Shield":{{"when":{{"Damage":{{"source":[{{"Is":{{"Target":0}}}}]}}}},"by":{by},"duration":"{duration}"}}}}]}}}}"#
    )
}

const OVERBLAZE: &str = "Each time target permanent would deal damage to a permanent or player this turn, it deals double that damage to that permanent or player instead.";

#[rstest]
#[case::overblaze(doubling(r#"{"Multiply":2}"#, "UntilEndOfTurn"), true)]
#[case::triple(doubling(r#"{"Multiply":3}"#, "UntilEndOfTurn"), false)]
#[case::plus_two(doubling(r#"{"Add":2}"#, "UntilEndOfTurn"), false)]
#[case::forever(doubling(r#"{"Multiply":2}"#, "Forever"), false)]
fn r302c_a_target_deals_double(#[case] json: String, #[case] reads: bool) {
    assert_eq!(writes(&json, OVERBLAZE), reads);
}

const PRESENCE: &str = r#"{"Any":["AdditionalCostPaid",{"AsCast":{"Compare":{"left":{"Count":[{"Subtype":"Dragon"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}]}"#;
const PREROGATIVE: &str = "If you revealed a Dragon card or controlled a Dragon as you cast this spell, this spell can't be countered.";

#[rstest]
#[case::from_the_stack(&format!(r#"{{"Rule":{{"rule":{{"CantBeCountered":"This"}},"condition":{PRESENCE},"from":"Stack"}}}}"#), PREROGATIVE, true)]
#[case::from_the_battlefield(&format!(r#"{{"Rule":{{"rule":{{"CantBeCountered":"This"}},"condition":{PRESENCE}}}}}"#), PREROGATIVE, false)]
#[case::kicked_is_no_reveal(
    r#"{"Rule":{"rule":{"CantBeCountered":"This"},"condition":"Kicked","from":"Stack"}}"#,
    PREROGATIVE,
    false
)]
#[case::kicked_split_second(r#"{"Static":{"affected":{"Each":["This",{"In":"Stack"}]},"mods":[{"AddKeyword":"SplitSecond"}],"condition":"Kicked","from":"Stack"}}"#, "If this spell was kicked, it has split second.", true)]
#[case::kicked_flash_is_not_split_second(r#"{"Static":{"affected":{"Each":["This",{"In":"Stack"}]},"mods":[{"AddKeyword":"Flash"}],"condition":"Kicked","from":"Stack"}}"#, "If this spell was kicked, it has split second.", false)]
#[case::look_at_x(r#"{"Spell":{"effects":[{"LookAtTop":{"count":"X","as":"top"}},{"Choose":{"from":{"Bound":"top"},"as":"kept","rest_as":"rest"}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}},{"Move":{"what":{"Bound":"rest"},"to":{"Library":{"place":"Bottom","order":"Chosen"}}}}]}}"#, "Look at the top X cards of your library. Put one of them into your hand and the rest on the bottom of your library in any order.", true)]
#[case::look_at_x_is_not_three(r#"{"Spell":{"effects":[{"LookAtTop":{"count":"X","as":"top"}},{"Choose":{"from":{"Bound":"top"},"as":"kept","rest_as":"rest"}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}},{"Move":{"what":{"Bound":"rest"},"to":{"Library":{"place":"Bottom","order":"Chosen"}}}}]}}"#, "Look at the top three cards of your library. Put one of them into your hand and the rest on the bottom of your library in any order.", false)]
fn r302c_rules_of_the_spell_on_the_stack(
    #[case] json: &str,
    #[case] text: &str,
    #[case] reads: bool,
) {
    assert_eq!(writes(json, text), reads);
}

fn banefire_rules(least: u32, other: u32, combat: &str) -> (Ability, Ability) {
    (
        ability(&format!(
            r#"{{"Rule":{{"rule":{{"CantBeCountered":"This"}},"condition":{{"Compare":{{"left":"X","op":"Ge","right":{least}}}}},"from":"Stack"}}}}"#
        )),
        ability(&format!(
            r#"{{"Rule":{{"rule":{{"DamageCantBePrevented":{{"source":["This"]{combat}}}}},"condition":{{"Compare":{{"left":"X","op":"Ge","right":{other}}}}},"from":"Stack"}}}}"#
        )),
    )
}

const BANEFIRE: &str =
    "If X is 5 or more, this spell can't be countered and the damage can't be prevented.";

#[rstest]
#[case::banefire(5, 5, "", true)]
#[case::another_least(4, 4, "", false)]
#[case::two_conditions(5, 4, "", false)]
#[case::only_combat_damage(5, 5, r#","combat":true"#, false)]
fn r302c_banefire_rules_read_as_one_sentence(
    #[case] least: u32,
    #[case] other: u32,
    #[case] combat: &str,
    #[case] reads: bool,
) {
    let (first, second) = banefire_rules(least, other, combat);
    let found = static_pair_texts(&first, &second)
        .iter()
        .any(|text| same_written_text(text, BANEFIRE));
    assert_eq!(found, reads);
}
