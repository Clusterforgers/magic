//! Goal: a move's own sentence decides whether it may take none, a printed "reveal the top" reveals, a printed "random
//! order" orders the rest at random, a spell that opens with exiling itself does so first, and cards put back from the
//! hand are ordered by their player; every near-miss keeps its old reading.
use rstest::rstest;
use serde_json::Value;

use super::{abilities, gaps};

fn read(text: &str) -> String {
    assert!(gaps(text).is_empty(), "{:?}", gaps(text));
    Value::Array(abilities(text)).to_string()
}

fn spell(line: &str, oracle: &str) -> String {
    read(&format!(
        "Name:T\nManaCost:B\nTypes:Sorcery\nA:SP$ {line}\nOracle:{oracle}\n"
    ))
}

const TWILIGHT: &str = "Pump | ValidTgts$ Creature | TargetMin$ 0 | TargetMax$ 1 | NumAtt$ -2 | NumDef$ -2 | IsCurse$ True | SubAbility$ DBReturn | SpellDescription$ ";
const RETURN: &str = "\nSVar:DBReturn:DB$ ChangeZone | Origin$ Graveyard | Destination$ Hand | ChangeNum$ 1 | Hidden$ True | ChangeType$ Creature.YouOwn";

#[rstest]
#[case::up_to_of_another_sentence(
    "Up to one target creature gets -2/-2 until end of turn. Return a creature card from your graveyard to your hand.",
    "",
    false
)]
#[case::may_of_its_own(
    "Up to one target creature gets -2/-2 until end of turn. You may return a creature card from your graveyard to your hand.",
    "",
    true
)]
#[case::optional_without_may(
    "Target creature gets -2/-2 until end of turn. Then return a creature card from your graveyard to your hand. (Reminder: a may here is not the card's.)",
    " | Optional$ True",
    false
)]
#[case::optional_with_may(
    "Target creature gets -2/-2 until end of turn. Then you may return a creature card from your graveyard to your hand.",
    " | Optional$ True",
    true
)]
#[case::zone_named_twice(
    "Up to one target creature gets -2/-2 until end of turn. Return a creature card from your graveyard to your hand. Then exile a card from your graveyard.",
    " | Optional$ True",
    true
)]
#[case::zone_named_by_a_look(
    "Target creature gets -2/-2 until end of turn. Look at your graveyard. You may return a creature card from it to your hand.",
    "",
    true
)]
fn fix20_a_move_may_take_none_only_by_its_own_sentence(
    #[case] text: &str,
    #[case] optional: &str,
    #[case] none: bool,
) {
    let out = spell(&format!("{TWILIGHT}{text}{RETURN}{optional}"), text);
    assert_eq!(out.contains(r#""count":{"max":1,"min":0}"#), none, "{out}");
}

fn dig(extra: &str, said: &str) -> String {
    read(&format!(
        "Name:T\nManaCost:G\nTypes:Creature Elf\nPT:1/1\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigDig | TriggerDescription$ When CARDNAME enters, {said}\nSVar:TrigDig:DB$ Dig | DigNum$ 5 | ChangeNum$ 1 | ChangeValid$ Land | DestinationZone$ Hand{extra}\nOracle:When this creature enters, {said}\n"
    ))
}

#[rstest]
#[case::reveal_the_top(
    "reveal the top five cards of your library. Put a land card from among them into your hand and the rest on the bottom of your library in any order.",
    true
)]
#[case::look_at_the_top(
    "look at the top five cards of your library. Put a land card from among them into your hand and the rest on the bottom of your library in any order.",
    false
)]
#[case::may_reveal_the_top(
    "you may reveal the top five cards of your library. Put a land card from among them into your hand and the rest on the bottom of your library in any order.",
    false
)]
fn fix20_a_printed_reveal_of_the_top_reveals_them(#[case] said: &str, #[case] reveals: bool) {
    let out = dig("", said);
    assert_eq!(
        out.contains(r#"{"Reveal":{"what":{"Bound":"top"}}}"#),
        reveals,
        "{out}"
    );
}

#[rstest]
#[case::random_printed("in a random order.", "", "Random")]
#[case::any_printed("in any order.", "", "Chosen")]
#[case::random_scripted("in any order.", " | RestRandomOrder$ True", "Random")]
fn fix20_a_printed_random_order_orders_the_rest_at_random(
    #[case] order: &str,
    #[case] extra: &str,
    #[case] expected: &str,
) {
    let out = dig(
        extra,
        &format!(
            "look at the top five cards of your library. Put a land card from among them into your hand and the rest on the bottom of your library {order}"
        ),
    );
    assert!(out.contains(&format!(r#""order":"{expected}""#)), "{out}");
}

#[rstest]
#[case::opens_with_exile("Exile CARDNAME. Each player discards their hand.", true)]
#[case::exile_last("Each player discards their hand. Exile CARDNAME.", false)]
fn fix20_a_spell_that_opens_with_exiling_itself_does_so_first(
    #[case] said: &str,
    #[case] first: bool,
) {
    let out = spell(
        &format!(
            "Discard | Mode$ Hand | Defined$ Player | SubAbility$ DBExile | SpellDescription$ {said}\nSVar:DBExile:DB$ ChangeZone | Origin$ Stack | Destination$ Exile"
        ),
        &said.replace("CARDNAME", "T"),
    );
    assert_eq!(
        out.starts_with(r#"[{"Spell":{"effects":[{"Exile""#),
        first,
        "{out}"
    );
}

#[rstest]
#[case::two_cards(
    "ChangeNum$ 2 | Mandatory$ True | Reorder$ True",
    "put two cards from your hand on top of your library in any order.",
    true
)]
#[case::one_card(
    "ChangeNum$ 1 | Mandatory$ True",
    "put a card from your hand on top of your library.",
    false
)]
#[case::another_player(
    "ChangeNum$ 2 | Mandatory$ True | Reorder$ True | DefinedPlayer$ Targeted",
    "target player puts two cards from their hand on top of their library in any order.",
    false
)]
fn fix20_cards_put_back_from_the_hand_are_ordered_by_their_player(
    #[case] keys: &str,
    #[case] said: &str,
    #[case] chosen: bool,
) {
    let target = if keys.contains("Targeted") {
        " | ValidTgts$ Player"
    } else {
        ""
    };
    let out = spell(
        &format!(
            "Draw | NumCards$ 3{target} | SubAbility$ DBBack | SpellDescription$ Draw three cards, then {said}\nSVar:DBBack:DB$ ChangeZone | Origin$ Hand | Destination$ Library | {keys}"
        ),
        said,
    );
    assert_eq!(out.contains(r#""order":"Chosen""#), chosen, "{out}");
}
