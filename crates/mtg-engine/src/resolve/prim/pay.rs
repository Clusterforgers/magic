//! `Effect::Pay`: a player may pay a cost, and what follows depends on whether they do ("unless that player pays {1}",
//! "you may pay 2 life. If you do").

use std::iter::Peekable;

use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::Effect;
use mtg_cards::value::Amount;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use super::wrong_effect;
use crate::cast::payment_decision;
use crate::choose::{CardsOptions, Choice, YesNoOptions};
use crate::decision::{Decision, DecisionKind};
use crate::events::Event;
use crate::help::Offered;
use crate::mana::{can_afford, reserve_energy};
use crate::parts::{Part, Payer};
use crate::price::{Leaves, Price};
use crate::priority::mana_options;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::spend::Spend;
use crate::wording;

/// The players the effect names who are in the game are asked in turn order whether they will pay ("Pay {2}?", "Pay 2
/// life?", "Discard a card?"), until one does. A player who cannot pay, with the floating mana and the permanents they
/// may tap, the life, the counters, the energy and the objects the cost takes, is asked the same question at the same
/// time and can only say no (`YesNoOptions::only_no`), so neither the table nor the clock shows whether they could pay.
/// A yes leads to a `ChooseCards` for each part that picks among more objects than it takes, in the order the cost
/// prints them, and then the payment decision (`PayMana`, which cannot be backed out of) when the cost has mana; the
/// `paid` effects follow once everything is paid, with the events of the parts, and when nobody pays, the `unpaid`
/// effects follow.
///
/// A step with k questions runs k + 1 times. The journal holds, for each player asked, a no or a yes, then the picks
/// of a player who said yes and, for a cost with mana, a second yes that the payment's answer logs along with its
/// events (`cast::pay_answer`). The entries of a player who has left the game since are not read, and a player who left
/// before paying has not paid.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Pay {
        who, cost, times, ..
    } = effect
    else {
        return Err(wrong_effect("Pay"));
    };
    let times = usize::try_from(cx.scope.amount(times)?.max(0)).unwrap_or(usize::MAX);
    // A cost paid no times is paid, and nobody is asked.
    if times == 0 {
        return Ok(Run::Done {
            next: Next::Enter(0),
            events: Vec::new(),
            bound: Vec::new(),
            snapshots: Vec::new(),
        });
    }
    let price = price_of(&repeated(&c6_eg_counted(cx, cost)?, times))?;
    let state = cx.state();
    let mut journal = cx
        .journal()
        .iter()
        .filter(|entry| state.is_alive(entry.0))
        .peekable();
    let mut paid = None;
    for player in cx.scope.players(who)? {
        let Some((_, asked)) = journal.next_if(|entry| entry.0 == player) else {
            if state.is_alive(player) {
                return Ok(Run::Ask(question(cx, player, &price)));
            }
            continue;
        };
        match asked {
            Choice::YesNo(false) => {}
            Choice::YesNo(true) => match settle(cx, player, &price, &mut journal)? {
                Settled::Ask(decision) => return Ok(Run::Ask(decision)),
                Settled::Paid(events) => {
                    paid = Some(events);
                    break;
                }
            },
            other => return Err(unexpected(other, player)),
        }
    }
    if journal.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a question that was not asked".into(),
        ));
    }
    Ok(Run::Done {
        next: Next::Enter(usize::from(paid.is_none())),
        events: paid.unwrap_or_default(),
        bound: Vec::new(),
        snapshots: Vec::new(),
    })
}

/// The cost with the life of a counted amount ("pay that much life", "pay X life, where X is ...") counted as the
/// effect resolves (CR 119.4, CR 608.2h); a negative count pays nothing.
fn c6_eg_counted(cx: &Cx<'_>, cost: &Cost) -> Result<Cost, ResolveError> {
    let mut counted = cost.clone();
    for item in &mut counted.0 {
        if let CostItem::PayLife(amount) = item
            && !matches!(amount, Amount::Fixed(_) | Amount::X)
        {
            let count = cx.scope.amount(amount)?.clamp(0, i64::from(i32::MAX));
            *amount = Amount::Fixed(i32::try_from(count).unwrap_or(i32::MAX));
        }
    }
    Ok(counted)
}

