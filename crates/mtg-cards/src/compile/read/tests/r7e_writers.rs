//! Goal: the events the Forge translator models its own way (one object's damage as a batch, `Did` actions, cast triggers by a row) are written as the card words them, and another object, kind of damage, player, action or spell is not their text.
use super::*;

/// Whether one of the wordings of the ability is the text.
fn writes(json: &str, text: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

/// A trigger on an event that draws a card.
fn draws(when: &str) -> String {
    format!(r#"{{"Triggered":{{"when":{when},"effects":[{{"Draw":{{}}}}]}}}}"#)
}

#[rstest]
#[case::boros_reckoner(
    r#"{"Batch":{"DealsDamage":{"source":[],"to":"This"}}}"#,
    "Whenever ~ is dealt damage, draw a card."
)]
#[case::dealt_combat_damage(
    r#"{"Batch":{"DealsDamage":{"source":[],"to":"This","combat":true}}}"#,
    "Whenever ~ is dealt combat damage, draw a card."
)]
#[case::binding_agony(
    r#"{"Batch":{"DealsDamage":{"source":[],"to":"Attached"}}}"#,
    "Whenever enchanted creature is dealt damage, draw a card."
)]
#[case::blazing_sunsteel(
    r#"{"Batch":{"DealsDamage":{"source":[],"to":{"Each":[{"Type":"Creature"},{"Is":"Attached"}]}}}}"#,
    "Whenever equipped creature is dealt damage, draw a card."
)]
#[case::angel_of_deliverance(
    r#"{"Batch":{"DealsDamage":{"source":["This"]}}}"#,
    "Whenever ~ deals damage, draw a card."
)]
#[case::armadillo_cloak(
    r#"{"Batch":{"DealsDamage":{"source":[{"Is":"Attached"}]}}}"#,
    "Whenever enchanted creature deals damage, draw a card."
)]
#[case::banshees_blade(
    r#"{"Batch":{"DealsDamage":{"combat":true,"source":[{"Type":"Creature"},{"Is":"Attached"}]}}}"#,
    "Whenever equipped creature deals combat damage, draw a card."
)]
#[case::arashin_war_beast(
    r#"{"Batch":{"DealsDamage":{"combat":true,"source":["This"],"to":{"Each":[{"Type":"Creature"},"Blocking"]}}}}"#,
    "Whenever ~ deals combat damage to one or more blocking creatures, draw a card."
)]
#[case::mutates(
    r#"{"Did":{"action":"Mutated","what":["This"]}}"#,
    "Whenever ~ mutates, draw a card."
)]
#[case::exert_when_you_do(
    r#"{"Did":{"action":"Exerted","what":["This"]}}"#,
    "When you do, draw a card."
)]
#[case::explores(
    r#"{"Did":{"action":"Explored","what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#,
    "Whenever a creature you control explores, draw a card."
)]
#[case::scry(
    r#"{"Did":{"action":"Scried","who":"You"}}"#,
    "Whenever you scry, draw a card."
)]
#[case::surveil(
    r#"{"Did":{"action":"Surveilled","who":"You"}}"#,
    "Whenever you surveil, draw a card."
)]
#[case::opponent_shuffles(
    r#"{"Did":{"action":"Shuffled","who":"Opponents"}}"#,
    "Whenever an opponent shuffles their library, draw a card."
)]
#[case::player_loses(
    r#"{"Did":{"action":"LostGame"}}"#,
    "Whenever a player loses the game, draw a card."
)]
#[case::opponent_plays_land(
    r#"{"Did":{"action":"PlayedLand","what":[{"Type":"Land"},{"ControlledBy":"Opponents"}]}}"#,
    "Whenever an opponent plays a land, draw a card."
)]
#[case::player_plays_land(
    r#"{"Did":{"action":"PlayedLand","what":[{"Type":"Land"}]}}"#,
    "Whenever a player plays a land, draw a card."
)]
#[case::transforms_into(
    r#"{"Did":{"action":"Transformed","what":["This"]}}"#,
    "When ~ transforms into ~, draw a card."
)]
#[case::whenever_transforms_into(
    r#"{"Did":{"action":"Transformed","what":["This"]}}"#,
    "Whenever ~ transforms into ~, draw a card."
)]
#[case::enters_or_transforms(
    r#"{"AnyOf":[{"ZoneChange":{"to":"Battlefield","what":["This"]}},{"Did":{"action":"Transformed","what":["This"]}}]}"#,
    "Whenever ~ enters or transforms into ~, draw a card."
)]
#[case::cast_this(r#"{"SpellCast":{"what":["This"]}}"#, "When you cast ~, draw a card.")]
#[case::historic(
    r#"{"SpellCast":{"by":"You","what":[{"AnyOf":[{"Supertype":"Legendary"},{"Type":"Artifact"},{"Subtype":"Saga"}]}]}}"#,
    "Whenever you cast a historic spell, draw a card."
)]
#[case::not_from_your_hand(
    r#"{"SpellCast":{"by":"You","what":[{"AnyOf":[{"Not":{"WasCast":{"from":"Hand"}}},{"Not":{"OwnedBy":"You"}}]}]}}"#,
    "Whenever you cast a spell from anywhere other than your hand, draw a card."
)]
#[case::kicked(
    r#"{"SpellCast":{"by":"You","what":["Kicked"]}}"#,
    "Whenever you cast a kicked spell, draw a card."
)]
#[case::not_owned(
    r#"{"SpellCast":{"by":"You","what":[{"Not":{"OwnedBy":"You"}}]}}"#,
    "Whenever you cast a spell you don't own, draw a card."
)]
#[case::from_your_graveyard(
    r#"{"SpellCast":{"by":"You","what":[{"WasCast":{"from":"Graveyard"}},{"OwnedBy":"You"}]}}"#,
    "Whenever you cast a spell from your graveyard, draw a card."
)]
#[case::from_exile(
    r#"{"SpellCast":{"by":"You","what":[{"WasCast":{"from":"Exile"}}]}}"#,
    "Whenever you cast a spell from exile, draw a card."
)]
#[case::targets_yours(
    r#"{"SpellCast":{"by":"You","what":[{"ControlledBy":"You"},{"Targeting":[{"Type":"Creature"},{"ControlledBy":"You"},{"In":"Battlefield"}]}]}}"#,
    "Whenever you cast a spell that targets a creature you control, draw a card."
)]
#[case::instant_targets(
    r#"{"SpellCast":{"by":"You","what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"Targeting":[{"Type":"Creature"},{"In":"Battlefield"}]}]}}"#,
    "Whenever you cast an instant or sorcery spell that targets a creature, draw a card."
)]
#[case::you_discard(
    r#"{"Batch":{"Discarded":{"by":"You"}}}"#,
    "Whenever you discard one or more cards, draw a card."
)]
#[case::you_discard_lands(
    r#"{"Batch":{"Discarded":{"by":"You","what":[{"Type":"Land"}]}}}"#,
    "Whenever you discard one or more land cards, draw a card."
)]
#[case::a_player_discards(
    r#"{"Batch":{"Discarded":{}}}"#,
    "Whenever a player discards one or more cards, draw a card."
)]
#[case::camellia(
    r#"{"Batch":{"Sacrificed":{"by":"You","what":[{"Subtype":"Food"}]}}}"#,
    "Whenever you sacrifice one or more Foods, draw a card."
)]
#[case::cast_and_attacks(
    r#"{"AnyOf":[{"SpellCast":{"what":["This"]}},{"Attacks":{"what":["This"]}}]}"#,
    "When you cast ~ and whenever ~ attacks, draw a card."
)]
#[case::upkeep_and_cast(
    r#"{"AnyOf":[{"SpellCast":{"by":"You","what":[{"Color":"Red"}]}},{"StepBegins":{"step":"Upkeep","whose":"You"}}]}"#,
    "At the beginning of your upkeep and whenever you cast a red spell, draw a card."
)]
#[case::cycle_and_dies(
    r#"{"AnyOf":[{"Cycled":{"what":["This"]}},{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":["This"]}}]}"#,
    "When you cycle ~ and when ~ dies, draw a card."
)]
#[case::eerie(
    r#"{"AnyOf":[{"ZoneChange":{"to":"Battlefield","what":[{"Type":"Enchantment"},{"ControlledBy":"You"}]}},{"Did":{"action":"FullyUnlocked","what":[{"Subtype":"Room"}],"who":"You"}}]}"#,
    "Whenever an enchantment you control enters and whenever you fully unlock a Room, draw a card."
)]
#[case::scry_or_surveil(
    r#"{"AnyOf":[{"Did":{"action":"Scried","who":"You"}},{"Did":{"action":"Surveilled","who":"You"}}]}"#,
    "Whenever you scry or surveil, draw a card."
)]
#[case::cast_or_cycle(
    r#"{"AnyOf":[{"SpellCast":{"what":["This"]}},{"Cycled":{"what":["This"]}}]}"#,
    "When you cast or cycle ~, draw a card."
)]
#[case::land_or_spell(
    r#"{"AnyOf":[{"SpellCast":{"by":"You"}},{"Did":{"action":"PlayedLand","what":[{"Type":"Land"},{"ControlledBy":"You"}]}}]}"#,
    "Whenever you play a land or cast a spell, draw a card."
)]
#[case::play_a_card(
    r#"{"AnyOf":[{"Did":{"action":"PlayedLand","what":[{"Type":"Land"},{"ControlledBy":"You"}]}},{"SpellCast":{"by":"You"}}]}"#,
    "Whenever you play a card, draw a card."
)]
#[case::kicked_if(
    r#"{"SpellCast":{"by":"You","what":["Kicked"]}}"#,
    "Whenever you cast a spell, if that spell was kicked, draw a card."
)]
#[case::enchanted_player_loses(
    r#"{"Did":{"action":"LostGame","who":"AttachedPlayer"}}"#,
    "When enchanted player loses the game, draw a card."
)]
fn the_event_is_written_as_the_card_words_it(#[case] when: &str, #[case] text: &str) {
    let json = draws(when);
    assert!(
        writes(&json, text),
        "{text}: {:?}",
        written_variants(&ability(&json))
    );
}

