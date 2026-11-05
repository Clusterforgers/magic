//! `Effect::DealDamage`: a source deals damage to players, creatures, planeswalkers and battles (CR 120).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::choose::Target;
use crate::combat::DamageTarget;
use crate::damage::{self, Dealer};
use crate::resolve::{Cx, LET, Next, ResolveError, Run};

/// The source deals each recipient still there to take it its amount; damage of nothing is no event.
///
/// The source is the spell or the ability's source unless the effect names another. A divided amount gives each
/// target its own share or an even part of the whole (`Scope::amounts_for`); a target that is not legal at
/// resolution has none. Recipients that left the game, are not on the battlefield any more or cannot be dealt
/// damage are passed over. A source with lifelink gains its controller the damage in the same batch
/// (`damage::dealt`).
///
/// With `excess`, the part of each amount beyond what is lethal to a creature, planeswalker or battle (CR 120.10) is
/// added up and kept under the name; with a `to`, only the lethal part goes to the recipient and the excess is dealt to
/// the players or objects `to` names instead. The damage is read before any prevention, which acts on the events.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::DealDamage {
        source,
        to,
        amount,
        excess,
    } = effect
    else {
        return Err(wrong_effect("DealDamage"));
    };
    let source = match source {
        None => cx.scope.source,
        Some(named) => match cx.scope.objects(named)?.first().copied() {
            Some(object) => object,
            // A creature that is no longer a legal target deals no damage; the spell does the rest (CR 608.2b).
            None => return Ok(Run::events(Vec::new())),
        },
    };
    let recipients = cx.scope.refs(to)?;
    let amounts = cx.scope.amounts_for(amount, &recipients)?;
    // A source that has left deals its damage as it was: with the keywords and under the controller it had.
    let dealer = match cx.scope.snapshot_of(source) {
        Some(shot) => Dealer::known(shot),
        None => Dealer::live(cx.state(), source.id),
    };
    let mut events = Vec::new();
    let mut beyond: u32 = 0;
    let mut instead = Vec::new();
    for (recipient, amount) in recipients.into_iter().zip(amounts) {
        let mut amount = u32::try_from(amount.max(0)).unwrap_or(u32::MAX);
        if amount == 0 {
            continue;
        }
        let target = match recipient {
            Target::Player(player) if cx.state().is_alive(player) => DamageTarget::Player(player),
            Target::Object(object) if cx.scope.can_be_dealt_damage(object) => {
                DamageTarget::Object(object.id)
            }
            _ => continue,
        };
        if let (Some(excess), DamageTarget::Object(id)) = (excess, target)
            && let Some(lethal) = damage::lethal_for_excess(cx.state(), id, dealer.has_deathtouch())
        {
            let over = amount.saturating_sub(lethal);
            beyond = beyond.saturating_add(over);
            if excess.to.is_some() {
                amount -= over;
                instead.push(over);
            }
        }
        events.extend(damage::dealt_by(cx.state(), &dealer, target, amount));
    }
    let mut bound = Vec::new();
    if let Some(excess) = excess {
        if let Some(to) = &excess.to {
            for over in instead.into_iter().filter(|over| *over > 0) {
                for recipient in cx.scope.refs(to)? {
                    let target = match recipient {
                        Target::Player(player) if cx.state().is_alive(player) => {
                            DamageTarget::Player(player)
                        }
                        Target::Object(object) if cx.scope.can_be_dealt_damage(object) => {
                            DamageTarget::Object(object.id)
                        }
                        _ => continue,
                    };
                    events.extend(damage::dealt_by(cx.state(), &dealer, target, over));
                }
            }
        }
        if let Some(name) = &excess.bind {
            bound.push((format!("{LET}{beyond}:{name}"), Vec::new()));
        }
    }
    Ok(Run::Done {
        events,
        bound,
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: with an excess, damage to a creature beyond what is lethal to it (the toughness less the damage already marked) is counted under the name and, with a recipient named, dealt to that recipient instead while the creature takes only the lethal part; an amount that is not beyond lethal, a player as the recipient and an effect without an excess change nothing.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::events::Event;
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope, prim};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn game(toughness: i32, marked: u32) -> GameState {
        let mut table = setup(
            vec![Vec::new(); 2],
            vec![vec![creature("Source", 1, toughness)], Vec::new()],
        );
        table.players[0].battlefield[0].damage = marked;
        GameState::from_setup(&table).unwrap()
    }

    /// The damage events as (to the creature, to the player) and the number kept under `n`.
    fn run(game: &GameState, json: &str) -> (u32, u32, Option<String>) {
        let effect: Effect = serde_json::from_str(json).unwrap();
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        let Ok(Run::Done { events, bound, .. }) = prim::run(&cx, &effect) else {
            panic!("the step needs no answer");
        };
        let (mut creature, mut player) = (0, 0);
        for event in &events {
            match event {
                Event::DamageDealt {
                    target: DamageTarget::Object(_),
                    amount,
                    ..
                } => creature += amount,
                Event::DamageDealt {
                    target: DamageTarget::Player(_),
                    amount,
                    ..
                } => player += amount,
                _ => {}
            }
        }
        let kept = bound
            .iter()
            .find_map(|(name, _)| name.strip_prefix(LET).map(str::to_owned));
        (creature, player, kept)
    }

    #[rstest]
    #[case::beyond(2, 0, 5, r#","excess":{"as":"n"}"#, (5, 0, Some("3:n")))]
    #[case::exactly_lethal(2, 0, 2, r#","excess":{"as":"n"}"#, (2, 0, Some("0:n")))]
    #[case::short_of_lethal(4, 0, 2, r#","excess":{"as":"n"}"#, (2, 0, Some("0:n")))]
    #[case::damage_already_marked(4, 3, 2, r#","excess":{"as":"n"}"#, (2, 0, Some("1:n")))]
    #[case::sent_to_the_player(2, 0, 5, r#","excess":{"to":"You"}"#, (2, 3, None))]
    #[case::sent_and_counted(2, 0, 5, r#","excess":{"to":"You","as":"n"}"#, (2, 3, Some("3:n")))]
    #[case::short_of_lethal_sends_nothing(4, 0, 2, r#","excess":{"to":"You"}"#, (2, 0, None))]
    #[case::no_excess_asked(2, 0, 5, "", (5, 0, None))]
    fn damage_beyond_lethal_is_counted_or_sent_on(
        #[case] toughness: i32,
        #[case] marked: u32,
        #[case] amount: u32,
        #[case] excess: &str,
        #[case] want: (u32, u32, Option<&str>),
    ) {
        let json = format!(r#"{{"DealDamage":{{"to":"This","amount":{amount}{excess}}}}}"#);
        let (creature, player, kept) = run(&game(toughness, marked), &json);
        assert_eq!(
            (creature, player, kept),
            (want.0, want.1, want.2.map(str::to_owned))
        );
    }

    #[test]
    fn damage_to_a_player_has_no_excess() {
        let json = r#"{"DealDamage":{"to":"You","amount":5,"excess":{"as":"n"}}}"#;
        assert_eq!(run(&game(2, 0), json), (0, 5, Some("0:n".to_owned())));
    }
}
