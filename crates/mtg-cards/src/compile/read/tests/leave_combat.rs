//! Goal: the losses of a static ability ("loses all abilities and can't attack or block", "has base power and toughness 0/2, has defender, and loses all other abilities") and the removal of creatures from combat ("remove enchanted creature from combat. Then draw a card for each tapped creature its controller controls") read into the data, write back in the card's wording, and leave a near miss to the table.
use super::*;

#[rstest]
#[case::attack_and_block(
    "Enchanted creature loses all abilities and can't attack or block.",
    r#"{"Static":{"affected":"Attached","mods":["LoseAbilities","CantAttack","CantBlock"]}}"#
)]
#[case::attack_only(
    "Enchanted creature loses all abilities and can't attack.",
    r#"{"Static":{"affected":"Attached","mods":["LoseAbilities","CantAttack"]}}"#
)]
#[case::block_only(
    "Enchanted creature loses all abilities and can't block.",
    r#"{"Static":{"affected":"Attached","mods":["LoseAbilities","CantBlock"]}}"#
)]
#[case::the_loss_alone(
    "Enchanted creature loses all abilities.",
    r#"{"Static":{"affected":"Attached","mods":["LoseAbilities"]}}"#
)]
#[case::a_size_and_a_keyword_kept(
    "Enchanted creature has base power and toughness 0/2, has defender, and loses all other abilities.",
    r#"{"Static":{"affected":"Attached","mods":[{"SetPT":{"power":0,"toughness":2}},"LoseAbilities",{"AddKeyword":"Defender"}]}}"#
)]
#[case::two_keywords_kept(
    "Enchanted creature has base power and toughness 1/1, has flying and haste, and loses all other abilities.",
    r#"{"Static":{"affected":"Attached","mods":[{"SetPT":{"power":1,"toughness":1}},"LoseAbilities",{"AddKeyword":"Flying"},{"AddKeyword":"Haste"}]}}"#
)]
#[case::a_group(
    "Creatures you control lose all abilities and can't block.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":["LoseAbilities","CantBlock"]}}"#
)]
fn a_loss_of_abilities_reads_and_writes_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::the_host_on_entering(
    "When this Aura enters, remove enchanted creature from combat.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"RemoveFromCombat":{"what":"Attached"}}]}}"#
)]
#[case::the_host_then_a_draw_for_its_controllers_tapped_creatures(
    "When this Aura enters, remove enchanted creature from combat. Then draw a card for each tapped creature its controller controls.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"RemoveFromCombat":{"what":"Attached"}},{"Draw":{"count":{"Count":[{"Type":"Creature"},"Tapped",{"ControlledBy":{"ControllerOf":"Attached"}}]}}}]}}"#
)]
#[case::a_draw_without_then(
    "When this Aura enters, remove enchanted creature from combat. Draw a card for each tapped creature its controller controls.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"RemoveFromCombat":{"what":"Attached"}},{"Draw":{"count":{"Count":[{"Type":"Creature"},"Tapped",{"ControlledBy":{"ControllerOf":"Attached"}}]}}}]}}"#
)]
#[case::a_draw_for_your_own_tapped_creatures(
    "When this Aura enters, remove enchanted creature from combat. Then draw a card for each tapped creature you control.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"RemoveFromCombat":{"what":"Attached"}},{"Draw":{"count":{"Count":[{"Type":"Creature"},"Tapped",{"ControlledBy":"You"}]}}}]}}"#
)]
#[case::the_card_itself(
    "Whenever this creature attacks, remove ~ from combat.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"RemoveFromCombat":{"what":"This"}}]}}"#
)]
#[case::a_target_that_attacks(
    "Remove target attacking creature from combat.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},"Attacking"]}}],"effects":[{"RemoveFromCombat":{"what":{"Target":0}}}]}}"#
)]
#[case::a_target_of_the_defending_player(
    "Remove target creature defending player controls from combat.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Defending"}]}}],"effects":[{"RemoveFromCombat":{"what":{"Target":0}}}]}}"#
)]
#[case::every_attacker(
    "Remove all attacking creatures from combat.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"RemoveFromCombat":{"what":{"Each":[{"Type":"Creature"},"Attacking"]}}}]}}"#
)]
#[case::as_an_activated_ability(
    "{0}: Remove target attacking creature you control from combat.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":"{0}","targets":[{"what":{"Object":[{"Type":"Creature"},"Attacking",{"ControlledBy":"You"}]}}],"effects":[{"RemoveFromCombat":{"what":{"Target":0}}}]}}"#
)]
fn a_removal_from_combat_reads_and_writes_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::activated_abilities_too(
    "Enchanted creature loses all abilities and can't attack or block, and its activated abilities can't be activated.",
    ClauseKind::Static
)]
#[case::a_restriction_of_another_kind(
    "Enchanted creature loses all abilities and can't be blocked.",
    ClauseKind::Static
)]
#[case::the_size_without_a_keyword(
    "Enchanted creature has base power and toughness 0/2 and loses all other abilities.",
    ClauseKind::Static
)]
#[case::the_keywords_after_the_loss_of_the_others(
    "Enchanted creature has base power and toughness 0/2, loses all abilities, and has defender.",
    ClauseKind::Static
)]
#[case::a_pronoun_without_a_creature_before(
    "Whenever this creature attacks, remove it from combat.",
    ClauseKind::Triggered
)]
#[case::the_game_instead_of_combat("Remove target creature from the game.", ClauseKind::Spell)]
#[case::a_player("Remove target player from combat.", ClauseKind::Spell)]
fn near_misses_of_the_losses_and_the_removals_are_left_to_the_table(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}
