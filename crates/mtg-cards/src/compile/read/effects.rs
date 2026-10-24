//! Effects: a sentence is read into the effects of one template. A template reads the whole
//! sentence or nothing; to add a wording, write a template and list it in [`TEMPLATES`].

use mtg_core::card::Subtype;
use mtg_core::mana::ManaType;

use super::among;
use super::amounts::{
    cards_counted, counted, counted_parts, for_each, its_stat, number_word, points,
};
use super::as_long_as;
use super::attackers;
use super::cast;
use super::conditional;
use super::copies;
use super::copy_spells;
use super::costs;
use super::counters;
use super::create;
use super::defining;
use super::draws;
use super::library;
use super::modal;
use super::payments;
use super::prevention;
use super::selectors::{Form, all, each, granted_keywords, phrase, target};
use super::spend;
use super::tokens::Reader;
use super::where_x;
use crate::cost::Cost;
use crate::effect::{Destination, Effect, Order, Place, Produce};
use crate::mana::ManaCostX;
use crate::modify::{Duration, Modification};
use crate::refs::{Bounds, Pred, Ref, Selector, Targetable, ZoneKind};
use crate::value::{Amount, Condition, CounterKind};

/// A template: the effects of the sentence it reads, or `None` when the sentence is not its shape.
pub(super) type Template = fn(&mut Reader<'_>) -> Option<Vec<Effect>>;

/// The sentence shapes, tried in this order.
const TEMPLATES: [Template; 57] = [
    modal::modal,
    super::spent::sacrifice_unless,
    super::actions::fight,
    super::actions::attach,
    cast::discover_sentence,
    cast::exile_until,
    super::reveal_until::reveal_until,
    cast::exile_top,
    cast::exiled_card,
    payments::may_pay,
    may,
    create::chained,
    chain,
    put_back,
    super::actions::explore,
    super::actions::connive,
    super::actions::goad,
    super::manifest::manifest,
    draws::wheel,
    player_effect,
    super::damage::damage,
    super::zone_moves::exile_graveyard,
    super::random_moves::from_graveyard,
    removal,
    super::flicker::exile_and_return,
    super::becomes::creature,
    super::becomes::animate,
    super::changes::pump,
    evasion,
    super::zone_moves::from_graveyard,
    super::zone_moves::may_put_from_hand,
    return_card,
    return_chosen,
    return_any_number,
    copies::token_copy,
    copies::populate,
    copy_spells::copy,
    copy_spells::becomes_copy,
    copy_spells::choose_and_become,
    create::tokens,
    counters::put,
    counters::remove,
    counters::moving,
    counters::multiply,
    add_mana,
    library::search,
    library::search_by_controller,
    among::among,
    library::look,
    library::put_on_library,
    super::mass_moves::put_all,
    super::mass_moves::return_all,
    return_itself,
    shuffle_itself,
    prevention::rule_effect,
    prevention::shield,
    super::attack_tax::rule_effect,
];

/// The shapes that begin with a verb ("you" is the subject): "you may" can stand before each.
pub(super) const IMPERATIVES: [Template; 16] = [
    super::actions::goad,
    super::manifest::manifest,
    chain,
    removal,
    super::zone_moves::from_graveyard,
    return_card,
    return_chosen,
    copies::token_copy,
    copy_spells::copy,
    create::tokens,
    counters::put,
    counters::remove,
    counters::moving,
    counters::multiply,
    library::put_on_library,
    return_itself,
];

/// Every sentence shape, in the order they are tried: the shapes of `conditional` first, which read a sentence
/// that another shape would read in part.
fn templates() -> impl Iterator<Item = Template> {
    conditional::TEMPLATES
        .into_iter()
        .chain(TEMPLATES)
        .chain(super::lands::TEMPLATES)
}

/// Reads the next sentence and its period into `effects`; false when it is not a shape the
/// templates read.
pub fn sentence(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> bool {
    // A where clause at the end of the sentence gives the X inside it its amount.
    let named = where_x::ahead(r);
    let outer = std::mem::replace(&mut r.where_x, named);
    let read = sentence_shapes(r, effects);
    r.where_x = outer;
    read
}

/// The sentence shapes of `sentence`, tried in order.
fn sentence_shapes(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> bool {
    if r.attempt(|r| cast::after_exiling(r, effects)).is_some() {
        return true;
    }
    if r.attempt(|r| regeneration(r, effects)).is_some() {
        return true;
    }
    if r.attempt(|r| payments::after_may(r, effects)).is_some() {
        return true;
    }
    if r.attempt(|r| attackers::does_the_same(r, effects))
        .is_some()
    {
        return true;
    }
    if r.attempt(|r| super::token_binds::after_tokens(r, effects))
        .is_some()
    {
        return true;
    }
    for template in templates().chain([unless_pays as Template]) {
        let read = r.attempt(|r| {
            let mut found = template(r)?;
            // ", then populate" follows any sentence.
            found.extend(r.attempt(copies::then_populate).unwrap_or_default());
            if r.where_x {
                found = where_x::bind(r, &found)?;
            }
            r.sentence_end().then_some(found)
        });
        if let Some(found) = read {
            effects.extend(found);
            return true;
        }
    }
    false
}

/// "Counter target spell unless its controller pays {2}.", "You may draw a card unless that player
/// pays {1}.": the sentence before "unless" is what happens when the player does not pay.
fn unless_pays(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    templates().find_map(|template| {
        r.attempt(|r| {
            let unpaid = template(r)?;
            r.need("unless")?;
            let (who, cost) = payments::unless(r, &unpaid)?;
            Some(vec![Effect::Pay {
                who,
                cost,
                times: Amount::one(),
                paid: Vec::new(),
                unpaid,
            }])
        })
    })
}

/// The effects of the next sentence, up to its period.
pub(super) fn one_sentence(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    templates().find_map(|template| r.attempt(template))
}

/// The mana of a payment: `{2}`, `{U}{R}`; an `{X}` is not the payer's to choose.
pub(super) fn mana_cost(r: &mut Reader<'_>) -> Option<Cost> {
    let mut text = String::new();
    while let Some(symbol) = r.symbol() {
        text.push('{');
        text.push_str(symbol);
        text.push('}');
    }
    let cost: ManaCostX = text.parse().ok()?;
    (!text.is_empty() && !cost.has_x()).then(|| Cost::mana(cost))
}

/// "It can't be regenerated." changes the destruction before it.
fn regeneration(r: &mut Reader<'_>, effects: &mut [Effect]) -> Option<()> {
    let Some(Effect::Destroy { what, regenerate }) = effects.last_mut() else {
        return None;
    };
    let single = matches!(what, Ref::Target(_)) && r.targets.first()?.bounds.is_one();
    // One creature destroyed may be called "that creature" (Stormscape Battlemage).
    if !(single && r.words("that creature can't be regenerated")) {
        r.need(if single {
            "it can't be regenerated"
        } else {
            "they can't be regenerated"
        })?;
    }
    if !r.sentence_end() || !*regenerate {
        return None;
    }
    *regenerate = false;
    Some(())
}

/// "You may draw a card.": one step that the controller may do.
fn may(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("you may")?;
    if let Some(life) = r.attempt(may_life) {
        return Some(life);
    }
    IMPERATIVES
        .iter()
        .chain(&conditional::IMPERATIVES)
        .find_map(|template| {
            r.attempt(|r| {
                let found = template(r)?;
                // A chain of steps is not one choice: "may" would have to say which steps it covers.
                (found.len() == 1).then(|| {
                    vec![Effect::May {
                        who: Ref::You,
                        effects: found,
                    }]
                })
            })
        })
}

/// "you may gain 1 life", "you may lose 1 life": the life change is the one choice.
fn may_life(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let effect = match r.take_word()? {
        "gain" => Effect::GainLife {
            who: Ref::You,
            amount: life(r)?,
        },
        "lose" => Effect::LoseLife {
            who: Ref::You,
            amount: life(r)?,
        },
        _ => return None,
    };
    Some(vec![Effect::May {
        who: Ref::You,
        effects: vec![effect],
    }])
}

/// Steps of "you" joined by ", then": "Scry 1, then draw a card."
fn chain(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let mut effects = vec![imperative(r)?];
    while let Some(next) = r.attempt(|r| {
        r.punct(',').then_some(())?;
        r.need("then")?;
        match r.attempt(imperative) {
            Some(step) => Some(vec![step]),
            None => put_back(r).or_else(|| after_mill(r, &effects)),
        }
    }) {
        effects.extend(next);
    }
    Some(effects)
}

/// "Mill two cards, then ~ gets +X/+X until end of turn": the change of the card itself that follows a mill, which
/// counts what the mill left in the graveyard.
fn after_mill(r: &mut Reader<'_>, before: &[Effect]) -> Option<Vec<Effect>> {
    matches!(before, [Effect::Mill { .. }]).then_some(())?;
    let step = super::changes::pump(r)?;
    matches!(
        step.as_slice(),
        [Effect::Continuous {
            affected: Ref::This,
            ..
        }]
    )
    .then_some(step)
}

/// "Put two cards from your hand on top of your library in any order.": the player chooses cards
/// of their hand, bound as `back`, and puts them on their library. One card has no order to choose.
fn put_back(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("put")?;
    let number = number_word(r.take_word()?)?;
    r.word(if number == 1 { "card" } else { "cards" })
        .then_some(())?;
    r.need("from your hand on top of your library")?;
    let order = if number == 1 {
        None
    } else {
        r.need("in any order")?;
        Some(Order::Chosen)
    };
    let count = Bounds::exactly(&Amount::Fixed(i32::try_from(number).ok()?));
    let from = Selector(vec![Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)]);
    Some(vec![
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(from),
            count,
            bind: BACK.to_owned(),
            rest_as: None,
        },
        Effect::Move {
            what: Ref::Bound(BACK.to_owned()),
            to: Destination::Library {
                place: Place::Top,
                order,
            },
        },
    ])
}

/// The name the cards put back are bound to.
pub const BACK: &str = "back";

/// One step of "you" that begins with its verb: "scry 1", "draw a card", "mill three cards".
pub(super) fn imperative(r: &mut Reader<'_>) -> Option<Effect> {
    let who = Ref::You;
    Some(match r.take_word()? {
        "scry" => Effect::Scry {
            who,
            count: points(r)?,
        },
        "surveil" => Effect::Surveil {
            who,
            count: points(r)?,
        },
        "proliferate" => Effect::Proliferate { who },
        "learn" => Effect::Learn { who },
        "monstrosity" => Effect::Monstrosity { count: points(r)? },
        "adapt" => Effect::Adapt { count: points(r)? },
        "bolster" => Effect::Bolster { count: points(r)? },
        "amass" => Effect::Amass {
            subtype: Subtype(super::selectors::plural_subtype(r)?),
            count: points(r)?,
        },
        "sacrifice" => itself_sacrificed(r, who)?,
        "draw" => Effect::Draw {
            who,
            count: draws::drawn(r)?,
        },
        "mill" => Effect::Mill {
            who,
            count: cards_counted(r)?,
        },
        "shuffle" => {
            r.need("your library")?;
            Effect::Shuffle { who }
        }
        "discard" => {
            let count = draws::discarded(r, &who)?;
            let random = r.words("at random");
            Effect::Discard { who, count, random }
        }
        _ => return None,
    })
}

/// What follows "sacrifice" when the card sacrifices itself: "sacrifice ~", and "sacrifice it" once the ability has
/// spoken of the card.
fn itself_sacrificed(r: &mut Reader<'_>, who: Ref) -> Option<Effect> {
    // "When ~ enters, sacrifice it unless you pay {1}": the card, since nothing else has entered.
    let itself = r.it == Some(Ref::This) || (r.acts_itself && r.word_at(1) == Some("unless"));
    (r.source() || (itself && r.word("it"))).then_some(())?;
    Some(Effect::Sacrifice {
        who,
        filter: Selector::this(),
        count: Amount::one(),
    })
}

/// "sacrifice ~" or "sacrifice it": the card sacrifices itself.
pub(super) fn sacrifice_itself(r: &mut Reader<'_>) -> Option<Effect> {
    r.need("sacrifice")?;
    itself_sacrificed(r, Ref::You)
}

/// The player an effect is about: "you", "target player", "each opponent", "its controller".
pub(super) fn player(r: &mut Reader<'_>) -> Option<Ref> {
    if r.word("you") {
        return Some(Ref::You);
    }
    if r.words("each opponent") {
        return Some(Ref::Opponents);
    }
    if r.words("each player") {
        return Some(Ref::EachPlayer);
    }
    if r.words("its controller")
        || (r.it.is_some()
            && (r.words("that creature's controller") || r.words("that land's controller")))
    {
        return r.it.clone().map(|it| Ref::ControllerOf(Box::new(it)));
    }
    if r.that_player && (r.words("that player") || r.word("they")) {
        return Some(Ref::Triggering);
    }
    if r.that_permanent && r.words("that permanent's controller") {
        return Some(Ref::ControllerOf(Box::new(Ref::Triggering)));
    }
    r.attempt(|r| {
        let who = target(r)?;
        let spec = r.targets.first()?;
        (matches!(spec.what, Targetable::Player | Targetable::Opponent) && spec.bounds.is_one())
            .then_some(who)
    })
}

/// "gain 3 life", "lose X life", "gain life equal to its power": what follows the verb.
pub(super) fn life(r: &mut Reader<'_>) -> Option<Amount> {
    if r.word("life") {
        r.need("equal to")?;
        return r.attempt(its_stat).or_else(|| defining::value(r));
    }
    let amount = points(r)?;
    r.word("life").then_some(())?;
    counted(r, amount)
}

/// A player draws, discards, mills, gains, loses or sacrifices; one subject may have several verb
/// phrases, joined by "and" or in a list: "target opponent sacrifices a creature, discards a card,
/// and loses 3 life". The player the event is about may act after "and" ("you create a Wolf token and that player mills
/// ten cards"), and so may you after another player ("target player loses 2 life and you gain 2 life") or after
/// yourself ("you draw a card and you lose 1 life"). The effects are those of the sentence, in order.
fn player_effect(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let who = player(r)?;
    let mut effects = phrases(r, &who)?;
    // The sentences of two other players stay with the table.
    while let Some(more) = r.attempt(|r| {
        r.need("and")?;
        let next = if r.that_player && (r.words("that player") || r.word("they")) {
            Ref::Triggering
        } else if r.word("you") {
            Ref::You
        } else {
            return None;
        };
        phrases(r, &next)
    }) {
        effects.extend(more);
    }
    Some(effects)
}

/// The verb phrases of one player, joined by "and" or in a list.
fn phrases(r: &mut Reader<'_>, who: &Ref) -> Option<Vec<Effect>> {
    let mut effects = verb_phrase(r, who)?;
    while let Some(next) = r.attempt(|r| {
        let listed = r.punct(',');
        let joined = r.word("and");
        (listed || joined).then_some(())?;
        verb_phrase(r, who)
    }) {
        effects.extend(next);
    }
    // "draws a card and loses 1 life for each creature": which verb phrases the count belongs to is not the card's to say.
    let counts = |effect: &Effect| {
        matches!(
            effect,
            Effect::Draw { count, .. }
                | Effect::Mill { count, .. }
                | Effect::GainLife { amount: count, .. }
                | Effect::LoseLife { amount: count, .. }
                if counted_parts(count).is_some()
        )
    };
    if effects.len() > 1 && effects.iter().any(counts) {
        return None;
    }
    Some(effects)
}

/// One verb phrase: what a player does, or "create a token" for you.
fn verb_phrase(r: &mut Reader<'_>, who: &Ref) -> Option<Vec<Effect>> {
    if matches!(r.peek_word(), Some("create" | "creates")) {
        return create::made_by(r, who);
    }
    // The controller of a permanent the sentence before named investigates: "Its controller investigates."
    if matches!(who, Ref::ControllerOf(_)) && r.word("investigates") {
        return Some(vec![create::clue(who.clone())]);
    }
    predicate(r, who).map(|effect| vec![effect])
}

/// What a player does: the verb and its object, without the subject.
fn predicate(r: &mut Reader<'_>, who: &Ref) -> Option<Effect> {
    let who = who.clone();
    Some(match r.take_word()? {
        "draw" | "draws" => Effect::Draw {
            who,
            count: draws::drawn(r)?,
        },
        "mill" | "mills" => Effect::Mill {
            who,
            count: cards_counted(r)?,
        },
        "discard" | "discards" => {
            let count = draws::discarded(r, &who)?;
            let random = r.words("at random");
            Effect::Discard { who, count, random }
        }
        "gain" | "gains" => Effect::GainLife {
            who,
            amount: life(r)?,
        },
        "lose" | "loses" => Effect::LoseLife {
            who,
            amount: life(r)?,
        },
        "scry" if who == Ref::You => Effect::Scry {
            who,
            count: points(r)?,
        },
        "surveil" if who == Ref::You => Effect::Surveil {
            who,
            count: points(r)?,
        },
        "win" | "wins" => {
            r.need("the game")?;
            Effect::WinGame { who }
        }
        "become" | "becomes" => {
            r.need("the monarch")?;
            Effect::BecomeMonarch { who }
        }
        "shuffle" | "shuffles" => {
            r.need(if who == Ref::You {
                "your library"
            } else {
                "their library"
            })?;
            Effect::Shuffle { who }
        }
        "sacrifice" | "sacrifices" => sacrifice(r, who)?,
        // "you get {E}{E}": energy counters, written with their symbol.
        "get" | "gets" => Effect::PutCounters {
            on: who,
            counter: CounterKind::Other("energy".to_owned()),
            count: costs::energy(r)?,
        },
        _ => return None,
    })
}

/// "a creature or planeswalker" and "of their choice": the player chooses, as they always do.
fn sacrifice(r: &mut Reader<'_>, who: Ref) -> Option<Effect> {
    (r.word("a") || r.word("an")).then_some(())?;
    let filter = phrase(r, Form::Subject)?;
    filter.0.iter().all(describes_a_kind).then_some(())?;
    r.words("of their choice");
    Some(Effect::Sacrifice {
        who,
        filter,
        count: Amount::one(),
    })
}

/// Whether a predicate says what kind of permanent it is, and nothing of who has it or where.
pub(super) fn describes_a_kind(pred: &Pred) -> bool {
    match pred {
        Pred::Type(_)
        | Pred::Supertype(_)
        | Pred::Subtype(_)
        | Pred::Color(_)
        | Pred::Colorless
        | Pred::Token
        | Pred::Tapped
        | Pred::Untapped
        | Pred::Attacking
        | Pred::Blocking
        | Pred::Unblocked
        | Pred::HasKeyword(_)
        | Pred::Compare { .. } => true,
        Pred::Not(inner) => describes_a_kind(inner),
        Pred::AnyOf(preds) => preds.iter().all(describes_a_kind),
        _ => false,
    }
}

/// "Return target card from your graveyard to your hand.", "Return target creature to its owner's
/// hand.": a move to the owner's hand.
fn return_card(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("return")?;
    // The permanent itself once the ability has spoken of the card: "return it to its owner's hand" (a card that
    // returns from the graveyard says its name, and is the table's).
    if r.it == Some(Ref::This) && r.word("it") {
        r.need("to its owner's hand")?;
        return Some(vec![Effect::Move {
            what: if r.died { dead_card() } else { Ref::This },
            to: Destination::Hand,
        }]);
    }
    // The card by its name: "return ~ to its owner's hand".
    if r.ability && r.source() {
        r.need("to its owner's hand")?;
        return Some(vec![Effect::Move {
            what: if r.died { dead_card() } else { Ref::This },
            to: Destination::Hand,
        }]);
    }
    if let Some(what) = several_permanents(r) {
        r.need("to")?;
        super::mass_moves::their_owners(r)?;
        r.need("hands")?;
        return Some(vec![Effect::Move {
            what,
            to: Destination::Hand,
        }]);
    }
    let what = target_object(r).or_else(|| up_to_one_object(r))?;
    let Some(Targetable::Object(selector)) = r.targets.first().map(|spec| &spec.what) else {
        return None;
    };
    // "your hand" is the owner's hand only for a card in your own graveyard.
    let yours = selector.0.contains(&Pred::In(ZoneKind::Graveyard))
        && selector.0.contains(&Pred::OwnedBy(Ref::You));
    // A permanent goes to "its owner's hand"; a spell or a card in a zone would say more.
    let permanent = !selector
        .0
        .iter()
        .any(|pred| matches!(pred, Pred::In(_) | Pred::Spell));
    if yours {
        // A card of your graveyard reaches the battlefield under your control, which is its owner's.
        if r.words("to the battlefield") {
            return Some(vec![Effect::Move {
                what,
                to: Destination::Battlefield {
                    tapped: false,
                    under: None,
                    transformed: false,
                    counters: Vec::new(),
                },
            }]);
        }
        r.need("to your hand")?;
    } else if permanent {
        r.need("to its owner's hand")?;
        r.it = Some(what.clone());
    } else {
        return None;
    }
    Some(vec![Effect::Move {
        what,
        to: Destination::Hand,
    }])
}

/// "six target nonland permanents", "up to two target creatures": several target permanents.
fn several_permanents(r: &mut Reader<'_>) -> Option<Ref> {
    r.attempt(|r| {
        let what = target(r)?;
        let spec = r.targets.first()?;
        let Targetable::Object(selector) = &spec.what else {
            return None;
        };
        let several = spec.bounds.max != Some(Amount::Fixed(1));
        let permanents = !selector
            .0
            .iter()
            .any(|pred| matches!(pred, Pred::In(_) | Pred::Spell));
        (several && permanents).then_some(what)
    })
}

/// "Return a land you control to its owner's hand.": the player chooses one permanent they
/// control, bound as `returned`, and it goes to its owner's hand.
fn return_chosen(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("return")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let from = phrase(r, Form::Subject)?;
    let yours = from.0.contains(&Pred::ControlledBy(Ref::You));
    let on_the_battlefield = !from
        .0
        .iter()
        .any(|pred| matches!(pred, Pred::In(_) | Pred::Spell));
    (yours && on_the_battlefield).then_some(())?;
    r.need("to its owner's hand")?;
    Some(vec![
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(from),
            count: Bounds::default(),
            bind: RETURNED.to_owned(),
            rest_as: None,
        },
        Effect::Move {
            what: Ref::Bound(RETURNED.to_owned()),
            to: Destination::Hand,
        },
    ])
}

