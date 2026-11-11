//! Per-trigger answers: a seat lets the server answer the questions about its own triggers for the rest of the game.
//!
//! A rule ([`AutoSkip`]) is held with the stops ([`Stops::auto_skips`]) and keyed by what every player reads: the yes or
//! no of an optional trigger by the ability's key on the stack ([`AutoYield::of`]), the order of triggers that go on
//! the stack together by their sources' names and ability indices. The table answers a question by a rule at once, out
//! of Full Control only, so the pace shows the seat's choice and never a hidden card. A question whose answer could
//! hang on a hidden card (`YesNoOptions::unseen`) is always asked.

use mtg_core::ids::PlayerId;
use mtg_engine::choose::{
    TriggerEntry, TriggersChoice, TriggersOptions, YesNoChoice, YesNoOptions,
};
use mtg_engine::{Answer, Decision, DecisionKind, GameState};
use mtg_protocol::{
    AutoSkip, AutoYield, ErrorCode, MAX_AUTO_SKIPS, MAX_ORDER_TRIGGERS, ObjectView, TriggerKey,
    spectator_viewer,
};

use crate::actor::{Fatal, GameActor};
use crate::running::Rejection;
use crate::stops::Stops;
use crate::store::EventStore;

/// The answer a rule of the holder of `decision` gives it, if one does. The caller checks Full Control.
pub(crate) fn answer(decision: &Decision, stops: &Stops, state: &GameState) -> Option<Answer> {
    let seat = decision.player;
    match &decision.kind {
        DecisionKind::YesNo(options) => {
            let key = own_question(seat, options, state)?;
            let yes = stops.auto_answer(seat, &key)?;
            Some(Answer::YesNo(YesNoChoice(yes && !options.only_no)))
        }
        DecisionKind::OrderTriggers(options) => {
            let keys = trigger_keys(options, state)?;
            let mut sorted = keys.clone();
            sorted.sort();
            let order = stops.auto_order(seat, &sorted)?;
            in_rule_order(&keys, order).map(Answer::OrderTriggers)
        }
        _ => None,
    }
}

/// The key of the seat's own ability (an optional trigger, most often) that is resolving and asks `options` of its
/// source, if that is what asks: never a question a hidden card could decide, and never the return of a commander.
fn own_question(seat: PlayerId, options: &YesNoOptions, state: &GameState) -> Option<AutoYield> {
    if options.unseen || options.commander_return {
        return None;
    }
    let object = state.object(state.resolving()?.object())?;
    let ability = object.ability()?;
    if object.controller() != seat || ability.source().id != options.source {
        return None;
    }
    AutoYield::of(state, object.id())
}

/// The keys of the triggers to order, in the order they triggered; `None` when a source is hidden from some player.
fn trigger_keys(options: &TriggersOptions, state: &GameState) -> Option<Vec<TriggerKey>> {
    options
        .triggers
        .iter()
        .map(|entry| trigger_key(entry, state))
        .collect()
}

/// The key of one trigger to order: its source's name as a spectator reads it, and the ability's index.
pub(crate) fn trigger_key(entry: &TriggerEntry, state: &GameState) -> Option<TriggerKey> {
    let view = ObjectView::of(state, spectator_viewer(state), entry.source.id)?;
    let ability = u32::try_from(entry.ability).ok()?;
    (!view.name.is_empty()).then_some(TriggerKey {
        name: view.name,
        ability,
    })
}

/// The indices of `keys` in the order of `order`, which holds the same keys: of two triggers with one key, the
/// earlier goes first.
fn in_rule_order(keys: &[TriggerKey], order: &[TriggerKey]) -> Option<TriggersChoice> {
    let mut used = vec![false; keys.len()];
    let mut chosen = Vec::with_capacity(keys.len());
    for wanted in order {
        let index = (0..keys.len()).find(|&i| !used[i] && keys[i] == *wanted)?;
        used[index] = true;
        chosen.push(index);
    }
    (chosen.len() == keys.len()).then_some(TriggersChoice(chosen))
}

/// Why `rule` cannot be held, if it cannot.
fn refusal(rule: &AutoSkip) -> Option<&'static str> {
    match rule {
        AutoSkip::Answer { ability, .. } if ability.name.is_empty() => {
            Some("a rule names an ability by its source's name")
        }
        AutoSkip::Order { triggers }
            if triggers.len() < 2 || triggers.len() > MAX_ORDER_TRIGGERS =>
        {
            Some("an order rule puts between 2 and 16 triggers in order")
        }
        AutoSkip::Order { triggers } if triggers.iter().any(|t| t.name.is_empty()) => {
            Some("a rule names each trigger by its source's name")
        }
        _ => None,
    }
}