/// The cost paid `times` times at once: the mana repeated so that it adds up, and the number of life, energy, counters
/// and objects every other part takes multiplied, so that the whole of it is asked, checked and paid as one part (two
/// parts of 1 life would each look payable with 1 life).
fn repeated(cost: &Cost, times: usize) -> Cost {
    let factor = i32::try_from(times).unwrap_or(i32::MAX);
    let times_over = |amount: &Amount| match amount {
        Amount::Fixed(number) => Amount::Fixed(number.saturating_mul(factor)),
        other => other.clone(),
    };
    Cost(
        cost.0
            .iter()
            .flat_map(|item| match item {
                CostItem::Mana(_) => vec![item.clone(); times],
                CostItem::PayLife(amount) => vec![CostItem::PayLife(times_over(amount))],
                CostItem::PayEnergy(amount) => vec![CostItem::PayEnergy(times_over(amount))],
                CostItem::RemoveCounters {
                    counter,
                    count,
                    from,
                } => vec![CostItem::RemoveCounters {
                    counter: counter.clone(),
                    count: times_over(count),
                    from: from.clone(),
                }],
                CostItem::Sacrifice { filter, count } => vec![CostItem::Sacrifice {
                    filter: filter.clone(),
                    count: times_over(count),
                }],
                CostItem::Discard { count, filter } => vec![CostItem::Discard {
                    count: times_over(count),
                    filter: filter.clone(),
                }],
                CostItem::ExileFromGraveyard { filter, count } => {
                    vec![CostItem::ExileFromGraveyard {
                        filter: filter.clone(),
                        count: times_over(count),
                    }]
                }
                CostItem::ExileFromHand { filter, count } => vec![CostItem::ExileFromHand {
                    filter: filter.clone(),
                    count: times_over(count),
                }],
                CostItem::ReturnToHand { filter, count } => vec![CostItem::ReturnToHand {
                    filter: filter.clone(),
                    count: times_over(count),
                }],
                CostItem::TapPermanents { filter, count } => vec![CostItem::TapPermanents {
                    filter: filter.clone(),
                    count: times_over(count),
                }],
                // These are refused by `price_of` however often they are repeated.
                other => vec![other.clone(); times],
            })
            .collect(),
    )
}

fn unexpected(answer: &Choice, player: PlayerId) -> ResolveError {
    ResolveError::Invalid(format!(
        "{answer:?} is no answer to the payment put to {player:?}"
    ))
}

/// The price of a cost an effect asks for: mana, and the parts the engine pays (life, counters removed from the source,
/// energy, objects picked). A cost that loses the source, taps it, has an `{X}` or counts X is refused by name: no
/// announcement chooses an X in the middle of a resolution.
fn price_of(cost: &Cost) -> Result<Price, ResolveError> {
    let price = Price::announced(cost).map_err(ResolveError::Unsupported)?;
    match price.leaves {
        // Losing the source is a cost of an ability; no effect asks a player to pay it yet.
        Leaves::Sacrificed => return Err(ResolveError::Unsupported("Effect.Pay.SacrificeSelf")),
        Leaves::Exiled => return Err(ResolveError::Unsupported("Effect.Pay.ExileSelf")),
        Leaves::Discarded => return Err(ResolveError::Unsupported("Effect.Pay.DiscardSelf")),
        Leaves::Stays => {}
    }
    if price.x > 0 {
        return Err(ResolveError::Unsupported("Cost.Mana.X"));
    }
    if price.counted() {
        return Err(ResolveError::Unsupported("Effect.Pay.X"));
    }
    if price.tap || (price.mana.is_none() && price.parts.is_empty()) {
        return Err(ResolveError::Invalid(
            "a payment is mana, life, counters, energy or objects picked".into(),
        ));
    }
    Ok(price)
}

/// Whether the player has the mana, the life, the counters, the energy and the objects for `price`: the mana floating or
/// from the permanents they may tap, other than the ones the other parts take.
fn can_pay(cx: &Cx<'_>, player: PlayerId, price: &Price) -> bool {
    let state = cx.state();
    let Some(claimed) = price.claimed(state, player, cx.scope.source.id) else {
        return false;
    };
    state.player(player).is_some_and(|seat| {
        price.mana.as_ref().is_none_or(|mana| {
            let mut sources = mana_options(state, player);
            crate::mana_takes::leave(&mut sources, &claimed);
            reserve_energy(&mut sources, price.energy());
            let life = seat
                .life()
                .saturating_sub(i32::try_from(price.life()).unwrap_or(i32::MAX));
            can_afford(mana, seat.mana(), &sources, life, &Spend::other())
        })
    })
}

