//! Preventions that name the damage they prevent (CR 615): `ReplaceBy::PreventAnd` prevents all of it and runs effects
//! that read it ("prevent that damage and put that many +1/+1 counters on it"), `ReplaceBy::PreventAllBut` leaves a
//! fixed amount ("prevent all but 1 of that damage"); and the token replacements for one kind of token
//! (`ReplaceWhen::TokenCreation` with `what`), for which a run of tokens of several kinds is split by kind.

use mtg_cards::ability::Ability;
use mtg_cards::definition::CardDefinition;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Pred, Selector};
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};

use super::{Candidate, Item, Origin, Pass, Slot, Unit, slots_of};
use crate::events::Event;
use crate::resolve::w5d_run_instead;

/// Whether the token's card is of the kind: each predicate is a type, subtype, supertype or colour (or the negation of
/// one), which the validator holds a kind of token to; any other predicate does not match.
fn w9c_token_is(definition: &CardDefinition, what: &Selector) -> bool {
    what.0.iter().all(|pred| w9c_token_pred(definition, pred))
}

fn w9c_token_pred(definition: &CardDefinition, pred: &Pred) -> bool {
    match pred {
        Pred::Type(kind) => definition.types.contains(kind),
        Pred::Subtype(subtype) => definition.subtypes.contains(subtype),
        Pred::Supertype(supertype) => definition.supertypes.contains(supertype),
        Pred::Color(color) => definition.colors().contains(*color),
        Pred::Not(inner) => !w9c_token_pred(definition, inner),
        _ => false,
    }
}