/// "Then you may return any number of creatures that saddled it this turn to their owner's hand.": the player
/// chooses none or more of the permanents, bound as `returned`, and they go to their owners' hands.
fn return_any_number(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.word("then");
    r.need("you may return any number of")?;
    let from = phrase(r, Form::All)?;
    let theirs = from
        .0
        .iter()
        .any(|pred| matches!(pred, Pred::Crewed(_) | Pred::Saddled(_)));
    let on_the_battlefield = !from
        .0
        .iter()
        .any(|pred| matches!(pred, Pred::In(_) | Pred::Spell));
    (theirs && on_the_battlefield).then_some(())?;
    r.need("to their owner's hand")?;
    Some(vec![
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(from),
            count: Bounds::up_to(None),
            bind: RETURNED.to_owned(),
            rest_as: None,
        },
        Effect::Move {
            what: Ref::Bound(RETURNED.to_owned()),
            to: Destination::Hand,
        },
    ])
}

/// "Return this card from your graveyard to the battlefield [tapped].", "... to your hand.": the card returns itself
/// from the graveyard, where only an ability that works from there (`Reader::from_graveyard`) finds it.
fn return_itself(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.from_graveyard.then_some(())?;
    r.need("return")?;
    r.source().then_some(())?;
    r.need("from your graveyard")?;
    if r.words("to your hand") {
        return Some(vec![Effect::Move {
            what: Ref::This,
            to: Destination::Hand,
        }]);
    }
    r.need("to the battlefield")?;
    let tapped = r.word("tapped");
    Some(vec![Effect::Move {
        what: Ref::This,
        to: Destination::Battlefield {
            tapped,
            under: None,
            transformed: false,
            counters: Vec::new(),
        },
    }])
}

