//! Goal: the w17 wordings (subject "you" after a trigger, verb number, "each X" and "all Xs", colors before or after a noun, one verb for two things, costs and exclusions in any order, small forms, qualifiers, counts and conditions in any place) read as the text they mean, and a text of another meaning never reads as the clause.
use super::*;

#[rstest]
#[case::you_after_trigger(
    "Whenever a creature you control dies, create a Treasure token.",
    "Whenever a creature you control dies, you create a Treasure token.",
    true
)]
#[case::you_after_and(
    "Put a +1/+1 counter on ~ and create a Soldier token.",
    "Put a +1/+1 counter on ~ and you create a Soldier token.",
    true
)]
#[case::other_player_draws(
    "Whenever ~ attacks, target player draws a card.",
    "Whenever ~ attacks, you draw a card.",
    false
)]
#[case::you_in_the_middle(
    "Target player discards a card and you draw a card.",
    "Target player discards a card and draw a card.",
    false
)]
#[case::verb_number(
    "Creatures you control have flying.",
    "Creatures you control has flying.",
    true
)]
#[case::verb_number_other_keyword(
    "Creatures you control have flying.",
    "Creatures you control have trample.",
    false
)]
#[case::may_have_is_a_choice(
    "When ~ attacks, you may have it deal 1 damage to any target.",
    "When ~ attacks, it deals 1 damage to any target.",
    false
)]
#[case::all_and_each(
    "All nonland permanents are the chosen color.",
    "Each nonland permanent is the chosen color.",
    true
)]
#[case::all_and_each_object("Destroy all creatures.", "Destroy each creature.", true)]
#[case::each_other_kind("Destroy all creatures.", "Destroy each land.", false)]
#[case::each_one_object("Destroy all creatures.", "Destroy target creature.", false)]
fn w17_subjects_and_numbers(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::colors_before(
    "Counter target red or green spell.",
    "Counter target spell that's red or green.",
    true
)]
#[case::one_color(
    "Exile target black permanent.",
    "Exile target permanent that's black.",
    true
)]
#[case::other_color(
    "Counter target red or green spell.",
    "Counter target spell that's red or blue.",
    false
)]
#[case::no_color("Counter target red spell.", "Counter target spell.", false)]
#[case::pump_adds(
    "Max speed \u{2014} This creature gets +1/+0.",
    "Max speed \u{2014} ~ gets an additional +1/+0.",
    true
)]
#[case::pump_other_value("This creature gets +1/+0.", "~ gets an additional +1/+1.", false)]
#[case::max_speed_is_a_condition(
    "Max speed \u{2014} This creature has lifelink.",
    "~ has lifelink.",
    false
)]
#[case::mana_adds(
    "Whenever you tap a land for mana, add {G}.",
    "Whenever you tap a land for mana, add an additional {G}.",
    true
)]
#[case::zero_sign(
    "Target creature gets -0/-9999 until end of turn.",
    "Target creature gets +0/-9999 until end of turn.",
    true
)]
#[case::zero_sign_other_value(
    "Target creature gets -0/-9999 until end of turn.",
    "Target creature gets +0/-999 until end of turn.",
    false
)]
#[case::it_is(
    "If it's a creature, draw a card.",
    "If it is a creature, draw a card.",
    true
)]
#[case::it_is_a_land(
    "If it's a creature, draw a card.",
    "If it is a land, draw a card.",
    false
)]
#[case::one_of_your_opponents(
    "Whenever ~ deals combat damage to 1 of your opponents, draw a card.",
    "Whenever ~ deals combat damage to an opponent, draw a card.",
    true
)]
#[case::one_of_your_opponents_player(
    "Whenever ~ deals combat damage to 1 of your opponents, draw a card.",
    "Whenever ~ deals combat damage to a player, draw a card.",
    false
)]
#[case::owners_hands(
    "Return all nonland permanents to their owners' hands.",
    "Return all nonland permanents to their owner's hand.",
    true
)]
#[case::owners_library(
    "Return all nonland permanents to their owners' hands.",
    "Return all nonland permanents to their owner's library.",
    false
)]
fn w17_colors_and_small_forms(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::exile_two(
    "Exile target creature and target land.",
    "Exile target creature, then exile target land.",
    true
)]
#[case::exile_two_sentences(
    "Put a +1/+1 counter on target creature and a +1/+1 counter on up to one target Merfolk.",
    "Put a +1/+1 counter on target creature. Put a +1/+1 counter on up to one target Merfolk.",
    true
)]
#[case::exile_then_destroy(
    "Exile target creature and target land.",
    "Exile target creature, then destroy target land.",
    false
)]
#[case::exile_one(
    "Exile target creature and target land.",
    "Exile target creature.",
    false
)]
#[case::shared_zone(
    "Return up to one target instant card and up to one target sorcery card from your graveyard to your hand.",
    "Return up to one target instant card from your graveyard to your hand and return up to one target sorcery card from your graveyard to your hand.",
    true
)]
#[case::other_zone(
    "Return up to one target instant card and up to one target sorcery card from your graveyard to your hand.",
    "Return up to one target instant card from your graveyard to your hand and return up to one target sorcery card from your library to your hand.",
    false
)]
#[case::cost_order(
    "{T}, Sacrifice this artifact: Draw a card.",
    "Sacrifice ~, {T}: Draw a card.",
    true
)]
#[case::cost_missing(
    "{T}, Sacrifice this artifact: Draw a card.",
    "Sacrifice ~: Draw a card.",
    false
)]
#[case::sacrifice_list(
    "{2}, Sacrifice a green creature, a white creature, and a blue creature: Draw a card.",
    "{2}, Sacrifice a green creature and a white creature and a blue creature: Draw a card.",
    true
)]
#[case::sacrifice_other_list(
    "{2}, Sacrifice a green creature, a white creature, and a blue creature: Draw a card.",
    "{2}, Sacrifice a green creature and a white creature and a red creature: Draw a card.",
    false
)]
#[case::sacrifice_in_any_order(
    "{T}, Sacrifice two lands and this artifact: Destroy target attacking creature.",
    "{T}, Sacrifice ~ and two lands: Destroy target attacking creature.",
    true
)]
#[case::exclusions(
    "Gain control of target nonartifact, nonblack creature.",
    "Gain control of target nonblack, nonartifact creature.",
    true
)]
#[case::other_exclusions(
    "Gain control of target nonartifact, nonblack creature.",
    "Gain control of target nonblack, nonwhite creature.",
    false
)]
#[case::exclusion_choices(
    "Exile target noncreature artifact or noncreature enchantment.",
    "Exile target noncreature enchantment or noncreature artifact.",
    true
)]
#[case::other_exclusion_choice(
    "Exile target noncreature artifact or noncreature enchantment.",
    "Exile target noncreature artifact or noncreature land.",
    false
)]
#[case::adjectives(
    "Tap an untapped nontoken artifact you control: Add {G}.",
    "Tap a nontoken untapped artifact you control: Add {G}.",
    true
)]
#[case::adjective_dropped(
    "Tap an untapped nontoken artifact you control: Add {G}.",
    "Tap a nontoken artifact you control: Add {G}.",
    false
)]
fn w17_two_things_and_orders(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::unless_spent(
    "When this creature enters, sacrifice it unless {W} was spent to cast it.",
    "When ~ enters, if {W} wasn't spent to cast her, sacrifice it.",
    true
)]
#[case::unless_other_mana(
    "When this creature enters, sacrifice it unless {W} was spent to cast it.",
    "When ~ enters, if {U} wasn't spent to cast her, sacrifice it.",
    false
)]
#[case::if_spent(
    "When this creature enters, sacrifice it unless {W} was spent to cast it.",
    "When ~ enters, if {W} was spent to cast her, sacrifice it.",
    false
)]
#[case::attach_dread(
    "When this Equipment enters, manifest dread, then attach this Equipment to that creature.",
    "When ~ enters, manifest dread. Then attach it to that creature.",
    true
)]
#[case::attach_dread_missing(
    "When this Equipment enters, manifest dread, then attach this Equipment to that creature.",
    "When ~ enters, manifest dread.",
    false
)]
#[case::damage_twice(
    "~ deals 1 damage to target creature and 1 damage to that creature's controller.",
    "~ deals 1 damage to target creature. ~ deals 1 damage to that creature's controller.",
    true
)]
#[case::damage_twice_other_number(
    "~ deals 1 damage to target creature and 1 damage to that creature's controller.",
    "~ deals 1 damage to target creature. ~ deals 2 damage to that creature's controller.",
    false
)]
#[case::then_the_card(
    "Counter target spell and ~ deals 3 damage to target creature.",
    "Counter target spell, then ~ deals 3 damage to target creature.",
    true
)]
#[case::then_the_card_other_damage(
    "Counter target spell and ~ deals 3 damage to target creature.",
    "Counter target spell, then ~ deals 4 damage to target creature.",
    false
)]
#[case::then_it_cant(
    "Target creature gets +1/+0 and can't be blocked this turn.",
    "Target creature gets +1/+0. Then it can't be blocked this turn.",
    true
)]
#[case::then_it_cant_block(
    "Target creature gets +1/+0 and can't be blocked this turn.",
    "Target creature gets +1/+0. Then it can't block this turn.",
    false
)]
#[case::it_is_the_card(
    "As long as you've put one or more +1/+1 counters on ~ this turn, it has flying.",
    "As long as you've put one or more +1/+1 counters on ~ this turn, ~ has flying.",
    true
)]
#[case::it_is_the_card_other_ability(
    "As long as you've put one or more +1/+1 counters on ~ this turn, it has flying.",
    "As long as you've put one or more +1/+1 counters on ~ this turn, ~ has haste.",
    false
)]
#[case::may_each(
    "When ~ enters, you may have Ally creatures you control gain first strike until end of turn.",
    "When ~ enters, you may each Ally you control gains first strike until end of turn.",
    true
)]
#[case::may_each_no_choice(
    "When ~ enters, you may have Ally creatures you control gain first strike until end of turn.",
    "When ~ enters, each Ally you control gains first strike until end of turn.",
    false
)]
#[case::search_then_put(
    "Search your library for a card named Godsire, put it onto the battlefield, then shuffle.",
    "Search your library for a card named Godsire. Put it onto the battlefield. Then shuffle.",
    true
)]
#[case::search_without_put(
    "Search your library for a card named Godsire, put it onto the battlefield, then shuffle.",
    "Search your library for a card named Godsire. Then shuffle.",
    false
)]
fn w17_conditions_and_sequences(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::colored_spells(
    "Red spells and white spells you cast cost {1} less to cast.",
    "White spells and red spells you cast cost {1} less to cast.",
    true
)]
#[case::other_colored_spells(
    "Red spells and white spells you cast cost {1} less to cast.",
    "White spells and blue spells you cast cost {1} less to cast.",
    false
)]
#[case::keywords(
    "Target creature gains reach and first strike until end of turn.",
    "Target creature gains first strike and reach until end of turn.",
    true
)]
#[case::other_keywords(
    "Target creature gains reach and first strike until end of turn.",
    "Target creature gains double strike and reach until end of turn.",
    false
)]
#[case::count_last(
    "Target creature gets +1/+1 for each creature you control and gains trample until end of turn.",
    "Target creature gets +1/+1 and gains trample for each creature you control until end of turn.",
    true
)]
#[case::count_of_another_kind(
    "Target creature gets +1/+1 for each creature you control and gains trample until end of turn.",
    "Target creature gets +1/+1 and gains trample for each land you control until end of turn.",
    false
)]
#[case::condition_last(
    "As long as you control a Demon, this creature has flying.",
    "This creature has flying as long as you control a Demon.",
    true
)]
#[case::condition_of_another_kind(
    "As long as you control a Demon, this creature has flying.",
    "This creature has flying as long as you control an Angel.",
    false
)]
#[case::qualifiers(
    "Creatures of the chosen color without flying can't attack you.",
    "Creatures without flying of the chosen color can't attack you.",
    true
)]
#[case::other_qualifier(
    "Creatures of the chosen color without flying can't attack you.",
    "Creatures with flying of the chosen color can't attack you.",
    false
)]
#[case::there_is(
    "At the beginning of your upkeep, if twenty or more creature cards are in your graveyard, you win the game.",
    "At the beginning of your upkeep, if there is twenty or more creature cards in your graveyard, you win the game.",
    true
)]
#[case::there_is_other_zone(
    "At the beginning of your upkeep, if twenty or more creature cards are in your graveyard, you win the game.",
    "At the beginning of your upkeep, if there is twenty or more creature cards in your hand, you win the game.",
    false
)]
#[case::comma_list(
    "Enchanted creature gets +2/+2, has vigilance, and can block an additional creature each combat.",
    "Enchanted creature gets +2/+2 and has vigilance and can block an additional creature each combat.",
    true
)]
#[case::comma_list_choice(
    "Enchanted creature gets +2/+2, has vigilance, and can block an additional creature each combat.",
    "Enchanted creature gets +2/+2 or has vigilance or can block an additional creature each combat.",
    false
)]
#[case::up_to_list(
    "Destroy up to one target artifact, up to one target creature, and up to one target land.",
    "Destroy up to one target artifact and up to one target creature and up to one target land.",
    true
)]
#[case::each_of(
    "Untap all creatures you control. Each of those creatures gets +1/+1 until end of turn.",
    "Untap all creatures you control. Those creatures get +1/+1 until end of turn.",
    true
)]
#[case::each_of_other_pump(
    "Untap all creatures you control. Each of those creatures gets +1/+1 until end of turn.",
    "Untap all creatures you control. Those creatures get +2/+2 until end of turn.",
    false
)]
fn w17_lists_and_places(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::another(
    "Sacrifice a creature other than this creature.",
    "Sacrifice another creature.",
    true
)]
#[case::another_by_name_last(
    "Sacrifice a creature other than ~.",
    "Sacrifice another creature.",
    true
)]
#[case::another_dropped(
    "Sacrifice a creature other than this creature.",
    "Sacrifice a creature.",
    false
)]
#[case::target_other(
    "Return up to one target nonland permanent other than this creature to its owner's hand.",
    "Return up to one target other nonland permanent to its owner's hand.",
    true
)]
#[case::target_other_dropped(
    "Return up to one target nonland permanent other than this creature to its owner's hand.",
    "Return up to one target nonland permanent to its owner's hand.",
    false
)]
#[case::another_target(
    "{T}: Target creature you control other than this creature gains shroud until end of turn.",
    "{T}: Another target creature you control gains shroud until end of turn.",
    true
)]
#[case::other_than_for_a_group(
    "At the beginning of your end step, put a +1/+1 counter on each creature you control other than this creature.",
    "At the beginning of your end step, put a +1/+1 counter on each creature you control other than ~.",
    true
)]
#[case::other_than_for_a_group_dropped(
    "At the beginning of your end step, put a +1/+1 counter on each creature you control other than this creature.",
    "At the beginning of your end step, put a +1/+1 counter on each creature you control.",
    false
)]
#[case::all_counters(
    "Remove all +1/+1 counters from this creature.",
    "Remove X +1/+1 counters from ~, where X is the number of +1/+1 counters on it.",
    true
)]
#[case::all_counters_of_another_kind(
    "Remove all +1/+1 counters from this creature.",
    "Remove X +1/+1 counters from ~, where X is the number of -1/-1 counters on it.",
    false
)]
#[case::one_counter_kind(
    "Remove all +1/+1 counters from this creature.",
    "Remove X +1/+1 counters from ~.",
    false
)]
#[case::type_at_the_start(
    "Rats you control have deathtouch.",
    "Rat creatures you control have deathtouch.",
    true
)]
#[case::card_type_at_the_start(
    "Artifacts you control have deathtouch.",
    "Artifact creatures you control have deathtouch.",
    false
)]
#[case::those_creatures(
    "Those creatures get +1/+1 until end of turn.",
    "Those get +1/+1 until end of turn.",
    false
)]
#[case::each_you_control(
    "Creatures you control get +1/+1 until end of turn.",
    "Each creature you control gets +1/+1 until end of turn.",
    true
)]
#[case::each_creature(
    "Creatures you control get +1/+1 until end of turn.",
    "Each creature gets +1/+1 until end of turn.",
    false
)]
#[case::then_is_no_type(
    "Choose a color. Creatures you control gain protection from the chosen color until end of turn.",
    "Choose a color. Then creatures you control gain protection from the chosen color until end of turn.",
    true
)]
#[case::main_phase_is_a_noun(
    "At the beginning of your second main phase, create a Food token.",
    "At the beginning of your postcombat main phase, create a Food token.",
    true
)]
#[case::other_main_phase(
    "At the beginning of your second main phase, create a Food token.",
    "At the beginning of your precombat main phase, create a Food token.",
    false
)]
#[case::up_to_after_a_trigger(
    "When ~ enters, up to one target creature phases out.",
    "When ~ enters and up to one target creature phases out.",
    false
)]
#[case::plural_with_y(
    "When ~ enters, you may have Ally creatures you control gain vigilance until end of turn.",
    "When ~ enters, you may have Allies you control gain vigilance until end of turn.",
    true
)]
fn w17_other_than_and_types(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}
