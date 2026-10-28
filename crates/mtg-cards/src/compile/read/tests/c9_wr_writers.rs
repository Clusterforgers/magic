//! Goal: the c9 writers give the oracle wording of a may-exile and return of the one target and of a dead card returned at the next end step, under your or its owner's control, and of the whole abilities of the third samples with or without the ability's own damage source; another target, binding, controller, step, count, player or extra effect never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn c9_wr_written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const C9_WR_CONJURERS_CLOSET: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"May":{"effects":[{"Exile":{"what":{"Target":0},"as":"exiled"}},{"Move":{"what":{"Bound":"exiled"},"to":{"Battlefield":{"under":"You"}}}}]}}]}}"#;
const C9_WR_RESTORATION_ANGEL: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"Not":{"Subtype":"Angel"}},{"ControlledBy":"You"}]}}],"effects":[{"May":{"effects":[{"Exile":{"what":{"Target":0},"as":"flickered"}},{"Move":{"what":{"Bound":"flickered"},"to":{"Battlefield":{"under":"You"}}}}]}}]}}"#;
const C9_WR_RESURRECTION_ORB: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Is":"Attached"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Move":{"what":"Triggering","to":{"Battlefield":{}}}}]}}]}}"#;
const C9_WR_COURT_OF_IRE: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"targets":[{"what":"AnyTarget"}],"effects":[{"If":{"condition":{"Monarch":"You"},"then":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":7}}],"otherwise":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":2}}]}}]}}"#;
const C9_WR_PONDER: &str = r#"{"Spell":{"effects":[{"LookAtTop":{"count":3,"as":"top"}},{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Top","order":"Chosen"}}}},{"May":{"effects":[{"Shuffle":{}}]}},{"Draw":{}}]}}"#;
const C9_WR_PRICE_OF_KNOWLEDGE: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"Opponents"}},"effects":[{"DealDamage":{"source":"This","to":"Triggering","amount":{"Count":[{"In":"Hand"},{"OwnedBy":"Triggering"}]}}}]}}"#;
const C9_WR_HRAESVELGR: &str = r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"SpellCast":{"what":[{"Not":{"Type":"Creature"}}],"by":"You"}}]},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":1,"toughness":0}},"CantBeBlocked"],"duration":"UntilEndOfTurn"}}]}}"#;

const C9_WR_CLOSET_TEXT: &str = "At the beginning of your end step, you may exile target creature you control, then return that card to the battlefield under your control.";
const C9_WR_ANGEL_TEXT: &str = "When ~ enters, you may exile target non-Angel creature you control, then return that card to the battlefield under your control.";
const C9_WR_ORB_TEXT: &str = "Whenever equipped creature dies, return that card to the battlefield under its owner's control at the beginning of the next end step.";
const C9_WR_COURT_TEXT: &str = "At the beginning of your upkeep, ~ deals 2 damage to any target. If you're the monarch, it deals 7 damage instead.";
const C9_WR_PONDER_TEXT: &str = "Look at the top three cards of your library, then put them back in any order. You may shuffle.\nDraw a card.";
const C9_WR_PRICE_TEXT: &str = "At the beginning of each opponent's upkeep, ~ deals damage to that player equal to the number of cards in that player's hand.";
const C9_WR_HRAESVELGR_TEXT: &str = "When ~ enters and whenever you cast a noncreature spell, target creature gets +1/+0 until end of turn and can't be blocked this turn.";

