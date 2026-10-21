//! The X a triggered ability reads (CR 107.3): the one a trigger remembers as it triggers, so its intervening "if", its
//! targets and its effects all read the same number whether its source is still there or not.

use mtg_cards::refs::{Pred, ZoneKind};
use mtg_cards::trigger::TriggerPattern;

use crate::abilities::Cause;
use crate::snapshot::Snapshot;
use crate::zones::Zone;

/// The X read by an ability that triggers on `when` because of `cause`, where `source` is the object it belongs to as it
/// was. A delayed trigger reads the X of the spell or ability that set it up; the trigger "when this enters" the X its
/// spell was cast with (CR 107.3m); a trigger on a spell being cast the X that spell was cast with, whether it is a
/// trigger of the spell itself ("when you cast this spell") or of another permanent ("whenever you cast a spell").
/// Any other trigger reads 0, which is `None`.
pub(crate) fn read(
    when: &TriggerPattern,
    delayed: bool,
    source: &Snapshot,
    cause: &Cause,
) -> Option<u32> {
    if delayed {
        return source.x;
    }
    match when {
        TriggerPattern::ZoneChange {
            what,
            to: Some(ZoneKind::Battlefield),
            ..
        } if what.0.contains(&Pred::This) => source.x,
        TriggerPattern::SpellCast { .. } => cause
            .snapshots
            .iter()
            .find(|shot| shot.zone == Zone::Stack)
            .and_then(|shot| shot.x),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a triggered ability reads X only where the rules give it one: a delayed trigger the X of what set it up, the trigger "when this enters" the X of its spell, a trigger on a spell being cast the X of the spell cast; any other trigger of a permanent, and a trigger on another zone change, read 0.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use mtg_core::ids::PlayerId;
    use mtg_core::mana::ManaCost;
    use rstest::rstest;

    use super::*;
    use crate::choose::Choices;
    use crate::events::Event;
    use crate::fixtures::state;
    use crate::fold::apply_event;
    use crate::snapshot::snapshot;

    /// A spell on the stack cast with `x`, as the snapshot of the cast event keeps it.
    fn spell_cast_with(x: Option<u32>) -> Snapshot {
        let card = Arc::new(
            CardDefinition::new("Spell")
                .with_type(CardType::Sorcery)
                .with_mana_cost(ManaCost::parse("{R}").unwrap())
                .with_x_in_cost(1),
        );
        let mut game = state(vec![vec![card], vec![]], vec![vec![], vec![]]);
        let id = game.zone(Zone::Hand(PlayerId(0)))[0];
        let event = Event::SpellCast {
            player: PlayerId(0),
            object: id,
            choices: Choices {
                x,
                ..Choices::default()
            },
        };
        apply_event(&mut game, &event).unwrap();
        snapshot(&game, id).unwrap()
    }

    fn pattern(json: &str) -> TriggerPattern {
        serde_json::from_str(json).unwrap()
    }

    const ENTERS: &str = r#"{"ZoneChange":{"what":["This"],"to":"Battlefield"}}"#;
    const OTHER_ENTERS: &str =
        r#"{"ZoneChange":{"what":[{"Type":"Creature"},"Other"],"to":"Battlefield"}}"#;
    const DIES: &str = r#"{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}}"#;
    const UPKEEP: &str = r#"{"StepBegins":{"step":"Upkeep","whose":"You"}}"#;
    const CASTS_THIS: &str = r#"{"SpellCast":{"what":["This"],"by":"You"}}"#;
    const CASTS_ANY: &str = r#"{"SpellCast":{"by":"You"}}"#;

    #[rstest]
    #[case::enters(ENTERS, false, Some(4), Some(4))]
    #[case::enters_with_no_x(ENTERS, false, None, None)]
    #[case::enters_with_nothing(ENTERS, false, Some(0), Some(0))]
    #[case::another_enters(OTHER_ENTERS, false, Some(4), None)]
    #[case::dies(DIES, false, Some(4), None)]
    #[case::upkeep(UPKEEP, false, Some(4), None)]
    #[case::delayed(UPKEEP, true, Some(4), Some(4))]
    #[case::delayed_with_no_x(UPKEEP, true, None, None)]
    fn a_trigger_reads_the_x_the_rules_give_it(
        #[case] when: &str,
        #[case] delayed: bool,
        #[case] source_x: Option<u32>,
        #[case] expected: Option<u32>,
    ) {
        let mut source = spell_cast_with(source_x);
        source.zone = Zone::Battlefield;
        let cause = Cause::default();
        assert_eq!(read(&pattern(when), delayed, &source, &cause), expected);
    }

    #[rstest]
    #[case::this_spell_with_nothing(CASTS_THIS, Some(0))]
    #[case::this_spell(CASTS_THIS, Some(1))]
    #[case::this_spell_with_a_large_x(CASTS_THIS, Some(250))]
    #[case::another_spell(CASTS_ANY, Some(3))]
    #[case::a_spell_without_x(CASTS_ANY, None)]
    fn a_trigger_on_a_spell_being_cast_reads_the_x_of_that_spell(
        #[case] when: &str,
        #[case] x: Option<u32>,
    ) {
        let spell = spell_cast_with(x);
        let cause = Cause {
            snapshots: vec![spell.clone()],
            ..Cause::default()
        };
        // The source is a permanent that has no X of its own.
        let mut source = spell;
        source.zone = Zone::Battlefield;
        source.x = None;
        assert_eq!(read(&pattern(when), false, &source, &cause), x);
    }

    #[test]
    fn a_cast_trigger_whose_cause_holds_no_spell_reads_0() {
        let source = spell_cast_with(Some(5));
        assert_eq!(
            read(&pattern(CASTS_ANY), false, &source, &Cause::default()),
            None
        );
    }
}
