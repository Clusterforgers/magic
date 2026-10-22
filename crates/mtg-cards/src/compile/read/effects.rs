//! Effects: a sentence is read into the effects of one template. A template reads the whole
//! sentence or nothing; to add a wording, write a template and list it in [`TEMPLATES`].

use mtg_core::mana::ManaType;

use super::amounts::{cards, its_stat, number_word, points};
use super::cast;
use super::costs;
use super::counters;
use super::create;
use super::library;
use super::modal;
use super::selectors::{Form, all, each, granted_keywords, phrase, target};
use super::spend;
use super::tokens::Reader;
use crate::cost::Cost;
use crate::effect::{Destination, Effect, Order, Place, Produce};
use crate::mana::ManaCostX;
use crate::modify::{Duration, Modification};
use crate::refs::{Bounds, Pred, Ref, Selector, Targetable, ZoneKind};
use crate::value::{Amount, CounterKind};

/// A template: the effects of the sentence it reads, or `None` when the sentence is not its shape.
type Template = fn(&mut Reader<'_>) -> Option<Vec<Effect>>;

/// The sentence shapes, tried in this order.
const TEMPLATES: [Template; 23] = [
    modal::modal,
    cast::discover_sentence,
    cast::exile_until,
    cast::exile_top,
    may,
    chain,
    put_back,
    player_effect,
    damage,
    removal,
    pump,
    evasion,
    return_card,
    return_chosen,
    create::tokens,
    counters::put,
    counters::remove,
    counters::moving,
    add_mana,
    library::search,
    library::look,
    library::put_on_library,
    return_itself,
];

/// The shapes that begin with a verb ("you" is the subject): "you may" can stand before each.
const IMPERATIVES: [Template; 10] = [
    chain,
    removal,
    return_card,
    return_chosen,
    create::tokens,
    counters::put,
    counters::remove,
    counters::moving,
    library::put_on_library,
    return_itself,
];

/// Reads the next sentence and its period into `effects`; false when it is not a shape the
/// templates read.
pub fn sentence(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> bool {
    if r.attempt(|r| cast::after_exiling(r, effects)).is_some() {
        return true;
    }
    if r.attempt(|r| regeneration(r, effects)).is_some() {
        return true;
    }
    for template in TEMPLATES.into_iter().chain([unless_pays as Template]) {
        let read = r.attempt(|r| {
            let found = template(r)?;
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
    TEMPLATES.into_iter().find_map(|template| {
        r.attempt(|r| {
            let unpaid = template(r)?;
            r.need("unless")?;
            let who = payer(r, &unpaid)?;
            r.need("pays")?;
            let cost = mana_cost(r)?;
            Some(vec![Effect::Pay {
                who,
                cost,
                paid: Vec::new(),
                unpaid,
            }])
        })
    })
}

/// Who may pay: the controller of the spell that is countered, or the player the trigger is about.
fn payer(r: &mut Reader<'_>, unpaid: &[Effect]) -> Option<Ref> {
    if r.words("its controller") {
        let countered = matches!(
            unpaid,
            [Effect::Counter {
                what: Ref::Target(0)
            }]
        );
        return countered.then(|| Ref::ControllerOf(Box::new(Ref::Target(0))));
    }
    (r.that_player && r.words("that player")).then_some(Ref::Triggering)
}

/// The mana of a payment: `{2}`, `{U}{R}`; an `{X}` is not the payer's to choose.
fn mana_cost(r: &mut Reader<'_>) -> Option<Cost> {
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
    r.need(if single {
        "it can't be regenerated"
    } else {
        "they can't be regenerated"
    })?;
    if !r.sentence_end() || !*regenerate {
        return None;
    }
    *regenerate = false;
    Some(())
}

/// "You may draw a card.": one step that the controller may do.
fn may(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("you may")?;
    IMPERATIVES.iter().find_map(|template| {
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

/// Steps of "you" joined by ", then": "Scry 1, then draw a card."
fn chain(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let mut effects = vec![imperative(r)?];
    while let Some(next) = r.attempt(|r| {
        r.punct(',').then_some(())?;
        r.need("then")?;
        match r.attempt(imperative) {
            Some(step) => Some(vec![step]),
            None => put_back(r),
        }
    }) {
        effects.extend(next);
    }
    Some(effects)
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
    let count = Bounds {
        min: number,
        max: Some(Amount::Fixed(i32::try_from(number).ok()?)),
    };
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

fn imperative(r: &mut Reader<'_>) -> Option<Effect> {
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
        "draw" => Effect::Draw {
            who,
            count: cards(r)?,
        },
        "mill" => Effect::Mill {
            who,
            count: cards(r)?,
        },
        "shuffle" => {
            r.need("your library")?;
            Effect::Shuffle { who }
        }
        "discard" => {
            let count = cards(r)?;
            let random = r.words("at random");
            Effect::Discard { who, count, random }
        }
        _ => return None,
    })
}

/// The player an effect is about: "you", "target player", "each opponent", "its controller".
fn player(r: &mut Reader<'_>) -> Option<Ref> {
    if r.word("you") {
        return Some(Ref::You);
    }
    if r.words("each opponent") {
        return Some(Ref::Opponents);
    }
    if r.words("each player") {
        return Some(Ref::EachPlayer);
    }
    if r.words("its controller") {
        return r.it.clone().map(|it| Ref::ControllerOf(Box::new(it)));
    }
    if r.that_player && r.words("that player") {
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
fn life(r: &mut Reader<'_>) -> Option<Amount> {
    if r.word("life") {
        r.need("equal to")?;
        return its_stat(r);
    }
    let amount = points(r)?;
    r.word("life").then_some(amount)
}

/// A player draws, discards, mills, gains, loses or sacrifices; one subject may have several verb
/// phrases, joined by "and" or in a list: "target opponent sacrifices a creature, discards a card,
/// and loses 3 life". The player the event is about may act after "and": "you create a Wolf token and that player mills
/// ten cards". The effects are those of the sentence, in order.
fn player_effect(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let who = player(r)?;
    let mut effects = phrases(r, &who)?;
    // The sentences of two other players stay with the table.
    while let Some(more) = r.attempt(|r| {
        r.need("and")?;
        let next = (r.that_player && r.words("that player")).then_some(Ref::Triggering)?;
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
    Some(effects)
}

/// One verb phrase: what a player does, or "create a token" for you.
fn verb_phrase(r: &mut Reader<'_>, who: &Ref) -> Option<Vec<Effect>> {
    if *who == Ref::You && r.peek_word() == Some("create") {
        return create::tokens(r);
    }
    predicate(r, who).map(|effect| vec![effect])
}

/// What a player does: the verb and its object, without the subject.
fn predicate(r: &mut Reader<'_>, who: &Ref) -> Option<Effect> {
    let who = who.clone();
    Some(match r.take_word()? {
        "draw" | "draws" => Effect::Draw {
            who,
            count: cards(r)?,
        },
        "mill" | "mills" => Effect::Mill {
            who,
            count: cards(r)?,
        },
        "discard" | "discards" => {
            let count = cards(r)?;
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
    let what = target_object(r)?;
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

/// "Return this card from your graveyard to the battlefield.": the card puts itself onto the battlefield from the
/// graveyard, where only an ability that works from there (`Reader::from_graveyard`) finds it.
fn return_itself(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.from_graveyard.then_some(())?;
    r.need("return")?;
    r.source().then_some(())?;
    r.need("from your graveyard to the battlefield")?;
    Some(vec![rising()])
}

/// The card moves itself onto the battlefield.
fn rising() -> Effect {
    Effect::Move {
        what: Ref::This,
        to: Destination::Battlefield {
            tapped: false,
            under: None,
        },
    }
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
        [only] => *only == rising(),
        _ => false,
    }
}

/// The name the permanent returned to its owner's hand is bound to.
pub const RETURNED: &str = "returned";

/// Who or what takes damage: a single target, "each opponent", "each creature".
fn recipient(r: &mut Reader<'_>) -> Option<Ref> {
    if r.words("each opponent") {
        return Some(Ref::Opponents);
    }
    if r.words("each player") {
        return Some(Ref::EachPlayer);
    }
    if r.word("you") {
        return Some(Ref::You);
    }
    if r.that_player && r.words("that player") {
        return Some(Ref::Triggering);
    }
    if let Some(objects) = each(r) {
        return Some(objects);
    }
    r.attempt(|r| {
        let to = target(r)?;
        let spec = r.targets.first()?;
        (spec.bounds.is_one() && spec.divide.is_none()).then_some(to)
    })
}

/// "~ deals 3 damage to any target.", and "It deals 1 damage to any target." as the first thing an ability does, where "it" is
/// the source.
fn damage(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    (r.source() || (r.targets.is_empty() && r.word("it"))).then_some(())?;
    r.need("deals")?;
    let amount = points(r)?;
    r.need("damage to")?;
    let to = recipient(r)?;
    Some(vec![Effect::DealDamage {
        source: None,
        to,
        amount,
    }])
}

/// A target object chosen one at a time.
pub(super) fn target_object(r: &mut Reader<'_>) -> Option<Ref> {
    r.attempt(|r| {
        let what = target(r)?;
        let spec = r.targets.first()?;
        (matches!(spec.what, Targetable::Object(_)) && spec.bounds.is_one()).then_some(what)
    })
}

/// What a verb acts on: target objects, or "all creatures".
fn object(r: &mut Reader<'_>) -> Option<Ref> {
    if let Some(objects) = all(r) {
        return Some(objects);
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
    Some(vec![match verb {
        "destroy" => Effect::Destroy {
            what,
            regenerate: true,
        },
        "exile" => Effect::Exile { what },
        "tap" => Effect::Tap { what },
        "untap" => Effect::Untap { what },
        "regenerate" => Effect::Regenerate { what },
        _ => Effect::Counter { what },
    }])
}

/// Whether what a verb acts on is named by a selector that looks only at the battlefield: no zone is named.
fn is_on_the_battlefield(r: &Reader<'_>, what: &Ref) -> bool {
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
fn subject(r: &mut Reader<'_>) -> Option<(Ref, bool)> {
    if r.source() {
        return Some((Ref::This, false));
    }
    if let Some(what) = target_object(r) {
        return Some((what, false));
    }
    if let Some(objects) = all(r) {
        return Some((objects, true));
    }
    phrase(r, Form::All).map(|selector| (Ref::Each(selector), true))
}

/// "+2/+2", "-3/-3", "+X/+X".
fn signed(r: &mut Reader<'_>) -> Option<Amount> {
    r.attempt(|r| {
        if r.punct('+') {
            if r.x_free && r.word("x") {
                return Some(Amount::X);
            }
            return Some(Amount::Fixed(i32::try_from(r.number()?).ok()?));
        }
        r.punct('-').then_some(())?;
        Some(Amount::Fixed(-i32::try_from(r.number()?).ok()?))
    })
}

pub(super) fn power_toughness(r: &mut Reader<'_>) -> Option<Modification> {
    let power = signed(r)?;
    r.punct('/').then_some(())?;
    let toughness = signed(r)?;
    Some(Modification::ModifyPT { power, toughness })
}

/// "flying", "flying and haste", "protection from white and from black": keywords given.
pub(super) fn granted(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    let mut mods: Vec<Modification> = granted_keywords(r)?
        .into_iter()
        .map(Modification::AddKeyword)
        .collect();
    while let Some(next) = r.attempt(|r| {
        r.need("and")?;
        granted_keywords(r)
    }) {
        mods.extend(next.into_iter().map(Modification::AddKeyword));
    }
    Some(mods)
}

/// "Target creature gets +3/+3 until end of turn.", "... gains flying until end of turn."
fn pump(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let (affected, plural) = subject(r)?;
    let (gets, gains) = if plural {
        ("get", "gain")
    } else {
        ("gets", "gains")
    };
    let mut mods = Vec::new();
    if r.word(gets) {
        mods.push(power_toughness(r)?);
        if r.word("and") {
            r.word(gains).then_some(())?;
            mods.extend(granted(r)?);
        }
    } else {
        r.word(gains).then_some(())?;
        mods = granted(r)?;
    }
    r.need("until end of turn")?;
    Some(vec![Effect::Continuous {
        affected,
        mods,
        duration: Duration::UntilEndOfTurn,
    }])
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
fn mana_type(symbol: &str) -> Option<ManaType> {
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

/// "Add {G}{G}.", "Add {W} or {U}.", "Add one mana of any color." or "Add one mana of the chosen color."
fn add_mana(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("add")?;
    let mana = if r.words("one mana of any color") {
        Produce::AnyColor(1)
    } else if r.words("one mana of the chosen color") {
        Produce::ChosenColor(1)
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
    };
    let only = spend::only(r).unwrap_or_default();
    Some(vec![Effect::AddMana {
        who: Ref::You,
        mana,
        times: Amount::one(),
        only,
    }])
}