#[rstest]
#[case::closet(C9_WR_CONJURERS_CLOSET, C9_WR_CLOSET_TEXT)]
#[case::angel(C9_WR_RESTORATION_ANGEL, C9_WR_ANGEL_TEXT)]
#[case::closet_for_its_owner(
    &C9_WR_CONJURERS_CLOSET.replace(r#"{"under":"You"}"#, "{}"),
    &C9_WR_CLOSET_TEXT.replace("your control.", "its owner's control.")
)]
#[case::orb(C9_WR_RESURRECTION_ORB, C9_WR_ORB_TEXT)]
#[case::orb_under_you(
    &C9_WR_RESURRECTION_ORB.replace(r#""Battlefield":{}"#, r#""Battlefield":{"under":"You"}"#),
    &C9_WR_ORB_TEXT.replace("its owner's", "your")
)]
#[case::court(C9_WR_COURT_OF_IRE, C9_WR_COURT_TEXT)]
#[case::court_without_its_source(&C9_WR_COURT_OF_IRE.replace(r#""source":"This","#, ""), C9_WR_COURT_TEXT)]
#[case::ponder(C9_WR_PONDER, C9_WR_PONDER_TEXT)]
#[case::price(C9_WR_PRICE_OF_KNOWLEDGE, C9_WR_PRICE_TEXT)]
#[case::hraesvelgr(C9_WR_HRAESVELGR, C9_WR_HRAESVELGR_TEXT)]
fn c9_wr_the_oracle_wording_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(c9_wr_written(json, clause), "{clause}");
}

#[rstest]
#[case::closet_for_its_owner_said_yours(
    &C9_WR_CONJURERS_CLOSET.replace(r#"{"under":"You"}"#, "{}"),
    C9_WR_CLOSET_TEXT
)]
#[case::closet_returns_another_name(
    &C9_WR_CONJURERS_CLOSET.replace(r#"{"Bound":"exiled"}"#, r#"{"Bound":"other"}"#),
    C9_WR_CLOSET_TEXT
)]
#[case::closet_returns_tapped(
    &C9_WR_CONJURERS_CLOSET.replace(r#"{"under":"You"}"#, r#"{"under":"You","tapped":true}"#),
    C9_WR_CLOSET_TEXT
)]
#[case::closet_then_draws(
    &C9_WR_CONJURERS_CLOSET.replace(r"]}}]}}", r#",{"Draw":{}}]}}]}}"#),
    C9_WR_CLOSET_TEXT
)]
#[case::angel_targets_any_creature(
    &C9_WR_RESTORATION_ANGEL.replace(r#"{"Not":{"Subtype":"Angel"}},"#, ""),
    C9_WR_ANGEL_TEXT
)]
#[case::orb_under_you_said_its_owners(
    &C9_WR_RESURRECTION_ORB.replace(r#""Battlefield":{}"#, r#""Battlefield":{"under":"You"}"#),
    C9_WR_ORB_TEXT
)]
#[case::orb_at_your_end_step(
    &C9_WR_RESURRECTION_ORB.replace(r#"{"step":"End"}"#, r#"{"step":"End","whose":"You"}"#),
    C9_WR_ORB_TEXT
)]
#[case::orb_returns_the_card_itself(
    &C9_WR_RESURRECTION_ORB.replace(r#""what":"Triggering""#, r#""what":"This""#),
    C9_WR_ORB_TEXT
)]
#[case::court_for_six(&C9_WR_COURT_OF_IRE.replace(r#""amount":7"#, r#""amount":6"#), C9_WR_COURT_TEXT)]
#[case::court_from_the_target(
    &C9_WR_COURT_OF_IRE.replace(r#""source":"This""#, r#""source":{"Target":0}"#),
    C9_WR_COURT_TEXT
)]
#[case::ponder_without_the_draw(&C9_WR_PONDER.replace(r#",{"Draw":{}}"#, ""), C9_WR_PONDER_TEXT)]
#[case::price_for_your_hand(
    &C9_WR_PRICE_OF_KNOWLEDGE.replace(r#""OwnedBy":"Triggering""#, r#""OwnedBy":"You""#),
    C9_WR_PRICE_TEXT
)]
#[case::hraesvelgr_on_creature_spells(
    &C9_WR_HRAESVELGR.replace(r#"{"Not":{"Type":"Creature"}}"#, r#"{"Type":"Creature"}"#),
    C9_WR_HRAESVELGR_TEXT
)]
fn c9_wr_another_shape_never_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(!c9_wr_written(json, clause), "{clause}");
}