#[rstest]
// A group dealt damage as one batch is not "a creature is dealt damage", which triggers for each creature.
#[case::group_dealt(
    r#"{"Batch":{"DealsDamage":{"source":[],"to":{"Each":[{"Type":"Creature"}]}}}}"#,
    "Whenever a creature is dealt damage, draw a card."
)]
#[case::other_kind_of_damage(
    r#"{"Batch":{"DealsDamage":{"source":[],"to":"This","combat":true}}}"#,
    "Whenever ~ is dealt damage, draw a card."
)]
#[case::dealt_not_deals(
    r#"{"Batch":{"DealsDamage":{"source":[],"to":"This"}}}"#,
    "Whenever ~ deals damage, draw a card."
)]
#[case::deals_not_dealt(
    r#"{"Batch":{"DealsDamage":{"source":["This"]}}}"#,
    "Whenever ~ is dealt damage, draw a card."
)]
#[case::noncombat_not_combat(
    r#"{"Batch":{"DealsDamage":{"combat":false,"source":["This"]}}}"#,
    "Whenever ~ deals combat damage, draw a card."
)]
#[case::to_a_player_not_any(
    r#"{"Batch":{"DealsDamage":{"source":["This"],"to":"EachPlayer"}}}"#,
    "Whenever ~ deals damage, draw a card."
)]
#[case::other_object_mutates(
    r#"{"Did":{"action":"Mutated","what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#,
    "Whenever ~ mutates, draw a card."
)]
#[case::other_player_scries(
    r#"{"Did":{"action":"Scried","who":"Opponents"}}"#,
    "Whenever you scry, draw a card."
)]
#[case::surveil_not_scry(
    r#"{"Did":{"action":"Surveilled","who":"You"}}"#,
    "Whenever you scry, draw a card."
)]
#[case::you_play_not_opponent(
    r#"{"Did":{"action":"PlayedLand","what":[{"Type":"Land"},{"ControlledBy":"You"}]}}"#,
    "Whenever an opponent plays a land, draw a card."
)]
#[case::nonbasic_land(
    r#"{"Did":{"action":"PlayedLand","what":[{"Type":"Land"},{"Not":{"Supertype":"Basic"}},{"ControlledBy":"Opponents"}]}}"#,
    "Whenever an opponent plays a land, draw a card."
)]
#[case::other_transforms(
    r#"{"Did":{"action":"Transformed","what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#,
    "When ~ transforms into ~, draw a card."
)]
#[case::opponent_casts_this(
    r#"{"SpellCast":{"by":"Opponents","what":["This"]}}"#,
    "When you cast ~, draw a card."
)]
#[case::opponent_casts_historic(
    r#"{"SpellCast":{"by":"Opponents","what":[{"AnyOf":[{"Supertype":"Legendary"},{"Type":"Artifact"},{"Subtype":"Saga"}]}]}}"#,
    "Whenever you cast a historic spell, draw a card."
)]
#[case::legendary_or_artifact(
    r#"{"SpellCast":{"by":"You","what":[{"AnyOf":[{"Supertype":"Legendary"},{"Type":"Artifact"}]}]}}"#,
    "Whenever you cast a historic spell, draw a card."
)]
#[case::from_hand_only(
    r#"{"SpellCast":{"by":"You","what":[{"Not":{"WasCast":{"from":"Hand"}}}]}}"#,
    "Whenever you cast a spell from anywhere other than your hand, draw a card."
)]
// "Whenever a player kicks a spell" is any player's spell (Saproling Infestation), not one you control.
#[case::kicked_by_you_is_not_any_player(
    r#"{"SpellCast":{"what":[{"ControlledBy":"You"},"Kicked"]}}"#,
    "Whenever a player kicks a spell, draw a card."
)]
#[case::any_graveyard(
    r#"{"SpellCast":{"by":"You","what":[{"WasCast":{"from":"Graveyard"}}]}}"#,
    "Whenever you cast a spell from your graveyard, draw a card."
)]
#[case::targets_any_creature(
    r#"{"SpellCast":{"by":"You","what":[{"ControlledBy":"You"},{"Targeting":[{"Type":"Creature"},{"In":"Battlefield"}]}]}}"#,
    "Whenever you cast a spell that targets a creature you control, draw a card."
)]
// The batch triggers once for each player, not once for all of them.
#[case::players_discard(
    r#"{"Batch":{"Discarded":{}}}"#,
    "Whenever one or more players discard one or more cards, draw a card."
)]
#[case::opponent_discards(
    r#"{"Batch":{"Discarded":{"by":"Opponents"}}}"#,
    "Whenever you discard one or more cards, draw a card."
)]
#[case::creature_cards(
    r#"{"Batch":{"Discarded":{"by":"You","what":[{"Type":"Creature"}]}}}"#,
    "Whenever you discard one or more land cards, draw a card."
)]
#[case::opponent_casts_spell(
    r#"{"AnyOf":[{"SpellCast":{"by":"Opponents"}},{"Did":{"action":"PlayedLand","what":[{"Type":"Land"},{"ControlledBy":"You"}]}}]}"#,
    "Whenever you play a card, draw a card."
)]
#[case::scry_or_surveil_of_another(
    r#"{"AnyOf":[{"Did":{"action":"Scried","who":"Opponents"}},{"Did":{"action":"Surveilled","who":"You"}}]}"#,
    "Whenever you scry or surveil, draw a card."
)]
#[case::cast_or_cycle_another(
    r#"{"AnyOf":[{"SpellCast":{"what":["This"]}},{"Cycled":{"what":["Other"]}}]}"#,
    "When you cast or cycle ~, draw a card."
)]
#[case::upkeep_of_each(
    r#"{"AnyOf":[{"SpellCast":{"by":"You","what":[{"Color":"Red"}]}},{"StepBegins":{"step":"Upkeep"}}]}"#,
    "At the beginning of your upkeep and whenever you cast a red spell, draw a card."
)]
#[case::any_spell_not_kicked(
    r#"{"SpellCast":{"by":"You"}}"#,
    "Whenever you cast a spell, if that spell was kicked, draw a card."
)]
fn another_event_is_not_the_text(#[case] when: &str, #[case] text: &str) {
    let json = draws(when);
    assert!(
        !writes(&json, text),
        "{text}: {:?}",
        written_variants(&ability(&json))
    );
}

