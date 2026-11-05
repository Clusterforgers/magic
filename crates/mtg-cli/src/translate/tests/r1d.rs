//! Goal: round 1 pack D reads the leftover parameters of the zone verbs into the rules the engine runs, and keeps what it cannot say a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn sorcery(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line} | SpellDescription$ x\n")
}

/// The effects of the first ability of a sorcery made of the line.
fn effects(line: &str) -> Value {
    let text = sorcery(line);
    assert_eq!(gaps(&text), Vec::<String>::new(), "{line}");
    abilities(&text)[0]["Spell"]["effects"].clone()
}

#[rstest]
#[case::mana_value_limit(
    "Play | Valid$ Card.nonLand+YouOwn | ValidSA$ Spell.cmcLE4 | ValidZone$ Hand | WithoutManaCost$ True | Optional$ True",
    json!([{"In": "Hand"}])
)]
#[case::two_types(
    "Play | Valid$ Card.YouOwn | ValidSA$ Instant.cmcLE3,Sorcery.cmcLE3 | ValidZone$ Hand | WithoutManaCost$ True | Optional$ True",
    json!([{"In": "Hand"}])
)]
fn a_play_limited_by_the_spell_is_a_cast_of_cards_that_match(
    #[case] line: &str,
    #[case] has: Value,
) {
    let found = effects(line).to_string();
    for pred in has.as_array().unwrap() {
        assert!(found.contains(&pred.to_string()), "{found}");
    }
    assert!(
        found.contains("CastFromZone") && !found.contains("\"play\""),
        "{found}"
    );
}

#[rstest]
#[case::activated(
    "Play | Valid$ Card.YouOwn | ValidSA$ Activated | ValidZone$ Hand | Optional$ True"
)]
fn a_play_limited_by_another_kind_stays_a_gap(#[case] line: &str) {
    let text = sorcery(line);
    assert!(gaps(&text).contains(&"key:Play.ValidSA".to_owned()));
}

/// A sorcery of the line with the extra script lines (`SVar:` definitions) after it.
fn with_lines(line: &str, extra: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line} | SpellDescription$ x\n{extra}\n")
}

#[rstest]
#[case::each_hand_whole(
    "Discard | Mode$ Hand | Defined$ Player",
    "",
    &["\"who\":\"EachPlayer\"", "\"Count\":[{\"In\":\"Hand\"},{\"OwnedBy\":\"EachPlayer\"}]"]
)]
#[case::unless_a_type(
    "Discard | Defined$ You | NumCards$ 2 | Mode$ TgtChoose | UnlessType$ Creature",
    "",
    &["\"Pay\"", "\"unpaid\":[{\"Discard\":{\"count\":2", "\"cost\":[{\"Discard\":{\"filter\""]
)]
#[case::look_and_choose(
    "Discard | ValidTgts$ Opponent | Mode$ LookYouChoose | Optional$ True | NumCards$ 2",
    "",
    &["\"by\":\"You\"", "\"optional\":true"]
)]
#[case::reveal_and_discard_all(
    "Discard | ValidTgts$ Player | Mode$ RevealDiscardAll | DiscardValid$ Card.nonLand",
    "",
    &["\"Reveal\"", "\"Count\":[{\"In\":\"Hand\"}"]
)]
#[case::sacrifice_all_you_control(
    "SacrificeAll | ValidCards$ Creature.YouCtrl",
    "",
    &["\"Sacrifice\"", "\"count\":{\"Count\":[{\"Type\":\"Creature\"}"]
)]
#[case::sacrifice_all_by_every_player(
    "SacrificeAll | ValidCards$ Permanent.nonLand",
    "",
    &["\"who\":\"EachPlayer\""]
)]
#[case::move_all_under_your_control(
    "ChangeZoneAll | ChangeType$ Creature | Origin$ Graveyard | Destination$ Battlefield | GainControl$ True",
    "",
    &["\"under\":\"You\""]
)]
#[case::dig_a_counted_number(
    "Dig | DigNum$ 3 | ChangeNum$ X | NoReveal$ True",
    "SVar:X:Count$xPaid",
    &["\"max\":\"X\"", "\"min\":\"X\""]
)]
#[case::each_player_reveals_the_top(
    "PeekAndReveal | Defined$ Player | PeekAmount$ 1",
    "",
    &["\"who\":\"EachPlayer\""]
)]
#[case::reveal_a_card_at_random(
    "Reveal | Random$ True | Defined$ You",
    "",
    &["\"ChooseAtRandom\"", "\"In\":\"Hand\""]
)]
#[case::reveal_up_to_five(
    "Reveal | NumCards$ 5 | Optional$ True | RevealValid$ Card.nonLand+YouOwn",
    "",
    &["\"min\":0", "\"max\":5"]
)]
#[case::copy_of_a_named_card(
    "ChangeZone | ValidTgts$ Card.nonCreature | Origin$ Graveyard | Destination$ Exile | RememberChanged$ True | SubAbility$ DBPlay",
    "SVar:DBPlay:DB$ Play | Defined$ Remembered | CopyCard$ True | Controller$ You | WithoutManaCost$ True | Optional$ True",
    &["\"CopyCard\"", "\"free\":true"]
)]
fn the_zone_verbs_read_their_leftover_parameters(
    #[case] line: &str,
    #[case] extra: &str,
    #[case] has: &[&str],
) {
    let text = with_lines(line, extra);
    let found = abilities(&text)[0].to_string();
    assert_eq!(gaps(&text), Vec::<String>::new(), "{found}");
    for part in has {
        assert!(found.contains(part), "{part} missing from {found}");
    }
}

#[rstest]
#[case::group_and_a_pick(
    "PeekAndReveal | Defined$ Player | RevealValid$ Card.Land",
    "key:PeekAndReveal.Defined+RevealValid"
)]
#[case::random_among_every_hand(
    "Reveal | Random$ True | Defined$ Player",
    "key:Reveal.Random+group"
)]
#[case::unless_with_the_whole_hand(
    "Discard | Mode$ Hand | Defined$ You | UnlessType$ Land",
    "key:Discard.UnlessType+shape"
)]
#[case::sacrifice_all_with_a_controller(
    "SacrificeAll | ValidCards$ Creature | Controller$ You",
    "key:SacrificeAll.Controller"
)]
fn what_the_new_forms_cannot_say_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert!(gaps(&with_lines(line, "")).contains(&gap.to_owned()));
}

#[test]
fn a_creature_that_returns_as_an_enchantment_has_only_that_card_type() {
    let text = "Name:T\nTypes:Enchantment\nT:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self | Execute$ TrigReturn | TriggerDescription$ x\n\
SVar:TrigReturn:DB$ ChangeZone | Defined$ TriggeredNewCardLKICopy | Origin$ Graveyard | Destination$ Battlefield | StaticEffect$ Animate\n\
SVar:Animate:Mode$ Continuous | Affected$ Card.IsRemembered | AddType$ Enchantment | RemoveCardTypes$ True\n";
    let found = abilities(text)[0].to_string();
    assert!(found.contains("\"SetTypes\":[\"Enchantment\"]"), "{found}");
    assert!(!found.contains("\"AddType\""), "{found}");
}