impl<S: EventStore> GameActor<S> {
    /// Gives `seat` the rule `rule` in place of its rule with the same key, from the next question on.
    ///
    /// A rule that cannot be held, or one past [`MAX_AUTO_SKIPS`], is refused with `Error{BadMessage}`.
    pub(crate) async fn set_auto_skip(
        &mut self,
        seat: PlayerId,
        rule: AutoSkip,
    ) -> Result<(), Fatal> {
        if let Some(rejection) = self.out_of_game(seat) {
            self.refuse(seat, rejection);
            return Ok(());
        }
        if let Some(reason) = refusal(&rule) {
            self.refuse(seat, Rejection::new(ErrorCode::BadMessage, reason));
            return Ok(());
        }
        let mut stops = self.stops.clone();
        stops.set_auto_skip(seat, rule);
        if stops.auto_skips(seat).len() > MAX_AUTO_SKIPS {
            let reason = format!("a seat holds at most {MAX_AUTO_SKIPS} rules");
            self.refuse(seat, Rejection::new(ErrorCode::BadMessage, reason));
            return Ok(());
        }
        self.take_stops(seat, stops).await
    }

    /// Takes back `seat`'s rule with the key of `rule`; one it does not hold changes nothing and gets silence.
    pub(crate) async fn cancel_auto_skip(
        &mut self,
        seat: PlayerId,
        rule: AutoSkip,
    ) -> Result<(), Fatal> {
        if let Some(rejection) = self.out_of_game(seat) {
            self.refuse(seat, rejection);
            return Ok(());
        }
        let mut stops = self.stops.clone();
        stops.cancel_auto_skip(seat, &rule);
        self.take_stops(seat, stops).await
    }
}

#[cfg(test)]
mod tests {
    //! Goal: an order rule names the triggers in its own order whatever order they triggered in, takes the earlier of two with one key first, and fits only the very same set; a rule that cannot be held is refused.
    use super::*;
    use rstest::rstest;

    /// The key of ability `ability` of a source named `name`.
    fn key(name: &str, ability: u32) -> TriggerKey {
        TriggerKey {
            name: name.to_owned(),
            ability,
        }
    }

    #[rstest]
    #[case::as_triggered(&[("A", 0), ("B", 0)], &[("A", 0), ("B", 0)], Some(vec![0, 1]))]
    #[case::reversed(&[("A", 0), ("B", 0)], &[("B", 0), ("A", 0)], Some(vec![1, 0]))]
    #[case::two_abilities_of_one_card(&[("A", 1), ("A", 0)], &[("A", 0), ("A", 1)], Some(vec![1, 0]))]
    #[case::copies_keep_their_order(&[("A", 0), ("B", 0), ("A", 0)], &[("A", 0), ("A", 0), ("B", 0)], Some(vec![0, 2, 1]))]
    #[case::one_more_trigger(&[("A", 0), ("B", 0), ("C", 0)], &[("A", 0), ("B", 0)], None)]
    #[case::another_trigger(&[("A", 0), ("C", 0)], &[("A", 0), ("B", 0)], None)]
    fn an_order_rule_names_its_triggers_in_its_own_order(
        #[case] triggered: &[(&str, u32)],
        #[case] rule: &[(&str, u32)],
        #[case] chosen: Option<Vec<usize>>,
    ) {
        let keys: Vec<_> = triggered.iter().map(|(n, a)| key(n, *a)).collect();
        let order: Vec<_> = rule.iter().map(|(n, a)| key(n, *a)).collect();
        assert_eq!(in_rule_order(&keys, &order), chosen.map(TriggersChoice));
    }

    #[rstest]
    #[case::an_answer(AutoSkip::Answer { ability: AutoYield { name: "A".into(), text: String::new() }, yes: true }, true)]
    #[case::an_unnamed_answer(AutoSkip::Answer { ability: AutoYield { name: String::new(), text: "t".into() }, yes: true }, false)]
    #[case::an_order(AutoSkip::Order { triggers: vec![key("A", 0), key("A", 0)] }, true)]
    #[case::one_trigger(AutoSkip::Order { triggers: vec![key("A", 0)] }, false)]
    #[case::too_many(AutoSkip::Order { triggers: vec![key("A", 0); MAX_ORDER_TRIGGERS + 1] }, false)]
    #[case::an_unnamed_trigger(AutoSkip::Order { triggers: vec![key("A", 0), key("", 1)] }, false)]
    fn only_rules_that_can_be_kept_are_held(#[case] rule: AutoSkip, #[case] held: bool) {
        assert_eq!(refusal(&rule).is_none(), held);
    }
}