#[rstest]
#[case::boros_reckoner(
    r#"{"Batch":{"DealsDamage":{"source":[],"to":"This"}}}"#,
    r#"[{"DealDamage":{"amount":"Triggering","to":{"Target":0}}}]"#,
    r#"[{"what":"AnyTarget"}]"#,
    "Whenever ~ is dealt damage, it deals that much damage to any target."
)]
#[case::exerted_gets(
    r#"{"Did":{"action":"Exerted","what":["This"]}}"#,
    r#"[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":2,"toughness":2}}],"duration":"UntilEndOfTurn"}}]"#,
    "[]",
    "When you do, it gets +2/+2 until end of turn."
)]
fn the_card_of_its_own_event_is_it(
    #[case] when: &str,
    #[case] effects: &str,
    #[case] targets: &str,
    #[case] text: &str,
) {
    let json =
        format!(r#"{{"Triggered":{{"when":{when},"effects":{effects},"targets":{targets}}}}}"#);
    assert!(
        writes(&json, text),
        "{text}: {:?}",
        written_variants(&ability(&json))
    );
}

const ANY_PLAYER_PAYS: &str =
    r#"{"Spell":{"effects":[{"Pay":{"who":"EachPlayer","cost":"{2}","unpaid":[{"Draw":{}}]}}]}}"#;
const YOUR_GRAVEYARD: &str = r#"{"Replacement":{"by":{"MoveTo":"Exile"},"when":{"ZoneChange":{"to":"Graveyard","what":[{"OwnedBy":"You"}]}}}}"#;
const OPPONENTS_GRAVEYARD: &str = r#"{"Replacement":{"by":{"MoveTo":"Exile"},"when":{"ZoneChange":{"to":"Graveyard","what":[{"OwnedBy":"Opponents"}]}}}}"#;

/// The players are asked in turn until one pays (Soul Strings), and what you own takes in your tokens (Festival of
/// Embers).
#[rstest]
#[case::any_player_pays(ANY_PLAYER_PAYS, "Draw a card unless any player pays {2}.", true)]
#[case::not_each_player(ANY_PLAYER_PAYS, "Draw a card unless each player pays {2}.", false)]
#[case::card_or_token(
    YOUR_GRAVEYARD,
    "If a card or token would be put into your graveyard from anywhere, exile it instead.",
    true
)]
#[case::a_card(
    YOUR_GRAVEYARD,
    "If a card would be put into your graveyard from anywhere, exile it instead.",
    true
)]
#[case::not_an_opponents(
    OPPONENTS_GRAVEYARD,
    "If a card or token would be put into your graveyard from anywhere, exile it instead.",
    false
)]
fn the_players_and_the_objects_are_worded_as_the_card_says(
    #[case] json: &str,
    #[case] text: &str,
    #[case] written: bool,
) {
    assert_eq!(
        writes(json, text),
        written,
        "{text}: {:?}",
        written_variants(&ability(json))
    );
}