/// "Shuffle it into its owner's library.": the card the trigger is about (`r.it` is `This`) goes into its owner's
/// library, which is then shuffled.
fn shuffle_itself(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    (r.it == Some(Ref::This)).then_some(())?;
    r.need("shuffle it into its owner's library")?;
    Some(vec![Effect::Move {
        what: if r.died { dead_card() } else { Ref::This },
        to: Destination::Library {
            place: Place::Shuffled,
            order: None,
        },
    }])
}

/// The card that went to a graveyard, which is "it" in what its own trigger does about that: the card of the ability in
/// the graveyard (it is a new object there, so `This` alone names the one that left).
pub fn dead_card() -> Ref {
    Ref::Each(Selector(vec![Pred::In(ZoneKind::Graveyard), Pred::This]))
}

/// Whether "return ~ from your graveyard" comes next: the card returns itself, so the ability works from there.
pub fn returns_from_graveyard(r: &mut Reader<'_>) -> bool {
    r.lookahead(|r| {
        r.need("return")?;
        r.source().then_some(())?;
        r.need("from your graveyard")
    })
    .is_some()
}

/// Whether the effects put the card itself onto the battlefield, so that its ability works from the graveyard.
pub fn returns_itself(effects: &[Effect]) -> bool {
    match effects {
        [
            Effect::May {
                who: Ref::You,
                effects,
            },
        ] => returns_itself(effects),
        [
            Effect::Move {
                what: Ref::This,
                to:
                    Destination::Battlefield {
                        under: None,
                        transformed: false,
                        ..
                    },
            },
        ] => true,
        _ => false,
    }
}

