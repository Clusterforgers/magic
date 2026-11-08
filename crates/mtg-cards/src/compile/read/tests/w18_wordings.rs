//! Goal: the w18 forms (what a permanent becomes in any wording, "any number of" bounded by its own count, mana that may not pay for a kind of spell, damage prevented from a kind of source, a second action on the same object, small forms and verb numbers) read as the text they mean, and a text of another meaning never reads as the clause.
use super::*;

#[rstest]
#[case::base_stats_first(
    "Until end of turn, target creature loses all abilities and becomes a green Elephant with base power and toughness 3/3.",
    "Until end of turn, target creature has base power and toughness 3/3 and becomes a green and becomes an Elephant and loses all abilities.",
    true
)]
#[case::keywords_in_the_with_list(
    "{3}: Until end of turn, ~ becomes a Dragon with base power and toughness 4/4, flying, and haste.",
    "{3}: Until end of turn, ~ has base power and toughness 4/4 and becomes a Dragon and gains haste and flying.",
    true
)]
#[case::abilities_lost_in_the_list(
    "Until end of turn, target creature becomes a blue Dragon Illusion with base power and toughness 4/4, loses all abilities, and gains flying.",
    "Until end of turn, target creature has base power and toughness 4/4 and becomes a blue and becomes a Dragon Illusion and gains flying and loses all abilities.",
    true
)]
#[case::other_size(
    "Until end of turn, target creature becomes a green Elephant with base power and toughness 3/3.",
    "Until end of turn, target creature has base power and toughness 2/2 and becomes a green and becomes an Elephant.",
    false
)]
#[case::abilities_kept(
    "Until end of turn, target creature loses all abilities and becomes a green Elephant with base power and toughness 3/3.",
    "Until end of turn, target creature has base power and toughness 3/3 and becomes a green and becomes an Elephant.",
    false
)]
#[case::other_keyword(
    "Until end of turn, target creature becomes a Dragon with base power and toughness 4/4 and gains flying.",
    "Until end of turn, target creature has base power and toughness 4/4 and becomes a Dragon and gains haste.",
    false
)]
#[case::other_kind(
    "Until end of turn, target creature becomes a Dragon with base power and toughness 4/4.",
    "Until end of turn, target creature has base power and toughness 4/4 and becomes a Drake.",
    false
)]
#[case::other_duration(
    "Target creature becomes a Dragon with base power and toughness 4/4 until end of turn.",
    "Target creature has base power and toughness 4/4 and becomes a Dragon.",
    false
)]
fn w18_what_a_permanent_becomes(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::artifacts(
    "Destroy any number of target artifacts and/or enchantments.",
    "Destroy up to X target artifacts and/or enchantments, where X is the number of artifacts and enchantments on the battlefield.",
    true
)]
#[case::you_control(
    "Exile any number of target nonland permanents you control.",
    "Exile up to X target nonland permanents you control, where X is the number of nonland permanents you control.",
    true
)]
#[case::other_pool(
    "Destroy any number of target artifacts.",
    "Destroy up to X target artifacts, where X is the number of lands you control.",
    false
)]
#[case::narrower_pool(
    "Destroy any number of target creatures.",
    "Destroy up to X target creatures, where X is the number of creatures you control.",
    false
)]
#[case::another_x_in_the_targets(
    "Destroy any number of target artifacts with total mana value X or less.",
    "Destroy up to X target artifacts with total mana value X or less, where X is the number of artifacts on the battlefield.",
    false
)]
#[case::a_fixed_bound(
    "Destroy up to two target artifacts.",
    "Destroy any number of target artifacts.",
    false
)]
fn w18_any_number_of_targets(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::nonartifact(
    "{T}: Add {C}. This mana can't be spent to cast a nonartifact spell.",
    "{T}: Add {C}. Then spend this mana only to cast an artifact spell or to activate an ability.",
    true
)]
#[case::other_kind(
    "{T}: Add {C}. This mana can't be spent to cast a noncreature spell.",
    "{T}: Add {C}. Then spend this mana only to cast an artifact spell or to activate an ability.",
    false
)]
#[case::only_spells(
    "{T}: Add {C}. This mana can't be spent to cast a nonartifact spell.",
    "{T}: Add {C}. Spend this mana only to cast an artifact spell.",
    false
)]
fn w18_mana_that_pays_for_a_kind_of_spell(
    #[case] left: &str,
    #[case] right: &str,
    #[case] same: bool,
) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::kind_of_source(
    "Prevent all damage that would be dealt to ~ by Deserts.",
    "Prevent that damage if a Desert would deal damage to ~.",
    true
)]
#[case::artifact_sources(
    "Prevent all damage that would be dealt to ~ by artifact sources.",
    "Prevent that damage if an artifact would deal damage to ~.",
    true
)]
#[case::dealt_by_the_card(
    "Prevent all damage that ~ would deal to snow creatures.",
    "Prevent that damage if ~ would deal damage to a snow creature.",
    true
)]
#[case::other_source(
    "Prevent all damage that would be dealt to ~ by Deserts.",
    "Prevent that damage if a creature would deal damage to ~.",
    false
)]
#[case::other_object(
    "Prevent all damage that would be dealt to ~ by Deserts.",
    "Prevent that damage if a Desert would deal damage to you.",
    false
)]
#[case::any_source_is_not_a_permanent(
    "Prevent all damage that would be dealt to you by sources you don't control.",
    "Prevent that damage if a permanent you don't control would deal damage to you.",
    false
)]
fn w18_damage_from_a_kind_of_source(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::cant_be_blocked(
    "Target creature gets +1/+0 until end of turn and can't be blocked this turn.",
    "Target creature gets +1/+0 until end of turn, then that creature can't be blocked this turn.",
    true
)]
#[case::it_cant(
    "Target creature gets +2/+0 until end of turn and it can't be blocked this turn.",
    "Target creature gets +2/+0 until end of turn, then it can't be blocked this turn.",
    true
)]
#[case::other_restriction(
    "Target creature gets +1/+0 until end of turn and can't be blocked this turn.",
    "Target creature gets +1/+0 until end of turn, then that creature can't attack this turn.",
    false
)]
#[case::other_object(
    "Target creature gets +1/+0 until end of turn and can't be blocked this turn.",
    "Target creature gets +1/+0 until end of turn, then target creature can't be blocked this turn.",
    false
)]
fn w18_a_second_action_on_the_same_object(
    #[case] left: &str,
    #[case] right: &str,
    #[case] same: bool,
) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::and_or(
    "Return up to 2 instant and/or sorcery cards from your graveyard to your hand.",
    "Return up to 2 instant and sorcery cards from your graveyard to your hand.",
    true
)]
#[case::or_is_not_and(
    "Return up to 2 instant or sorcery cards from your graveyard to your hand.",
    "Return up to 2 instant and sorcery cards from your graveyard to your hand.",
    false
)]
#[case::energy_symbol("You get {E}{E}.", "You get two energy counters.", true)]
#[case::energy_count("You get {E}{E}.", "You get three energy counters.", false)]
#[case::that_card(
    "Exile target card from a graveyard, then return it to the battlefield.",
    "Exile target card from a graveyard, then return that card to the battlefield.",
    true
)]
#[case::that_land(
    "Destroy target land. That land doesn't untap during its controller's next untap step.",
    "Destroy target land. That permanent doesn't untap during its controller's next untap step.",
    true
)]
#[case::that_land_other_effect(
    "Destroy target land. That land doesn't untap during its controller's next untap step.",
    "Destroy target land. That permanent doesn't untap during its controller's next two untap steps.",
    false
)]
#[case::owner_shuffles(
    "At the beginning of the end step, ~'s owner shuffles it into their library.",
    "At the beginning of the end step, shuffle ~ into its owner's library.",
    true
)]
#[case::owner_of_the_target(
    "Choose target artifact or enchantment. Its owner shuffles it into their library.",
    "Target artifact or enchantment's owner shuffles it into their library.",
    true
)]
#[case::you_shuffle_is_not_the_owner(
    "At the beginning of the end step, ~'s owner shuffles it into their library.",
    "At the beginning of the end step, you shuffle ~ into your library.",
    false
)]
#[case::the_card_itself(
    "When ~ dies, you may exile ~. When you do, draw a card.",
    "When ~ dies, you may exile it. When you do, draw a card.",
    true
)]
#[case::the_card_from_which_counters_go(
    "At the beginning of your end step, you may remove a counter from ~.",
    "At the beginning of your end step, you may remove a counter from it.",
    true
)]
#[case::another_object_is_not_the_card(
    "When ~ dies, return target creature card to your hand, then exile ~.",
    "When ~ dies, return target creature card to your hand, then exile it.",
    false
)]
#[case::owners_possessive(
    "Return target creature to the battlefield under their owner's control.",
    "Return target creature to the battlefield under their owners' control.",
    true
)]
#[case::controllers_are_not_owners(
    "Return target creature to the battlefield under their owner's control.",
    "Return target creature to the battlefield under your control.",
    false
)]
#[case::bare_plural_subject(
    "{1}{S}: Snow creatures you control get +1/+1 until end of turn.",
    "{1}{S}: Each snow creature you control gets +1/+1 until end of turn.",
    true
)]
#[case::bare_plural_other_kind(
    "{1}{S}: Snow creatures you control get +1/+1 until end of turn.",
    "{1}{S}: Each nonsnow creature you control gets +1/+1 until end of turn.",
    false
)]
#[case::all_players_put(
    "All players may search their library for a card and put it into their hand.",
    "All players may search their library for a card and puts it into their hand.",
    true
)]
#[case::other_player_puts(
    "You may search your library for a card and put it into your hand.",
    "Target player may search their library for a card and puts it into their hand.",
    false
)]
fn w18_small_forms(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}
