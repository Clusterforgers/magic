//! Goal: the w15 wordings (a toll counted by "{X}, where X is the number of", "sacrifice any number", the other card of two looked at, the top card revealed and sorted, a pair of types counted, damage divided among up to X targets, "also deals", names of quoted abilities, support, blight and adamant) read as the text they mean, and a text of another meaning never reads as the clause.
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
    assert_eq!(same_written_text(left, right), same, "{right}");
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
    assert_eq!(same_written_text(left, right), same, "{right}");
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
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::battlefield(
    "{2}{G}{G}: Reveal the top card of your library. If it's a creature card, put it onto the battlefield. Otherwise, put it into your graveyard.",
    "{2}{G}{G}: Reveal the top card of your library. Then put all creature cards revealed this way onto the battlefield and the rest into your graveyard.",
    true
)]
#[case::that_card_for_it(
    "{T}: Reveal the top card of your library. If it's a land card, put it onto the battlefield. Otherwise, put it into your hand.",
    "{T}: Reveal the top card of your library. If it's a land card, put it onto the battlefield. Otherwise, put that card into your hand.",
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
    assert_eq!(same_written_text(left, right), same, "{right}");
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
    assert_eq!(same_written_text(left, right), same, "{right}");
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
    assert_eq!(same_written_text(left, right), same, "{right}");
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
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::support(
    "At the beginning of combat on your turn, you may pay {3}{W}. If you do, support 2.",
    "At the beginning of combat on your turn, you may pay {3}{W}. If you do, put a +1/+1 counter on each of up to two target other creatures.",
    true
)]
#[case::support_three(
    "At the beginning of combat on your turn, you may pay {3}{W}. If you do, support 2.",
    "At the beginning of combat on your turn, you may pay {3}{W}. If you do, put a +1/+1 counter on each of up to three target other creatures.",
    false
)]
#[case::other_counter(
    "At the beginning of combat on your turn, you may pay {3}{W}. If you do, support 2.",
    "At the beginning of combat on your turn, you may pay {3}{W}. If you do, put a -1/-1 counter on each of up to two target other creatures.",
    false
)]
#[case::blight(
    "As an additional cost to cast this spell, blight 1 or pay {3}.",
    "As an additional cost to cast ~, put a -1/-1 counter on a creature you control or pay {3}.",
    true
)]
#[case::blight_two(
    "As an additional cost to cast this spell, blight 2 or pay {3}.",
    "As an additional cost to cast ~, put two -1/-1 counters on a creature you control or pay {3}.",
    true
)]
#[case::blight_for_one(
    "As an additional cost to cast this spell, blight 2 or pay {3}.",
    "As an additional cost to cast ~, put a -1/-1 counter on a creature you control or pay {3}.",
    false
)]
#[case::blight_of_another_kind(
    "As an additional cost to cast this spell, blight 1 or pay {3}.",
    "As an additional cost to cast ~, put a +1/+1 counter on a creature you control or pay {3}.",
    false
)]
fn w15_support_and_blight(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::adamant(
    "Adamant — If at least three white mana was spent to cast this spell, this creature enters with a +1/+1 counter on it.",
    "Adamant — ~ enters that has a +1/+1 counter on it if {W}{W}{W} was spent to cast it.",
    true
)]
#[case::other_color(
    "Adamant — If at least three white mana was spent to cast this spell, this creature enters with a +1/+1 counter on it.",
    "Adamant — ~ enters that has a +1/+1 counter on it if {R}{R}{R} was spent to cast it.",
    false
)]
#[case::other_counter(
    "Adamant — If at least three white mana was spent to cast this spell, this creature enters with a +1/+1 counter on it.",
    "Adamant — ~ enters that has two +1/+1 counters on it if {W}{W}{W} was spent to cast it.",
    false
)]
#[case::fewer_mana(
    "Adamant — If at least three white mana was spent to cast this spell, this creature enters with a +1/+1 counter on it.",
    "Adamant — ~ enters that has a +1/+1 counter on it if {W}{W} was spent to cast it.",
    false
)]
fn w15_adamant(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::sacrifice_it(
    "{1}{R}, {T}, Remove two pressure counters from this land and sacrifice it: Create a 4/4 red Hellion creature token with haste.",
    "{1}{R}, {T}, Remove two pressure counters from ~, Sacrifice it: Create a 4/4 red Hellion creature token with haste.",
    true
)]
#[case::sacrifice_a_land(
    "{1}{R}, {T}, Remove two pressure counters from this land and sacrifice it: Create a 4/4 red Hellion creature token with haste.",
    "{1}{R}, {T}, Remove two pressure counters from ~, Sacrifice a land: Create a 4/4 red Hellion creature token with haste.",
    false
)]
#[case::create_or_create(
    "When this creature enters, create a Food token or a Treasure token.",
    "When ~ enters, you create a Food token or create a Treasure token.",
    true
)]
#[case::create_and_create(
    "When this creature enters, create a Food token or a Treasure token.",
    "When ~ enters, you create a Food token and create a Treasure token.",
    false
)]
fn w15_small_words(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::lesser(
    "Whenever a nontoken creature you control dies, mill two cards. Then return a creature card with lesser mana value than the creature that died from your graveyard to your hand.",
    "Whenever a nontoken creature you control dies, mill two cards. Then return a creature card with mana value less than that creature's mana value from your graveyard to your hand.",
    true
)]
#[case::lesser_sacrificed(
    "Whenever you sacrifice a creature, return target creature card with lesser mana value from your graveyard to the battlefield.",
    "Whenever you sacrifice a creature, return target creature card with mana value less than that creature's mana value from your graveyard to the battlefield.",
    true
)]
#[case::greater(
    "Whenever you sacrifice a creature, return target creature card with lesser mana value from your graveyard to the battlefield.",
    "Whenever you sacrifice a creature, return target creature card with mana value greater than that creature's mana value from your graveyard to the battlefield.",
    false
)]
#[case::other_zone(
    "Whenever you sacrifice a creature, return target creature card with lesser mana value from your graveyard to the battlefield.",
    "Whenever you sacrifice a creature, return target creature card with mana value less than that creature's mana value from your hand to the battlefield.",
    false
)]
fn w15_lesser_mana_value(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::for_each(
    "Draw a card. Until end of turn, target creature gains trample and gets +1/+0 for each card you've drawn this turn.",
    "Draw a card. Until end of turn, target creature gains trample and gets +X/+0, where X is the number of cards you've drawn this turn.",
    true
)]
#[case::both(
    "~ gets +1/+1 for each creature you control.",
    "~ gets +X/+X, where X is the number of creatures you control.",
    true
)]
#[case::toughness(
    "~ gets +0/+1 for each creature you control.",
    "~ gets +0/+X, where X is the number of creatures you control.",
    true
)]
#[case::other_stat(
    "~ gets +1/+0 for each creature you control.",
    "~ gets +0/+X, where X is the number of creatures you control.",
    false
)]
#[case::other_pool(
    "~ gets +1/+0 for each creature you control.",
    "~ gets +X/+0, where X is the number of lands you control.",
    false
)]
#[case::two_for_each(
    "~ gets +2/+0 for each creature you control.",
    "~ gets +X/+0, where X is the number of creatures you control.",
    false
)]
fn w15_for_each_and_where_x(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::or_and_or(
    "When this creature enters, each opponent may put an artifact or enchantment card from their hand onto the battlefield.",
    "When ~ enters, each opponent may put an enchantment and/or artifact card from their hand onto the battlefield.",
    true
)]
#[case::in_the_order_of_the_names(
    "Put an Equipment or Aura card into your hand.",
    "Put an Aura or Equipment card into your hand.",
    true
)]
#[case::other_kind(
    "When this creature enters, each opponent may put an artifact or enchantment card from their hand onto the battlefield.",
    "When ~ enters, each opponent may put a land and/or artifact card from their hand onto the battlefield.",
    false
)]
#[case::one_kind(
    "When this creature enters, each opponent may put an artifact or enchantment card from their hand onto the battlefield.",
    "When ~ enters, each opponent may put an artifact card from their hand onto the battlefield.",
    false
)]
#[case::other_zone(
    "When this creature enters, each opponent may put an artifact or enchantment card from their hand onto the battlefield.",
    "When ~ enters, each opponent may put an enchantment and/or artifact card from their graveyard onto the battlefield.",
    false
)]
fn w15_either_kind(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::ticket("{G}, {T}: You get {TK}.", "{G}, {T}: You get a ticket counter.", true)]
#[case::two_tickets(
    "{G}, {T}: You get {TK}{TK}.",
    "{G}, {T}: You get a ticket counter.",
    false
)]
#[case::each_other_player(
    "When this creature enters, each other player may return a creature card from their graveyard to the battlefield.",
    "When ~ enters, each opponent may return a creature card from their graveyard to the battlefield.",
    true
)]
#[case::each_player(
    "When this creature enters, each player may return a creature card from their graveyard to the battlefield.",
    "When ~ enters, each opponent may return a creature card from their graveyard to the battlefield.",
    false
)]
#[case::combat_step(
    "At the beginning of your combat step, tap ~.",
    "At the beginning of combat on your turn, tap ~.",
    true
)]
#[case::each_combat(
    "At the beginning of each combat, tap ~.",
    "At the beginning of combat on your turn, tap ~.",
    false
)]
#[case::same_name(
    "Whenever a player casts a spell, that player gains X life, where X is the number of cards in all graveyards with the same name as that spell.",
    "Whenever a player casts a spell, that player gains X life, where X is the number of cards with the same name as that creature in all graveyards.",
    true
)]
#[case::same_name_in_a_hand(
    "Whenever a player casts a spell, that player gains X life, where X is the number of cards in all graveyards with the same name as that spell.",
    "Whenever a player casts a spell, that player gains X life, where X is the number of cards with the same name as that creature in your graveyard.",
    false
)]
#[case::plural_name("When ~ enter, draw a card.", "When ~ enters, draw a card.", true)]
fn w15_same_words(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::either_order(
    "As an additional cost to cast this spell, pay {4} or sacrifice an artifact or creature.",
    "As an additional cost to cast ~, sacrifice an artifact or creature or pay {4}.",
    true
)]
#[case::other_mana(
    "As an additional cost to cast this spell, pay {4} or sacrifice an artifact or creature.",
    "As an additional cost to cast ~, sacrifice an artifact or creature or pay {3}.",
    false
)]
#[case::other_kind(
    "As an additional cost to cast this spell, pay {4} or sacrifice an artifact or creature.",
    "As an additional cost to cast ~, sacrifice a land or pay {4}.",
    false
)]
#[case::and_for_or(
    "As an additional cost to cast this spell, pay {4} or sacrifice an artifact or creature.",
    "As an additional cost to cast ~, sacrifice an artifact or creature and pay {4}.",
    false
)]
fn w15_costs_in_either_order(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::blocks_a_permanent(
    "Whenever this creature blocks a creature, that creature doesn't untap during its controller's next untap step.",
    "Whenever ~ blocks a permanent, that creature doesn't untap during its controller's next untap step.",
    true
)]
#[case::blocks_a_land(
    "Whenever this creature blocks a creature, that creature doesn't untap during its controller's next untap step.",
    "Whenever ~ blocks a land, that creature doesn't untap during its controller's next untap step.",
    false
)]
#[case::blocks_or_attacks(
    "Whenever this creature blocks a creature, that creature doesn't untap during its controller's next untap step.",
    "Whenever ~ attacks a permanent, that creature doesn't untap during its controller's next untap step.",
    false
)]
fn w15_blocks_a_permanent(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::one_or_more(
    "When this creature dies, if there's a Lesson card in your graveyard, draw a card.",
    "When ~ dies, if there are 1 or more Lesson cards in your graveyard, draw a card.",
    true
)]
#[case::an_aura(
    "~ gets +1/+1 as long as there's an Aura card in your graveyard.",
    "~ gets +1/+1 as long as there are 1 or more Aura cards in your graveyard.",
    true
)]
#[case::two_or_more(
    "~ gets +1/+1 as long as there's a Lesson card in your graveyard.",
    "~ gets +1/+1 as long as there are 2 or more Lesson cards in your graveyard.",
    false
)]
#[case::other_zone(
    "~ gets +1/+1 as long as there's a Lesson card in your graveyard.",
    "~ gets +1/+1 as long as there are 1 or more Lesson cards in your hand.",
    false
)]
#[case::other_kind(
    "~ gets +1/+1 as long as there's a Lesson card in your graveyard.",
    "~ gets +1/+1 as long as there are 1 or more Aura cards in your graveyard.",
    false
)]
fn w15_one_or_more(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::may_reveal(
    "When this creature enters, look at the top four cards of your library. You may reveal a noncreature, nonland card from among them and put it into your hand. Put the rest on the bottom of your library in a random order.",
    "When ~ enters, look at the top four cards of your library and may reveal a noncreature nonland card from among them and put it into your hand. Then put the rest of those cards on the bottom in a random order.",
    true
)]
#[case::other_count(
    "When this creature enters, look at the top four cards of your library. You may reveal a noncreature, nonland card from among them and put it into your hand. Put the rest on the bottom of your library in a random order.",
    "When ~ enters, look at the top five cards of your library and may reveal a noncreature nonland card from among them and put it into your hand. Then put the rest of those cards on the bottom in a random order.",
    false
)]
#[case::other_kind(
    "When this creature enters, look at the top four cards of your library. You may reveal a noncreature, nonland card from among them and put it into your hand. Put the rest on the bottom of your library in a random order.",
    "When ~ enters, look at the top four cards of your library and may reveal a noncreature card from among them and put it into your hand. Then put the rest of those cards on the bottom in a random order.",
    false
)]
#[case::rest_on_top(
    "When this creature enters, look at the top four cards of your library. You may reveal a noncreature, nonland card from among them and put it into your hand. Put the rest on the bottom of your library in a random order.",
    "When ~ enters, look at the top four cards of your library and may reveal a noncreature nonland card from among them and put it into your hand. Then put the rest of those cards on top in a random order.",
    false
)]
#[case::forced(
    "When this creature enters, look at the top four cards of your library. You may reveal a noncreature, nonland card from among them and put it into your hand. Put the rest on the bottom of your library in a random order.",
    "When ~ enters, look at the top four cards of your library. Reveal a noncreature nonland card from among them and put it into your hand. Then put the rest of those cards on the bottom in a random order.",
    false
)]
fn w15_look_and_may_reveal(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::mode_name_is_printed_text(
    "Choose one — / • Two-Weapon Fighting — Double target creature's power and toughness until end of turn. / • Archery — This spell deals 5 damage to target creature with flying.",
    "Choose one — / • Double target creature's power and toughness until end of turn. / • ~ deals 5 damage to target creature with flying.",
    false
)]
#[case::a_mode_the_data_leaves_unnamed(
    "Choose one — / • Two-Weapon Fighting — Double target creature's power and toughness until end of turn. / • This spell deals 5 damage to target creature with flying.",
    "Choose one — / • Double target creature's power and toughness until end of turn. / • ~ deals 5 damage to target creature with flying.",
    false
)]
#[case::quoted_ability_word(
    "II — This Saga gains \"Landfall — Whenever a land you control enters, create a 1/1 green Elf creature token.\"",
    "II — ~ gains \"Whenever a land you control enters, create a 1/1 green Elf creature token.\"",
    true
)]
#[case::other_effect_in_a_mode(
    "Choose one — / • Archery — This spell deals 5 damage to target creature with flying.",
    "Choose one — / • ~ deals 4 damage to target creature with flying.",
    false
)]
#[case::other_event_in_a_quote(
    "II — This Saga gains \"Landfall — Whenever a land you control enters, create a 1/1 green Elf creature token.\"",
    "II — ~ gains \"Whenever a creature you control enters, create a 1/1 green Elf creature token.\"",
    false
)]
#[case::a_clause_is_not_a_name(
    "• Destroy target artifact. — Draw a card.",
    "• Draw a card.",
    false
)]
fn w15_names_of_quoted_abilities(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}