/// The name the permanent returned to its owner's hand is bound to.
pub const RETURNED: &str = "returned";

/// A target object chosen one at a time.
pub(super) fn target_object(r: &mut Reader<'_>) -> Option<Ref> {
    r.attempt(|r| {
        let what = target(r)?;
        let spec = r.targets.first()?;
        (matches!(spec.what, Targetable::Object(_)) && spec.bounds.is_one()).then_some(what)
    })
}

/// "enchanted creature" in the ability of an Aura: the permanent it is attached to.
pub(super) fn attached(r: &mut Reader<'_>) -> Option<Ref> {
    r.ability.then_some(())?;
    r.words("enchanted creature").then_some(Ref::Attached)
}

/// "up to one target creature": a target object that may also be left unchosen.
pub(super) fn up_to_one_object(r: &mut Reader<'_>) -> Option<Ref> {
    r.attempt(|r| {
        let what = target(r)?;
        let spec = r.targets.first()?;
        let bounds = Bounds::up_to(Some(Amount::Fixed(1)));
        (matches!(spec.what, Targetable::Object(_)) && spec.bounds == bounds).then_some(what)
    })
}

/// What a verb acts on: target objects, or "all creatures".
pub(super) fn object(r: &mut Reader<'_>) -> Option<Ref> {
    if let Some(objects) = all(r) {
        return Some(objects);
    }
    if let Some(host) = attached(r) {
        return Some(host);
    }
    r.attempt(|r| {
        let what = target(r)?;
        matches!(r.targets.first()?.what, Targetable::Object(_)).then_some(what)
    })
}

