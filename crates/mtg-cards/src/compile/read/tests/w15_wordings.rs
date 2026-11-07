//! Goal: the w15 wordings (a toll counted by "{X}, where X is the number of", "sacrifice any number", the other card of two looked at, the top card revealed and sorted, a pair of types counted, "any other" card in a graveyard, damage divided among up to X targets, "also deals") read as the text they mean, and a text of another meaning never reads as the clause.
use super::*;

#[rstest]
#[case::each_land_type(
    "Counter target spell unless its controller pays {1} for each basic land type among lands you control.",
    "Counter target spell unless its controller pays {X}, where X is the number of basic land types among lands you control.",
    true
)]
#[case::each_card(
    "Counter target spell unless its controller pays {1} for each card in your hand.",
    "Counter target spell unless its controller pays {X}, where X is the number of cards in your hand.",
    true
)]
#[case::each_party_member(
    "Counter target spell unless its controller pays {1} for each creature in your party.",
    "Counter target spell unless its controller pays {X}, where X is the number of creatures in your party.",
    true
)]
#[case::other_noun(
    "Counter target spell unless its controller pays {1} for each card in your hand.",
    "Counter target spell unless its controller pays {X}, where X is the number of lands in your hand.",
    false
)]
#[case::other_zone(
    "Counter target spell unless its controller pays {1} for each card in your hand.",
    "Counter target spell unless its controller pays {X}, where X is the number of cards in your graveyard.",
    false
)]
#[case::two_for_each(
    "Counter target spell unless its controller pays {2} for each card in your hand.",
    "Counter target spell unless its controller pays {X}, where X is the number of cards in your hand.",
    false
)]
#[case::plus_one(
    "Counter target spell unless its controller pays {1} for each card in your hand.",
    "Counter target spell unless its controller pays {X}, where X is 1 plus the number of cards in your hand.",
    false
)]
fn w15_toll_counted(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_text(left, right), same, "{right}");
}

#[rstest]
#[case::any_number(
    "As an additional cost to cast this spell, sacrifice any number of creatures.",
    "As an additional cost to cast ~, sacrifice X creatures.",
    true
)]
#[case::you_may(
    "As an additional cost to cast this spell, you may sacrifice any number of Spirits.",
    "As an additional cost to cast ~, sacrifice X Spirits.",
    true
)]
#[case::other_kind(
    "As an additional cost to cast this spell, sacrifice any number of creatures.",
    "As an additional cost to cast ~, sacrifice X lands.",
    false
)]
#[case::fixed_count(
    "As an additional cost to cast this spell, sacrifice any number of creatures.",
    "As an additional cost to cast ~, sacrifice two creatures.",
    false
)]
#[case::one(
    "As an additional cost to cast this spell, sacrifice a creature.",
    "As an additional cost to cast ~, sacrifice X creatures.",
    false
)]
#[case::optional_x(
    "As an additional cost to cast this spell, sacrifice any number of creatures.",
    "As an additional cost to cast ~, you may sacrifice X creatures.",
    false
)]
#[case::life(
    "As an additional cost to cast this spell, sacrifice any number of creatures.",
    "As an additional cost to cast ~, pay X life.",
    false
)]
fn w15_sacrifice_any_number(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_text(left, right), same, "{right}");
}

#[rstest]
#[case::the_other(
    "Look at the top two cards of your library. Put one of them into your hand and the other on the bottom of your library.",
    "Look at the top two cards of your library. Put one of them into your hand and the rest on the bottom of your library in any order.",
    true
)]
#[case::the_other_in_any_order(
    "Look at the top two cards of your library. Put one of them into your hand and the other on the bottom of your library in any order.",
    "Look at the top two cards of your library. Put one of them into your hand and the rest on the bottom of your library in any order.",
    true
)]
#[case::three_cards(
    "Look at the top three cards of your library. Put one of them into your hand and the other on the bottom of your library.",
    "Look at the top three cards of your library. Put one of them into your hand and the rest on the bottom of your library in any order.",
    false
)]
#[case::on_top(
    "Look at the top two cards of your library. Put one of them into your hand and the other on the bottom of your library.",
    "Look at the top two cards of your library. Put one of them into your hand and the rest on top of your library in any order.",
    false
)]
#[case::into_the_graveyard(
    "Look at the top two cards of your library. Put one of them into your hand and the other on the bottom of your library.",
    "Look at the top two cards of your library. Put one of them into your hand and the rest into your graveyard.",
    false
)]
fn w15_the_other_card(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_text(left, right), same, "{right}");
}