/// "Pay {2}?", "Pay 2 life?", "Discard a card?": the parts of the cost as a card says them, with No the only answer when
/// the player cannot pay.
fn question(cx: &Cx<'_>, player: PlayerId, price: &Price) -> Decision {
    Decision {
        player,
        kind: DecisionKind::YesNo(YesNoOptions {
            only_no: !can_pay(cx, player, price),
            ..YesNoOptions::new(cx.scope.source.id, wording::pay(&said(price)))
        }),
    }
}

/// What the cost asks, part by part: "Pay {1}", "Pay 2 life", "Discard a card".
fn said(price: &Price) -> Vec<String> {
    price
        .mana
        .iter()
        .map(|mana| format!("Pay {mana}"))
        .chain(price.parts.iter().map(Part::text))
        .collect()
}

/// Where a player who said yes stands.
enum Settled {
    /// The next question: which objects to take, or the payment of the mana.
    Ask(Decision),
    /// Everything is paid, with these events.
    Paid(Vec<Event>),
}

/// The picks of the parts and the payment of the mana of a player who said yes, read from the journal in the order
/// they were asked.
fn settle<'a>(
    cx: &Cx<'_>,
    player: PlayerId,
    price: &Price,
    journal: &mut Peekable<impl Iterator<Item = &'a (PlayerId, Choice)>>,
) -> Result<Settled, ResolveError> {
    let state = cx.state();
    let source = cx.scope.source.id;
    let mut taken: Vec<Vec<ObjectId>> = Vec::new();
    for part in &price.parts {
        let Part::Pick(pick) = part else { continue };
        let barred = price.barred(part, source, &taken);
        let candidates = pick.candidates(&Payer {
            state,
            player,
            source,
            barred: &barred,
        });
        if candidates.len() < pick.count {
            return Err(ResolveError::Invalid(format!(
                "{player:?} said yes to a cost they cannot pay"
            )));
        }
        // Nothing to choose between: the part takes what there is.
        if candidates.len() == pick.count {
            taken.push(candidates);
            continue;
        }
        match journal.next_if(|entry| entry.0 == player) {
            Some((_, Choice::Cards(cards))) => taken.push(cards.clone()),
            None => {
                let count = u32::try_from(pick.count).unwrap_or(u32::MAX);
                return Ok(Settled::Ask(Decision {
                    player,
                    kind: DecisionKind::ChooseCards(CardsOptions {
                        source,
                        from: candidates,
                        min: count,
                        max: count,
                        text: pick.text(),
                        can_cancel: false,
                    }),
                }));
            }
            Some((_, other)) => return Err(unexpected(other, player)),
        }
    }
    if price.mana.is_some() {
        match journal.next_if(|entry| entry.0 == player) {
            Some((_, Choice::YesNo(true))) => {}
            None => {
                let picked: Vec<ObjectId> = taken.iter().flatten().copied().collect();
                return payment(cx, player, price, &picked).map(Settled::Ask);
            }
            Some((_, other)) => return Err(unexpected(other, player)),
        }
    }
    let mut picks = taken.iter();
    let mut events = Vec::new();
    for part in &price.parts {
        let picked = if matches!(part, Part::Pick(_)) {
            picks.next().map_or(&[][..], Vec::as_slice)
        } else {
            &[]
        };
        events.extend(part.events(state, player, source, picked));
    }
    Ok(Settled::Paid(events))
}

/// The payment decision of a player who said yes: no way back, since saying no was the way out. The permanents the other
/// parts take are not offered to tap, and the life they pay is not there to spend.
fn payment(
    cx: &Cx<'_>,
    player: PlayerId,
    price: &Price,
    picked: &[ObjectId],
) -> Result<Decision, ResolveError> {
    let state = cx.state();
    let mana = price.mana.clone().unwrap_or(ManaCost::ZERO);
    let mut sources = mana_options(state, player);
    crate::mana_takes::leave(&mut sources, picked);
    reserve_energy(&mut sources, price.energy());
    payment_decision(
        state,
        player,
        cx.scope.source.id,
        mana,
        sources,
        Offered::default(),
        Spend::other(),
        false,
        price.life(),
    )
    .map(|options| Decision {
        player,
        kind: DecisionKind::PayMana(options),
    })
    .ok_or_else(|| ResolveError::Invalid(format!("{player:?} is not in the game")))
}

