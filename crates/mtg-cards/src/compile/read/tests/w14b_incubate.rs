//! Goal: the incubate wordings of Sunfall, Excise the Imperfect and Brimaz read into the DSL and write back as the cards'
//! own text: another player who incubates, a count of the permanents "exiled this way", a cast trigger of a creature of a
//! subtype or an artifact creature and the Phyrexians that died under your control; their near-misses stay unread.
use super::*;

#[rstest]
#[case::exiled_this_way(
    "Exile all creatures. Incubate X, where X is the number of creatures exiled this way.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Exile":{"what":{"Each":[{"Type":"Creature"}]}}},{"Incubate":{"count":{"Exiled":[{"Type":"Creature"}]}}}]}}"#
)]
#[case::exiled_draw(
    "Exile all creatures. Draw a card for each creature exiled this way.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Exile":{"what":{"Each":[{"Type":"Creature"}]}}},{"Draw":{"count":{"Exiled":[{"Type":"Creature"}]}}}]}}"#
)]
#[case::exiled_life(
    "Exile all artifacts. You gain 1 life for each artifact exiled this way.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Exile":{"what":{"Each":[{"Type":"Artifact"}]}}},{"GainLife":{"amount":{"Exiled":[{"Type":"Artifact"}]}}}]}}"#
)]
#[case::its_controller(
    "Exile target nonland permanent. Its controller incubates X, where X is its mana value.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Not":{"Type":"Land"}}]}}],"effects":[{"Exile":{"what":{"Target":0}}},{"Incubate":{"who":{"ControllerOf":{"Target":0}},"count":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}}]}}"#
)]
#[case::target_player(
    "Target player incubates 2.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Incubate":{"who":{"Target":0},"count":2}}]}}"#
)]
#[case::subtype_or_artifact_creature(
    "Whenever you cast a Phyrexian creature or artifact creature spell, incubate X, where X is that spell's mana value.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Type":"Creature"},{"AnyOf":[{"Subtype":"Phyrexian"},{"Type":"Artifact"}]}],"by":"You"}},"effects":[{"Incubate":{"count":{"Stat":{"of":"Triggering","stat":"ManaValue"}}}}]}}"#
)]
#[case::another_subtype(
    "Whenever you cast a Golem creature or artifact creature spell, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Type":"Creature"},{"AnyOf":[{"Subtype":"Golem"},{"Type":"Artifact"}]}],"by":"You"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::phyrexian_died(
    "At the beginning of each end step, if a Phyrexian died under your control this turn, proliferate.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End"}},"if":{"Compare":{"left":{"Tallied":{"who":"You","what":"PhyrexiansDiedUnder"}},"op":"Ge","right":1}},"effects":[{"Proliferate":{}}]}}"#
)]
fn w14b_incubate_wordings_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::no_where("Exile target nonland permanent. Its controller incubates X.", ClauseKind::Spell)]
#[case::exiled_cards_of_a_hand(
    "Exile all creatures. Incubate X, where X is the number of cards exiled this way from your hand.",
    ClauseKind::Spell
)]
#[case::enchantment_creature(
    "Whenever you cast a Phyrexian creature or enchantment creature spell, draw a card.",
    ClauseKind::Triggered
)]
#[case::not_a_subtype(
    "Whenever you cast a Blue creature or artifact creature spell, draw a card.",
    ClauseKind::Triggered
)]
#[case::two_phyrexians(
    "At the beginning of each end step, if two Phyrexians died under your control this turn, proliferate.",
    ClauseKind::Triggered
)]
fn w14b_incubate_near_misses_stay_unread(#[case] text: &str, #[case] kind: ClauseKind) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}