/// "Destroy target creature.", "Exile all creatures.", "Counter target spell.", "Tap target creature.", "Untap
/// target land.", "Regenerate ~."
fn removal(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let verb = r.take_word()?;
    if !matches!(
        verb,
        "destroy" | "exile" | "counter" | "tap" | "untap" | "regenerate"
    ) {
        return None;
    }
    let turns = matches!(verb, "tap" | "untap" | "regenerate");
    // "Exile ~" is the card itself leaving, "Untap ~" the card turning over; nothing else of the card is destroyed or
    // countered this way.
    let what = if matches!(verb, "exile" | "tap" | "untap" | "regenerate") && r.source() {
        Ref::This
    } else {
        object(r)?
    };
    let spell = r.targets.first().is_some_and(|spec| {
        matches!(&spec.what, Targetable::Object(selector) if selector.0.first() == Some(&Pred::Spell))
    });
    if verb == "counter" && !spell {
        return None;
    }
    // Only permanents are tapped: not a spell on the stack and not a card in a graveyard, a hand or a library.
    if turns && (spell || !is_on_the_battlefield(r, &what)) {
        return None;
    }
    if matches!(what, Ref::Target(_)) && r.targets.first()?.bounds.is_one() && !spell {
        r.it = Some(what.clone());
    }
    // "Untap ~. It gains protection from the color of your choice until end of turn."
    if what == Ref::This && turns {
        r.it = Some(Ref::This);
    }
    // "Exile target creature an opponent controls until this creature leaves the battlefield."
    let duration = if verb == "exile" {
        r.attempt(until_exile_ends)
    } else {
        None
    };
    Some(vec![match verb {
        "destroy" => Effect::Destroy {
            what,
            regenerate: true,
        },
        "exile" => Effect::Exile {
            what,
            bind: None,
            duration,
        },
        "tap" => Effect::Tap { what },
        "untap" => Effect::Untap { what },
        "regenerate" => Effect::Regenerate { what },
        _ => Effect::Counter { what },
    }])
}