impl Pass<'_> {
    /// The kinds of token the replacement effects at hand name.
    fn w9c_token_kinds(&self) -> Vec<&Selector> {
        self.sources
            .iter()
            .filter_map(|source| match &*source.ability {
                Ability::Replacement {
                    when: ReplaceWhen::TokenCreation { what, .. },
                    ..
                } if !what.is_empty() => Some(what),
                _ => None,
            })
            .collect()
    }

    /// Splits the run of tokens at `at` by the kinds of token the replacement effects name, so an effect for one kind
    /// replaces only those tokens ("one or more creature tokens" of a Treasure and a Zombie made together): the groups
    /// keep the order of their first token and what was applied to the run.
    pub(super) fn w9c_split_tokens(&mut self, at: usize) {
        let Some(Item::Tokens(run)) = self.slots.get(at).map(|slot| &slot.item) else {
            return;
        };
        let kinds = self.w9c_token_kinds();
        if kinds.is_empty() {
            return;
        }
        let mut groups: Vec<(Vec<bool>, Vec<Event>)> = Vec::new();
        for token in run {
            let key: Vec<bool> = kinds
                .iter()
                .map(|what| w9c_token_is(&token.definition, what))
                .collect();
            let event = Event::TokenCreated(token.clone());
            match groups.iter_mut().find(|(seen, _)| *seen == key) {
                Some((_, events)) => events.push(event),
                None => groups.push((key, vec![event])),
            }
        }
        if groups.len() < 2 {
            return;
        }
        let Slot {
            applied, frozen, ..
        } = self.slots[at].clone();
        let parts: Vec<Slot> = groups
            .into_iter()
            .flat_map(|(_, events)| slots_of(events))
            .map(|mut slot| {
                slot.applied.clone_from(&applied);
                slot.frozen = frozen;
                slot
            })
            .collect();
        self.slots.splice(at..=at, parts);
    }

    /// Whether the item at `at` is of the kind of token `when` names; true for any other event or a pattern with no kind.
    pub(super) fn w9c_kind_matches(&self, at: usize, when: &ReplaceWhen) -> bool {
        let ReplaceWhen::TokenCreation { what, .. } = when else {
            return true;
        };
        match &self.slots[at].item {
            Item::Tokens(run) if !what.is_empty() => run
                .iter()
                .all(|token| w9c_token_is(&token.definition, what)),
            _ => true,
        }
    }

    /// Prevents all of the damage at `at` and runs the effects in its place, reading it as `Instead` does (its
    /// recipient, its amount and its source). Their events carry the effects already applied, this one included. When
    /// the effects would ask a question, the damage is prevented and nothing else happens.
    pub(super) fn w9c_prevent_and(
        &mut self,
        at: usize,
        candidate: &Candidate,
        effects: &[Effect],
        unit: &Unit,
        applied: Vec<Origin>,
    ) {
        let cause = self.w5d_cause(at, unit);
        let events = w5d_run_instead(
            self.state,
            candidate.controller,
            candidate.object,
            &cause,
            effects,
        )
        .unwrap_or_default();
        self.prevent(at, candidate, &ReplaceBy::Prevent, applied.clone());
        let slots: Vec<Slot> = slots_of(events)
            .into_iter()
            .map(|mut slot| {
                slot.applied.clone_from(&applied);
                slot
            })
            .collect();
        let at = at.min(self.slots.len());
        self.slots.splice(at..at, slots);
    }

    /// Prevents all but `kept` of the damage at `at`; damage of `kept` or less is left as it is.
    pub(super) fn w9c_prevent_all_but(
        &mut self,
        at: usize,
        candidate: &Candidate,
        kept: u32,
        applied: Vec<Origin>,
    ) {
        let amount = match &self.slots[at].item {
            Item::One(Event::DamageDealt { amount, .. }) => *amount,
            _ => 0,
        };
        let by = ReplaceBy::PreventUpTo(amount.saturating_sub(kept));
        self.prevent(at, candidate, &by, applied);
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a prevention that names its damage prevents all of it and does what it says with its amount (counters on
    //! the creature), or all but the amount it keeps, never applies to damage that can't be prevented, and a token
    //! replacement for one kind of token replaces only the tokens of that kind, as many as there are, splitting a run of
    //! several kinds; at two, three and four seats.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_core::card::CardType;
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::super::{Processed, process};
    use crate::choose::Target;
    use crate::combat::{DamageForm, DamageTarget};
    use crate::counters::CountersChanged;
    use crate::events::Event;
    use crate::fixtures::{creature, state};
    use crate::state::GameState;
    use crate::tokens::TokenCreated;
    use crate::zones::Zone;

    const COUNTERS: &str = r#"{"Replacement":{"when":{"Damage":{"to":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}},"by":{"PreventAnd":[{"PutCounters":{"on":"Triggering","counter":"+1/+1","count":"Triggering"}}]}}}"#;
    const ALL_BUT: &str = r#"{"Replacement":{"when":{"Damage":{"to":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}},"by":{"PreventAllBut":1}}}"#;
    const ANGELS: &str = r#"{"Replacement":{"when":{"TokenCreation":{"what":[{"Type":"Creature"}]}},"by":{"Instead":[{"CreateToken":{"token":{"Custom":{"name":"Angel","colors":["White"],"types":["Creature"],"subtypes":["Angel"],"power":4,"toughness":4}},"count":"Triggering"}}]}}}"#;
    const UNPREVENTABLE: &str = r#"{"Rule":{"rule":{"DamageCantBePrevented":{}}}}"#;

    fn ability(json: &str) -> Ability {
        serde_json::from_str(json).unwrap()
    }

    /// Seat 0 controls a Source with `rules` and a Wall; seat 1 controls a Bear; the other seats are empty.
    fn game(seats: usize, rules: &[&str]) -> GameState {
        let mut source = CardDefinition::new("Source").with_type(CardType::Enchantment);
        for rule in rules {
            source = source.with_rules(ability(rule));
        }
        let mut battlefields = vec![
            vec![Arc::new(source), creature("Wall", 0, 4)],
            vec![creature("Bear", 2, 2)],
        ];
        battlefields.resize(seats, Vec::new());
        state(vec![vec![]; seats], battlefields)
    }

    fn at(game: &GameState, index: usize) -> ObjectId {
        game.zone(Zone::Battlefield)[index]
    }

    fn events(game: &GameState, batch: Vec<Event>) -> Vec<Event> {
        match process(game, batch, &[]).unwrap() {
            Processed::Events(events) => events,
            Processed::Ask { .. } => panic!("nothing is asked"),
        }
    }

    fn hit(source: ObjectId, to: ObjectId, amount: u32) -> Event {
        Event::DamageDealt {
            source,
            target: DamageTarget::Object(to),
            amount,
            deathtouch: false,
            form: DamageForm::default(),
            toxic: 0,
        }
    }

    #[rstest]
    fn prevented_damage_becomes_that_many_counters(
        #[values(2, 3, 4)] seats: usize,
        #[values(1, 3)] amount: u32,
    ) {
        let game = game(seats, &[COUNTERS]);
        let (wall, bear) = (at(&game, 1), at(&game, 2));
        let out = events(&game, vec![hit(bear, wall, amount)]);
        assert!(
            matches!(&out[..], [Event::CountersChanged(CountersChanged { on: Target::Object(on), delta, .. })]
                if on.id == wall && *delta == i32::try_from(amount).unwrap()),
            "{out:?}"
        );
        // Damage to a creature of another player is dealt.
        let other = hit(wall, bear, amount);
        assert_eq!(events(&game, vec![other.clone()]), [other]);
    }

    #[rstest]
    #[case::more(3, 1)]
    #[case::one(1, 1)]
    fn all_but_one_of_the_damage_is_prevented(#[case] amount: u32, #[case] dealt: u32) {
        let game = game(2, &[ALL_BUT]);
        let (wall, bear) = (at(&game, 1), at(&game, 2));
        let out = events(&game, vec![hit(bear, wall, amount)]);
        assert!(
            matches!(&out[..], [Event::DamageDealt { amount: a, .. }] if *a == dealt),
            "{out:?}"
        );
    }

    #[rstest]
    #[case::counters(COUNTERS)]
    #[case::all_but(ALL_BUT)]
    fn damage_that_cant_be_prevented_is_dealt(#[case] rule: &str) {
        let game = game(3, &[rule, UNPREVENTABLE]);
        let (wall, bear) = (at(&game, 1), at(&game, 2));
        let dealt = hit(bear, wall, 3);
        assert_eq!(events(&game, vec![dealt.clone()]), [dealt]);
    }

    fn token(game: &GameState, id: u64, kind: CardType) -> Event {
        let definition = CardDefinition::new("Thing").with_type(kind);
        Event::TokenCreated(TokenCreated {
            object: ObjectId(game.next_object + id),
            controller: PlayerId(0),
            name: "Thing".into(),
            tapped: false,
            attacking: None,
            definition: Arc::new(definition),
            copy_of: None,
        })
    }

    fn names(out: &[Event]) -> Vec<(String, bool)> {
        out.iter()
            .filter_map(|event| match event {
                Event::TokenCreated(made) => Some((
                    made.name.clone(),
                    made.definition.types.contains(&CardType::Creature),
                )),
                _ => None,
            })
            .collect()
    }

    #[rstest]
    fn creature_tokens_become_as_many_angels_and_the_rest_stay(
        #[values(2, 4)] seats: usize,
        #[values(1, 2)] creatures: u64,
    ) {
        let game = game(seats, &[ANGELS]);
        let mut batch = vec![token(&game, 0, CardType::Artifact)];
        batch.extend((1..=creatures).map(|id| token(&game, id, CardType::Creature)));
        let out = names(&events(&game, batch));
        let angels = out.iter().filter(|(name, _)| name == "Angel").count();
        assert_eq!(u64::try_from(angels).unwrap(), creatures, "{out:?}");
        assert_eq!(out.len(), 1 + angels, "{out:?}");
        assert!(out.contains(&("Thing".to_owned(), false)), "{out:?}");
    }
}
