//! The tokens a seat's cards make (`CardDefinition::made_tokens`), offered to be made by hand with every ability.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use mtg_cards::card::TokenSpec;
use mtg_cards::definition::CardDefinition;
use mtg_core::ids::PlayerId;

use crate::object::ObjectKind;
use crate::state::GameState;

/// A token one of a seat's cards makes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardToken {
    /// The name of the card that makes it.
    pub card: String,
    /// Its place among the tokens that card makes.
    pub index: u32,
    /// The token.
    pub token: TokenSpec,
}

/// The tokens of each card name, with the definition they were read from; reading walks the whole definition, so a
/// projection of every batch reads each card once per process.
type Cache = Mutex<HashMap<String, (Weak<CardDefinition>, Arc<[TokenSpec]>)>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The tokens `def` makes, read once while that definition lives.
fn made_by(def: &Arc<CardDefinition>) -> Arc<[TokenSpec]> {
    let Ok(mut cache) = cache().lock() else {
        return def.made_tokens().into();
    };
    if let Some((read_from, specs)) = cache.get(&def.name)
        && read_from
            .upgrade()
            .is_some_and(|known| Arc::ptr_eq(&known, def))
    {
        return Arc::clone(specs);
    }
    let specs: Arc<[TokenSpec]> = def.made_tokens().into();
    cache.insert(def.name.clone(), (Arc::downgrade(def), Arc::clone(&specs)));
    specs
}

/// The tokens the cards `player` owns make, in any zone, by card name and then in the order the card writes them.
pub fn card_tokens(state: &GameState, player: PlayerId) -> Vec<CardToken> {
    let mut cards: BTreeMap<&str, &Arc<CardDefinition>> = BTreeMap::new();
    for object in state.objects() {
        if object.owner() == player && object.kind() == ObjectKind::Card {
            cards.entry(object.def.name.as_str()).or_insert(&object.def);
        }
    }
    cards
        .into_iter()
        .flat_map(|(card, def)| {
            made_by(def)
                .iter()
                .zip(0u32..)
                .map(|(token, index)| CardToken {
                    card: card.to_owned(),
                    index,
                    token: token.clone(),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The token `index` of the card named `card` that `player` owns, if that card makes one there.
pub(crate) fn card_token(
    state: &GameState,
    player: PlayerId,
    card: &str,
    index: u32,
) -> Option<TokenSpec> {
    let def = state
        .objects()
        .find(|object| {
            object.owner() == player && object.kind() == ObjectKind::Card && object.def.name == card
        })
        .map(|object| &object.def)?;
    made_by(def).get(usize::try_from(index).ok()?).cloned()
}