/// "until ~ leaves the battlefield": the exile lasts as long as the source stays (CR 610.3); "until an opponent becomes
/// the monarch": until the designation passes to an opponent of the controller.
fn until_exile_ends(r: &mut Reader<'_>) -> Option<Duration> {
    r.need("until")?;
    if r.words("an opponent becomes the monarch") {
        return Some(Duration::UntilOpponentBecomesMonarch);
    }
    r.source().then_some(())?;
    r.need("leaves the battlefield")?;
    Some(Duration::WhileSourceOnBattlefield)
}

/// Whether what a verb acts on is named by a selector that looks only at the battlefield: no zone is named.
pub(super) fn is_on_the_battlefield(r: &Reader<'_>, what: &Ref) -> bool {
    let looks_elsewhere =
        |selector: &Selector| selector.0.iter().any(|pred| matches!(pred, Pred::In(_)));
    match what {
        Ref::Target(index) => r.targets.get(usize::from(*index)).is_some_and(
            |spec| matches!(&spec.what, Targetable::Object(selector) if !looks_elsewhere(selector)),
        ),
        Ref::Each(selector) => !looks_elsewhere(selector),
        _ => true,
    }
}

/// The object a pump changes and whether it is several: "target creature", "~", "creatures you control".
pub(super) fn subject(r: &mut Reader<'_>) -> Option<(Ref, bool)> {
    if r.source() {
        return Some((Ref::This, false));
    }
    // "That token gains haste until end of turn.", "Those tokens gain haste until end of turn."
    if let Some(tokens @ Ref::Bound(_)) = r.it.clone() {
        if r.words("that token") {
            return Some((tokens, false));
        }
        if r.words("those tokens") {
            return Some((tokens, true));
        }
    }
    // "Whenever ~ attacks, it gets +1/+1 until end of turn."
    if let Some(it) = r.it.clone()
        && (r.word("it") || (it == Ref::Triggering && r.words("that creature")))
    {
        return Some((it, false));
    }
    if let Some(what) = target_object(r) {
        return Some((what, false));
    }
    if let Some(objects) = all(r) {
        return Some((objects, true));
    }
    // "Each other creature you control gets +1/+1 until end of turn."
    if let Some(objects) = each(r) {
        return Some((objects, false));
    }
    phrase(r, Form::All).map(|selector| (Ref::Each(selector), true))
}