#[rstest]
#[case::battlefield(
    "{2}{G}{G}: Reveal the top card of your library. If it's a creature card, put it onto the battlefield. Otherwise, put it into your graveyard.",
    "{2}{G}{G}: Reveal the top card of your library. Then put all creature cards revealed this way onto the battlefield and the rest into your graveyard.",
    true
)]
#[case::hand_and_bottom(
    "+1: Reveal the top card of your library. If it's a creature card, put it into your hand. Otherwise, put it on the bottom of your library.",
    "+1: Reveal the top card of your library. Then put all creature cards revealed this way into your hand and the rest on the bottom of your library in any order.",
    true
)]
#[case::and_for_then(
    "When this creature enters, reveal the top card of your library. If it's an artifact card, put it into your hand. Otherwise, put it into your graveyard.",
    "When this creature enters, reveal the top card of your library and put all artifact cards revealed this way into your hand and the rest into your graveyard.",
    true
)]
#[case::other_kind(
    "{2}{G}{G}: Reveal the top card of your library. If it's a creature card, put it onto the battlefield. Otherwise, put it into your graveyard.",
    "{2}{G}{G}: Reveal the top card of your library. Then put all land cards revealed this way onto the battlefield and the rest into your graveyard.",
    false
)]
#[case::other_zone(
    "{2}{G}{G}: Reveal the top card of your library. If it's a creature card, put it onto the battlefield. Otherwise, put it into your graveyard.",
    "{2}{G}{G}: Reveal the top card of your library. Then put all creature cards revealed this way into your hand and the rest into your graveyard.",
    false
)]
#[case::other_rest(
    "{2}{G}{G}: Reveal the top card of your library. If it's a creature card, put it onto the battlefield. Otherwise, put it into your graveyard.",
    "{2}{G}{G}: Reveal the top card of your library. Then put all creature cards revealed this way onto the battlefield and the rest into your hand.",
    false
)]
#[case::two_cards(
    "Reveal the top two cards of your library. Put all creature cards revealed this way into your hand and the rest on the bottom of your library in any order.",
    "Reveal the top two cards of your library. Put all creature cards revealed this way into your hand and the rest on the bottom of your library.",
    false
)]
fn w15_reveal_the_top_card(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_text(left, right), same, "{right}");
}

#[rstest]
#[case::aura_and_equipment(
    "~ gets +2/+2 for each Aura and Equipment attached to it.",
    "~ gets +2/+2 for each Equipment and/or Aura attached to it.",
    true
)]
#[case::other_type(
    "~ gets +2/+2 for each Aura and Equipment attached to it.",
    "~ gets +2/+2 for each Equipment and/or Fortification attached to it.",
    false
)]
#[case::one_type(
    "~ gets +2/+2 for each Aura and Equipment attached to it.",
    "~ gets +2/+2 for each Aura attached to it.",
    false
)]
#[case::other_bonus(
    "~ gets +2/+2 for each Aura and Equipment attached to it.",
    "~ gets +1/+1 for each Equipment and/or Aura attached to it.",
    false
)]
fn w15_a_pair_of_types_counted(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_text(left, right), same, "{right}");
}

#[rstest]
#[case::in_a_graveyard(
    "You may have this creature enter as a copy of any creature card in a graveyard.",
    "You may have ~ enter as a copy of any other creature card in a graveyard.",
    true
)]
#[case::on_the_battlefield(
    "You may have this creature enter as a copy of any creature on the battlefield.",
    "You may have ~ enter as a copy of any other creature on the battlefield.",
    false
)]
#[case::other_kind(
    "You may have this creature enter as a copy of any creature card in a graveyard.",
    "You may have ~ enter as a copy of any other land card in a graveyard.",
    false
)]
fn w15_a_copy_of_any_other_card(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_text(left, right), same, "{right}");
}

#[rstest]
#[case::any_number(
    "~ deals X damage divided as you choose among any number of targets, where X is the number of Mountains you control.",
    "~ deals X damage divided as you choose among up to X targets, where X is the number of Mountains you control.",
    true
)]
#[case::fixed_damage(
    "~ deals 3 damage divided as you choose among any number of targets.",
    "~ deals 3 damage divided as you choose among up to X targets.",
    false
)]
#[case::up_to_two(
    "~ deals X damage divided as you choose among any number of targets.",
    "~ deals X damage divided as you choose among up to two targets.",
    false
)]
fn w15_damage_divided_among_up_to_x(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_text(left, right), same, "{right}");
}

#[rstest]
#[case::also(
    "~ deals 2 damage to target creature. If the gift was promised, ~ also deals 3 damage to that creature's controller.",
    "~ deals 2 damage to target creature. If the gift was promised, ~ deals 3 damage to that creature's controller.",
    true
)]
#[case::other_amount(
    "~ deals 2 damage to target creature. If the gift was promised, ~ also deals 3 damage to that creature's controller.",
    "~ deals 2 damage to target creature. If the gift was promised, ~ deals 4 damage to that creature's controller.",
    false
)]
fn w15_also_deals(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_text(left, right), same, "{right}");
}