#[cfg(test)]
mod tests {
    //! Goal: a payment asks the players it names in turn order "Pay {2}?" until one says yes and then for the payment itself (no way back), asks a player who cannot pay in the same words with No as the only answer (for every kind of mana cost: generic, coloured, hybrid, Phyrexian, floating, tapped and wrong-coloured sources), skips a player who left the game, enters the paid effects once a payment is logged and the unpaid ones when nobody paid, reads a player's answers back from the journal (paying spends what could pay), refuses answers it did not ask for, and refuses by name a cost the effect cannot ask (losing the source, tapping it, an X, a counted amount).
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::color::Color;
    use mtg_core::mana::ManaType;
    use rstest::rstest;

    use super::*;
    use crate::decision::PayOptions;
    use crate::events::Event;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::player::LossReason;
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    fn forest() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::basic_land(Some(Color::Green)))
    }

    /// Seat 0 controls the source and the first of `lands` Forests; seat `n` the `lands[n]`th.
    fn game(lands: &[usize]) -> GameState {
        let battlefield = |seat: usize, extra: Vec<Arc<CardDefinition>>| {
            let mut cards = vec![forest(); lands[seat]];
            cards.extend(extra);
            cards
        };
        let mut battlefields = vec![battlefield(0, vec![creature("Source", 1, 1)])];
        battlefields.extend((1..lands.len()).map(|seat| battlefield(seat, Vec::new())));
        state(vec![Vec::new(); lands.len()], battlefields)
    }

    fn effect(json: &str) -> Effect {
        serde_json::from_str(json).unwrap()
    }

    fn pay(who: &str, cost: &str) -> Effect {
        effect(&format!(
            r#"{{"Pay":{{"who":{who},"cost":{cost},"paid":[{{"Draw":{{}}}}],"unpaid":[{{"Draw":{{}}}}]}}}}"#
        ))
    }

    fn run_with(
        game: &GameState,
        journal: &[(usize, bool)],
        effect: &Effect,
    ) -> Result<Run, ResolveError> {
        let journal = journal
            .iter()
            .map(|(seat, yes)| (PlayerId::from_index(*seat), Choice::YesNo(*yes)))
            .collect();
        run_journal(game, journal, effect)
    }

    fn run_journal(
        game: &GameState,
        journal: Vec<(PlayerId, Choice)>,
        effect: &Effect,
    ) -> Result<Run, ResolveError> {
        let source = game
            .zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| game.object(*id))
            .find(|o| o.card().name == "Source")
            .unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, effect)
    }

    /// What the effect leaves: who is asked what, or where the resolution goes next.
    #[derive(Debug, PartialEq, Eq)]
    enum Said {
        Asks(usize, String),
        /// Asked in the same words, but No is the only answer.
        Declines(usize, String),
        Pays(usize, String, bool),
        Goes(Next),
    }

    fn said(run: Result<Run, ResolveError>) -> Said {
        match run.unwrap() {
            Run::Ask(Decision {
                player,
                kind: DecisionKind::YesNo(options),
            }) if options.only_no => Said::Declines(player.index(), options.text),
            Run::Ask(Decision {
                player,
                kind: DecisionKind::YesNo(options),
            }) => Said::Asks(player.index(), options.text),
            Run::Ask(Decision {
                player,
                kind: DecisionKind::PayMana(options),
            }) => Said::Pays(player.index(), options.cost.to_string(), options.can_cancel),
            Run::Ask(other) => panic!("unexpected question {other:?}"),
            Run::Cast(cast) => panic!("unexpected cast {cast:?}"),
            Run::Done {
                events,
                bound,
                snapshots,
                next,
            } => {
                assert!(events.is_empty() && bound.is_empty() && snapshots.is_empty());
                Said::Goes(next)
            }
        }
    }

    const PAID: Said = Said::Goes(Next::Enter(0));
    const UNPAID: Said = Said::Goes(Next::Enter(1));

    fn asks(seat: usize) -> Said {
        Said::Asks(seat, "Pay {2}?".into())
    }

    fn cannot(seat: usize) -> Said {
        Said::Declines(seat, "Pay {2}?".into())
    }

    #[test]
    fn the_player_is_asked_in_the_words_of_the_card_and_then_for_the_payment_without_a_way_back() {
        let game = game(&[2, 0]);
        let pay = pay(r#""You""#, r#""{2}""#);
        assert_eq!(said(run_with(&game, &[], &pay)), asks(0));
        assert_eq!(
            said(run_with(&game, &[(0, true)], &pay)),
            Said::Pays(0, "{2}".into(), false)
        );
    }

    #[test]
    fn the_payment_decision_offers_the_sources_of_the_payer_for_the_source_of_the_effect() {
        let game = game(&[2, 0]);
        let Ok(Run::Ask(Decision {
            kind: DecisionKind::PayMana(options),
            ..
        })) = run_with(&game, &[(0, true)], &pay(r#""You""#, r#""{2}""#))
        else {
            panic!("a payment was expected");
        };
        let PayOptions {
            spell,
            sources,
            auto_available,
            can_cancel,
            ..
        } = options;
        assert_eq!(game.object(spell).unwrap().card().name, "Source");
        assert_eq!(sources.len(), 2);
        assert!(auto_available && !can_cancel);
    }

    #[rstest]
    #[case::no(&[(0, false)], UNPAID)]
    #[case::paid(&[(0, true), (0, true)], PAID)]
    fn the_answers_decide_which_effects_follow(
        #[case] given: &[(usize, bool)],
        #[case] said_: Said,
    ) {
        // The payment spent the lands, so the player could not pay again: the answers are read first.
        let game = game(&[0, 0]);
        assert_eq!(
            said(run_with(&game, given, &pay(r#""You""#, r#""{2}""#))),
            said_
        );
    }

    /// What seat 0 has to pay with: untapped Forests, Forests that are tapped, green mana floating and life lost.
    #[derive(Clone, Copy)]
    struct Means {
        untapped: usize,
        tapped: usize,
        floating: u32,
        life_lost: u32,
    }

    const NOTHING: Means = Means {
        untapped: 0,
        tapped: 0,
        floating: 0,
        life_lost: 0,
    };

    fn means(means: Means) -> GameState {
        let mut game = game(&[means.untapped + means.tapped, 0]);
        let lands: Vec<_> = game
            .zone(Zone::Battlefield)
            .iter()
            .copied()
            .filter(|id| game.object(*id).unwrap().card().name == "Forest")
            .collect();
        let events = lands
            .into_iter()
            .skip(means.untapped)
            .map(|object| Event::PermanentTapped { object })
            .chain((0..means.floating).map(|_| Event::ManaAdded {
                player: ME,
                mana: ManaType::Green,
                source: None,
                only: Vec::new(),
                lasts: mtg_cards::spend::ManaLasts::Step,
            }))
            .chain((means.life_lost > 0).then_some(Event::LifeLost {
                player: ME,
                amount: means.life_lost,
            }));
        for event in events {
            apply_event(&mut game, &event).unwrap();
        }
        game
    }

    #[rstest]
    // Generic mana: lands, floating mana, or both.
    #[case::two_lands_for_two("{2}", Means { untapped: 2, ..NOTHING }, true)]
    #[case::one_land_for_two("{2}", Means { untapped: 1, ..NOTHING }, false)]
    #[case::no_land_for_two("{2}", NOTHING, false)]
    #[case::floating_mana_pays("{2}", Means { floating: 2, ..NOTHING }, true)]
    #[case::a_land_and_floating_mana_pay("{2}", Means { untapped: 1, floating: 1, ..NOTHING }, true)]
    #[case::too_little_of_both("{3}", Means { untapped: 1, floating: 1, ..NOTHING }, false)]
    // Coloured mana: the colour is needed.
    #[case::the_colour_from_lands("{G}{G}", Means { untapped: 2, ..NOTHING }, true)]
    #[case::the_colour_floating("{G}", Means { floating: 1, ..NOTHING }, true)]
    #[case::the_wrong_colour("{R}", Means { untapped: 2, ..NOTHING }, false)]
    #[case::generic_met_but_not_the_colour("{1}{R}", Means { untapped: 2, ..NOTHING }, false)]
    // Lands that are tapped make nothing.
    #[case::only_tapped_lands("{1}", Means { tapped: 2, ..NOTHING }, false)]
    #[case::a_tapped_land_beside_an_untapped_one("{2}", Means { untapped: 1, tapped: 1, ..NOTHING }, false)]
    // Hybrid: either colour, or two generic.
    #[case::hybrid_with_one_of_its_colours("{R/G}", Means { untapped: 1, ..NOTHING }, true)]
    #[case::hybrid_with_neither_colour("{R/W}", Means { untapped: 2, ..NOTHING }, false)]
    #[case::two_generic_for_a_hybrid_symbol("{2/R}", Means { untapped: 2, ..NOTHING }, true)]
    #[case::one_land_for_a_two_generic_hybrid_symbol("{2/R}", Means { untapped: 1, ..NOTHING }, false)]
    // Phyrexian: the colour or two life, and a payment may take a player to 0 life (CR 119.4).
    #[case::phyrexian_with_the_colour("{G/P}", Means { untapped: 1, ..NOTHING }, true)]
    #[case::phyrexian_with_life("{R/P}", NOTHING, true)]
    #[case::phyrexian_with_exactly_the_life("{R/P}", Means { life_lost: 18, ..NOTHING }, true)]
    #[case::phyrexian_with_too_little_life("{R/P}", Means { life_lost: 19, ..NOTHING }, false)]
    #[case::two_phyrexian_symbols_with_the_life("{R/P}{R/P}", Means { life_lost: 16, ..NOTHING }, true)]
    #[case::two_phyrexian_symbols_one_life_short("{R/P}{R/P}", Means { life_lost: 17, ..NOTHING }, false)]
    #[case::phyrexian_with_a_land_and_life_for_the_rest("{R/P}{G/P}", Means { untapped: 1, life_lost: 18, ..NOTHING }, true)]
    fn a_player_who_cannot_pay_is_asked_the_same_question_with_no_the_only_answer(
        #[case] cost: &str,
        #[case] having: Means,
        #[case] can_pay: bool,
    ) {
        let game = means(having);
        let text = format!("Pay {cost}?");
        let expected = if can_pay {
            Said::Asks(0, text)
        } else {
            Said::Declines(0, text)
        };
        let effect = pay(r#""You""#, &format!(r#""{cost}""#));
        assert_eq!(said(run_with(&game, &[], &effect)), expected);
    }

    #[rstest]
    #[case::can_pay(Means { untapped: 2, ..NOTHING }, false)]
    #[case::cannot_pay(NOTHING, true)]
    fn the_source_and_the_words_are_the_same_whether_or_not_the_player_can_pay(
        #[case] having: Means,
        #[case] only_no: bool,
    ) {
        let game = means(having);
        let Ok(Run::Ask(Decision { player, kind })) =
            run_with(&game, &[], &pay(r#""You""#, r#""{2}""#))
        else {
            panic!("the player is asked");
        };
        assert_eq!(player, ME);
        let DecisionKind::YesNo(options) = kind else {
            panic!("a yes-or-no question is asked");
        };
        let source = game
            .zone(Zone::Battlefield)
            .iter()
            .find(|id| game.object(**id).unwrap().card().name == "Source")
            .unwrap();
        assert_eq!(options.source, *source);
        assert_eq!(options.text, "Pay {2}?");
        assert_eq!(options.only_no, only_no);
    }

    #[test]
    fn a_no_from_a_player_who_cannot_pay_leads_to_the_unpaid_effects() {
        let game = game(&[0, 0]);
        let effect = pay(r#""You""#, r#""{2}""#);
        assert_eq!(said(run_with(&game, &[], &effect)), cannot(0));
        assert_eq!(said(run_with(&game, &[(0, false)], &effect)), UNPAID);
    }

    #[rstest]
    #[case::none_asks_nothing_and_has_paid("{1}", "0", 0, &[], PAID)]
    #[case::none_of_a_cost_nobody_can_pay("{9}", "0", 0, &[], PAID)]
    #[case::once("{1}", "1", 2, &[], Said::Asks(0, "Pay {1}?".into()))]
    #[case::twice("{1}", "2", 2, &[], Said::Asks(0, "Pay {2}?".into()))]
    #[case::three_times_with_two_lands("{1}", "3", 2, &[], Said::Declines(0, "Pay {3}?".into()))]
    #[case::two_colours_twice("{G}{1}", "2", 4, &[], Said::Asks(0, "Pay {2}{G}{G}?".into()))]
    #[case::life_twice("[{\"PayLife\":1}]", "2", 0, &[], Said::Asks(0, "Pay 2 life?".into()))]
    #[case::c6_eg_counted_life("[{\"PayLife\":{\"Life\":\"You\"}}]", "1", 0, &[], Said::Asks(0, "Pay 20 life?".into()))]
    #[case::c6_eg_counted_life_twice("[{\"PayLife\":{\"Life\":\"You\"}}]", "2", 0, &[], Said::Declines(0, "Pay 40 life?".into()))]
    #[case::yes_to_twice("{1}", "2", 2, &[(0, true)], Said::Pays(0, "{2}".into(), false))]
    #[case::paid_twice("{1}", "2", 2, &[(0, true), (0, true)], PAID)]
    #[case::declined("{1}", "2", 2, &[(0, false)], UNPAID)]
    fn a_cost_paid_several_times_is_asked_as_all_of_them_together_and_none_is_paid_without_a_question(
        #[case] cost: &str,
        #[case] times: &str,
        #[case] lands: usize,
        #[case] given: &[(usize, bool)],
        #[case] said_: Said,
    ) {
        let cost = if cost.starts_with('[') {
            cost.to_owned()
        } else {
            format!("\"{cost}\"")
        };
        let effect = effect(&format!(
            r#"{{"Pay":{{"cost":{cost},"times":{times},"paid":[{{"Draw":{{}}}}],"unpaid":[{{"Draw":{{}}}}]}}}}"#
        ));
        let game = game(&[lands, 0]);
        assert_eq!(said(run_with(&game, given, &effect)), said_);
    }

    #[rstest]
    #[case::one_life_once_with_one("1", 19, Said::Asks(0, "Pay 1 life?".into()))]
    #[case::one_life_twice_with_one("2", 19, Said::Declines(0, "Pay 2 life?".into()))]
    #[case::one_life_twice_with_two("2", 18, Said::Asks(0, "Pay 2 life?".into()))]
    #[case::one_life_three_times_with_two("3", 18, Said::Declines(0, "Pay 3 life?".into()))]
    fn a_cost_in_life_paid_several_times_is_payable_only_with_the_life_for_all_of_it(
        #[case] times: &str,
        #[case] life_lost: u32,
        #[case] said_: Said,
    ) {
        let effect = effect(&format!(
            r#"{{"Pay":{{"cost":[{{"PayLife":1}}],"times":{times},"paid":[{{"Draw":{{}}}}],"unpaid":[{{"Draw":{{}}}}]}}}}"#
        ));
        let game = means(Means {
            life_lost,
            ..NOTHING
        });
        assert_eq!(said(run_with(&game, &[], &effect)), said_);
    }

    #[rstest]
    #[case::the_first_is_asked(&[3, 3, 3, 3], &[], asks(1))]
    #[case::the_first_says_no(&[3, 3, 3, 3], &[(1, false)], asks(2))]
    #[case::two_say_no(&[3, 3, 3, 3], &[(1, false), (2, false)], asks(3))]
    #[case::all_say_no(&[3, 3, 3, 3], &[(1, false), (2, false), (3, false)], UNPAID)]
    #[case::the_second_pays(&[3, 3, 3, 3], &[(1, false), (2, true)], Said::Pays(2, "{2}".into(), false))]
    #[case::the_second_has_paid(&[3, 3, 3, 3], &[(1, false), (2, true), (2, true)], PAID)]
    #[case::the_first_pays_and_nobody_else_is_asked(&[3, 3, 3, 3], &[(1, true), (1, true)], PAID)]
    #[case::the_first_cannot_pay_and_is_asked_all_the_same(&[3, 0, 3, 3], &[], cannot(1))]
    #[case::the_next_is_asked_after_a_player_who_could_not_pay_said_no(&[3, 0, 3, 3], &[(1, false)], asks(2))]
    #[case::a_player_who_cannot_pay_is_asked_in_turn(&[3, 3, 0, 3], &[(1, false)], cannot(2))]
    #[case::and_the_one_after_him_is_asked_next(&[3, 3, 0, 3], &[(1, false), (2, false)], asks(3))]
    #[case::none_can_pay_and_all_are_asked(&[3, 0, 1, 1], &[], cannot(1))]
    #[case::none_can_pay_and_all_say_no(&[3, 0, 1, 1], &[(1, false), (2, false), (3, false)], UNPAID)]
    fn several_players_are_asked_in_turn_order_until_one_pays(
        #[case] lands: &[usize],
        #[case] given: &[(usize, bool)],
        #[case] said_: Said,
    ) {
        let game = game(lands);
        let opponents = pay(r#""Opponents""#, r#""{2}""#);
        assert_eq!(said(run_with(&game, given, &opponents)), said_);
    }

    #[test]
    fn a_player_who_left_the_game_is_not_asked_and_has_not_paid() {
        let mut game = game(&[0, 3, 3]);
        let opponents = pay(r#""Opponents""#, r#""{2}""#);
        assert_eq!(said(run_with(&game, &[], &opponents)), asks(1));
        apply_event(
            &mut game,
            &Event::PlayerLost {
                player: PlayerId(1),
                reason: LossReason::Conceded,
            },
        )
        .unwrap();
        assert_eq!(said(run_with(&game, &[], &opponents)), asks(2));
        // Said yes and left before paying: their answer is not read, the payment is not made and the next player is
        // asked; the answers of the others are read as before.
        assert_eq!(said(run_with(&game, &[(1, true)], &opponents)), asks(2));
        assert_eq!(
            said(run_with(&game, &[(1, true), (2, false)], &opponents)),
            UNPAID
        );
        assert_eq!(
            said(run_with(&game, &[(1, true), (2, true)], &opponents)),
            Said::Pays(2, "{2}".into(), false)
        );
    }

    #[test]
    fn nobody_is_asked_when_nobody_is_named() {
        // A target that was never chosen names nobody: nothing was paid.
        let game = game(&[3, 3]);
        assert_eq!(
            said(run_with(&game, &[], &pay(r#"{"Target":0}"#, r#""{2}""#))),
            UNPAID
        );
    }

    #[rstest]
    #[case::wrong_kind(vec![(ME, Choice::Number(1))])]
    #[case::a_no_with_a_payment(vec![(ME, Choice::YesNo(false)), (ME, Choice::YesNo(true))])]
    #[case::an_answer_after_the_payment(vec![(ME, Choice::YesNo(true)), (ME, Choice::YesNo(true)), (ME, Choice::YesNo(true))])]
    #[case::a_yes_then_another_kind(vec![(ME, Choice::YesNo(true)), (ME, Choice::Option(0))])]
    fn an_answer_to_a_question_that_was_not_asked_is_refused(
        #[case] journal: Vec<(PlayerId, Choice)>,
    ) {
        let game = game(&[2, 2]);
        let error = run_journal(&game, journal, &pay(r#""You""#, r#""{2}""#)).unwrap_err();
        assert!(matches!(error, ResolveError::Invalid(_)), "{error:?}");
    }

    #[test]
    fn an_answer_of_a_player_the_effect_does_not_name_is_refused() {
        // The player named said no, so the answer of the other seat answers nothing.
        let game = game(&[0, 2]);
        let journal = [(0, false), (1, true)];
        let error = run_with(&game, &journal, &pay(r#""You""#, r#""{2}""#)).unwrap_err();
        assert!(matches!(error, ResolveError::Invalid(_)), "{error:?}");
    }

    #[rstest]
    #[case::counted_life(r#"[{"PayLife":"X"}]"#, ResolveError::Unsupported("Effect.Pay.X"))]
    #[case::counted_mana_and_energy(
        r#"[{"Mana":"{1}"},{"PayEnergy":"X"}]"#,
        ResolveError::Unsupported("Effect.Pay.X")
    )]
    #[case::sacrifice(
        r#"["SacrificeSelf"]"#,
        ResolveError::Unsupported("Effect.Pay.SacrificeSelf")
    )]
    #[case::mana_and_sacrifice(
        r#"[{"Mana":"{2}"},"SacrificeSelf"]"#,
        ResolveError::Unsupported("Effect.Pay.SacrificeSelf")
    )]
    #[case::x(r#""{X}""#, ResolveError::Unsupported("Cost.Mana.X"))]
    #[case::tapping(
        r#"[{"Mana":"{1}"},"Tap"]"#,
        ResolveError::Invalid(
            "a payment is mana, life, counters, energy or objects picked".into()
        )
    )]
    #[case::nothing("[]", ResolveError::Invalid(
            "a payment is mana, life, counters, energy or objects picked".into()
        ))]
    fn a_cost_the_effect_cannot_ask_is_refused_by_name(
        #[case] cost: &str,
        #[case] expected: ResolveError,
    ) {
        let game = game(&[2, 0]);
        let error = run_with(&game, &[], &pay(r#""You""#, cost)).unwrap_err();
        assert_eq!(error, expected);
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        let game = game(&[0, 0]);
        let error = run_with(&game, &[], &effect(r#"{"Draw":{}}"#)).unwrap_err();
        assert_eq!(
            error,
            ResolveError::Invalid("the Pay primitive was given another effect".into())
        );
    }
}