/// "protection from the color of your choice": the color is chosen as the effect begins, so only an effect that lasts
/// a while gives it.
fn chosen_protection(r: &mut Reader<'_>) -> Option<Modification> {
    r.words("protection from the color of your choice")
        .then_some(Modification::AddProtectionFromChosenColor)
}

/// What a pump gives after "gains": keywords, or protection from a color chosen as it resolves.
pub(super) fn pump_grants(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    match chosen_protection(r) {
        Some(protection) => Some(vec![protection]),
        None => granted(r),
    }
}

/// "+2/+2", "-3/-3", "+X/+X", "-X/-X".
fn signed(r: &mut Reader<'_>) -> Option<Amount> {
    r.attempt(|r| {
        if r.punct('+') {
            if r.x() {
                return Some(Amount::X);
            }
            return Some(Amount::Fixed(i32::try_from(r.number()?).ok()?));
        }
        r.punct('-').then_some(())?;
        if r.x() {
            return Some(Amount::Times {
                by: -1,
                of: Box::new(Amount::X),
            });
        }
        Some(Amount::Fixed(-i32::try_from(r.number()?).ok()?))
    })
}

pub(super) fn power_toughness(r: &mut Reader<'_>) -> Option<Modification> {
    let power = signed(r)?;
    r.punct('/').then_some(())?;
    let toughness = signed(r)?;
    Some(Modification::ModifyPT { power, toughness })
}

/// "flying", "flying and haste", "flying, vigilance, and haste", "protection from white and from black": keywords given.
pub(super) fn granted(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    let mut mods: Vec<Modification> = granted_keywords(r)?
        .into_iter()
        .map(Modification::AddKeyword)
        .collect();
    while let Some(next) = r.attempt(|r| {
        if r.punct(',') {
            r.word("and");
        } else {
            r.need("and")?;
        }
        granted_keywords(r)
    }) {
        mods.extend(next.into_iter().map(Modification::AddKeyword));
    }
    Some(mods)
}

/// "Target creature can't be blocked this turn.", "Creatures you control can't be blocked this turn."
fn evasion(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let (affected, _) = subject(r)?;
    r.need("can't be blocked this turn")?;
    Some(vec![Effect::Continuous {
        affected,
        mods: vec![Modification::CantBeBlocked],
        duration: Duration::UntilEndOfTurn,
    }])
}

/// The mana of a symbol, when it is one a land or creature can produce.
pub(super) fn mana_type(symbol: &str) -> Option<ManaType> {
    ManaType::ALL
        .into_iter()
        .find(|mana| symbol.len() == 1 && symbol.starts_with(mana.symbol()))
}

/// "{W} or {U}", "{B}, {R}, or {G}": one mana of a type chosen when it is added.
fn choice(r: &mut Reader<'_>) -> Option<Vec<ManaType>> {
    let mut list = vec![mana_type(r.symbol()?)?];
    while let Some(next) = r.attempt(|r| {
        let listed = r.punct(',');
        let joined = r.word("or");
        (listed || joined).then_some(())?;
        mana_type(r.symbol()?)
    }) {
        list.push(next);
    }
    (list.len() > 1).then_some(list)
}

/// "{W}{W}, {W}{U}, or {U}{U}": the lists of mana one of which is added, at least two of them and one of them with
/// more than one mana (a list of single mana is the choice "{W} or {U}").
fn choice_of_lists(r: &mut Reader<'_>) -> Option<Vec<Vec<ManaType>>> {
    let run = |r: &mut Reader<'_>| {
        let mut list = vec![mana_type(r.symbol()?)?];
        while let Some(next) = r.attempt(|r| mana_type(r.symbol()?)) {
            list.push(next);
        }
        Some(list)
    };
    let mut lists = vec![run(r)?];
    while let Some(next) = r.attempt(|r| {
        let listed = r.punct(',');
        let joined = r.word("or");
        (listed || joined).then_some(())?;
        run(r)
    }) {
        lists.push(next);
    }
    (lists.len() > 1 && lists.iter().any(|list| list.len() > 1)).then_some(lists)
}

/// "two mana of any one color", "X mana of any one color": several mana of the one color picked.
fn several_of_one_color(r: &mut Reader<'_>) -> Option<Amount> {
    let count = if r.x() {
        Amount::X
    } else {
        let number = number_word(r.take_word()?).filter(|count| *count >= 2)?;
        Amount::Fixed(i32::try_from(number).ok()?)
    };
    r.need("mana of any one color")?;
    Some(count)
}

/// "an amount of {G} equal to the greatest power among creatures you control": one type of mana, as many as the amount.
fn an_amount_of(r: &mut Reader<'_>) -> Option<(ManaType, Amount)> {
    r.need("an amount of")?;
    let mana = mana_type(r.symbol()?)?;
    Some((mana, where_x::equal_to(r)?))
}

/// The types before the last of a list that is joined by commas and a final "and/or".
fn comma_types(r: &mut Reader<'_>) -> Option<Vec<ManaType>> {
    let mut list = vec![mana_type(r.symbol()?)?];
    while let Some(next) = r.attempt(|r| {
        r.punct(',').then_some(())?;
        mana_type(r.symbol()?)
    }) {
        list.push(next);
    }
    Some(list)
}

/// "two mana in any combination of colors", "three mana in any combination of {R} and/or {G}": several mana, each of a
/// type picked when it is added.
pub(super) fn combination(r: &mut Reader<'_>) -> Option<Produce> {
    let count = if r.x() {
        Amount::X
    } else {
        let number = number_word(r.take_word()?).filter(|count| *count >= 2)?;
        Amount::Fixed(i32::try_from(number).ok()?)
    };
    r.need("mana in any combination of")?;
    if r.word("colors") {
        return Some(Produce::AnyCombination {
            count,
            among: Vec::new(),
        });
    }
    let mut among = comma_types(r)?;
    r.attempt(|r| {
        r.punct(',');
        r.word("and").then_some(())?;
        r.punct('/').then_some(())?;
        r.word("or").then_some(())?;
        among.push(mana_type(r.symbol()?)?);
        Some(())
    })?;
    Some(Produce::AnyCombination { count, among })
}

/// The lands a mana ability looks at, as "a land an opponent controls" and "a land you control" say.
fn lands_of(who: Ref) -> Selector {
    Selector(vec![
        Pred::Type(mtg_core::card::CardType::Land),
        Pred::ControlledBy(who),
    ])
}

/// "one mana of any color that a land an opponent controls could produce", "one mana of any type that a land you
/// control could produce" (colorless counts as a type).
fn could_produce(r: &mut Reader<'_>) -> Option<Produce> {
    r.need("one mana of any")?;
    let colorless = if r.word("type") {
        true
    } else {
        r.word("color").then_some(false)?
    };
    r.need("that a land")?;
    let among = if r.words("an opponent controls") {
        lands_of(Ref::Opponents)
    } else {
        r.need("you control")?;
        lands_of(Ref::You)
    };
    r.need("could produce")?;
    Some(Produce::ProducedBy {
        among,
        count: 1,
        colorless,
    })
}

/// "Add {G}{G}.", "Add {W} or {U}.", "Add one mana of any color.", "Add one mana of the chosen color.", "Add one
/// mana of any color in your commander's color identity." or "Add one mana of any color that a land an opponent
/// controls could produce."
fn add_mana(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("add")?;
    let mut counted = None;
    let mut mana = produced(r, &mut counted)?;
    // "Add {G}. If you control four or more creatures, add {G}{G} instead."
    if counted.is_none()
        && let Some((when, instead)) = r.attempt(instead)
    {
        mana = Produce::Instead {
            base: Box::new(mana),
            when,
            instead: Box::new(instead),
        };
    }
    // "Add {R} for each Goblin you control." adds the mana once for each; a number of times has no wording.
    let times = match counted.or_else(|| r.attempt(for_each)) {
        Some(each) => each,
        None => Amount::one(),
    };
    let only = spend::only(r).unwrap_or_default();
    Some(vec![Effect::AddMana {
        who: Ref::You,
        mana,
        times,
        only,
    }])
}

/// ". If you control a creature with power 4 or greater, add {G}{G} instead": the condition and the mana in place of
/// the first.
fn instead(r: &mut Reader<'_>) -> Option<(Condition, Produce)> {
    r.punct('.').then_some(())?;
    r.need("if")?;
    let when = as_long_as::body(r, None)?;
    r.punct(',').then_some(())?;
    r.need("add")?;
    let mana = produced(r, &mut None)?;
    r.need("instead")?;
    Some((when, mana))
}

/// The mana after "add": a fixed list, a choice, any color, a combination, or an amount of one type, which sets `counted`.
fn produced(r: &mut Reader<'_>, counted: &mut Option<Amount>) -> Option<Produce> {
    Some(
        if r.words("one mana of any color in your commander's color identity") {
            Produce::CommanderIdentity(1)
        } else if let Some(produced) = r.attempt(could_produce) {
            produced
        } else if r.words("one mana of any color") {
            Produce::AnyColor(Amount::one())
        } else if let Some(combination) = r.attempt(combination) {
            combination
        } else if let Some(count) = r.attempt(several_of_one_color) {
            Produce::AnyColor(count)
        } else if r.words("one mana of the chosen color") {
            Produce::ChosenColor(1)
        } else if let Some(mana) = r.attempt(|r| {
            let mana = mana_type(r.symbol()?)?;
            r.need("or one mana of the chosen color")?;
            Some(mana)
        }) {
            Produce::OrChosenColor(mana)
        } else if let Some((mana, amount)) = r.attempt(an_amount_of) {
            *counted = Some(amount);
            Produce::Fixed(vec![mana])
        } else if let Some(lists) = r.attempt(choice_of_lists) {
            Produce::OneOfLists(lists)
        } else if let Some(list) = r.attempt(choice) {
            Produce::OneOf(list)
        } else {
            let mut list = Vec::new();
            while let Some(mana) = r.attempt(|r| mana_type(r.symbol()?)) {
                list.push(mana);
            }
            if list.is_empty() {
                return None;
            }
            Produce::Fixed(list)
        },
    )
}
