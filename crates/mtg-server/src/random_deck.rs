//! A deck drawn at random for a seat: `random:precon` draws one of the playable precons, `random:mine` one of the
//! caller's stored decks (`random:mine:<group id>`: one of that group's).
//!
//! The draw prefers the decks that suit the table's format (a Commander table: decks with a command zone; a Normal
//! table: the others) and takes from all when none does, since legality only warns. It is uniform over the candidates
//! and seeded ([`seed_for`]), and the deck drawn is an ordinary [`ChosenDeck`], so the recipe keeps it and a replay
//! or resume plays the same cards. At a game the deck is hidden behind its kind until the start
//! ([`ChosenDeck::hidden_as`]); a Delves hero sees it at once, since the run's upgrades follow its commander.

use axum::http::HeaderMap;
use mtg_protocol::rest::{RANDOM_OWN_DECK, RANDOM_PRECON_DECK};
use sha2::{Digest, Sha256};
use sqlx::types::uuid::Uuid;

use crate::card_api::failed;
use crate::deck_choice::{Refused, resolve_as};
use crate::recipe::FormatKind;
use crate::seat_deck::ChosenDeck;
use crate::server::AppState;

/// What the lobby shows for a precon drawn at random until the start.
pub(crate) const RANDOM_PRECON_LABEL: &str = "Random precon";

/// What the lobby shows for one of the player's decks drawn at random until the start.
pub(crate) const RANDOM_OWN_LABEL: &str = "Random deck";

/// Where a random deck is drawn from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pool {
    /// Every playable precon.
    Precons,
    /// The caller's stored decks, or only those of one of their groups.
    Mine(Option<Uuid>),
}

impl Pool {
    /// The pool a deck id names, if it names one; a group that is no id names none.
    pub(crate) fn of(id: &str) -> Option<Pool> {
        let id = id.trim();
        if id == RANDOM_PRECON_DECK {
            return Some(Pool::Precons);
        }
        let rest = id.strip_prefix(RANDOM_OWN_DECK)?;
        if rest.is_empty() {
            return Some(Pool::Mine(None));
        }
        let group = rest.strip_prefix(':')?;
        Uuid::parse_str(group)
            .ok()
            .map(|group| Pool::Mine(Some(group)))
    }

    /// The label the lobby shows until the start.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Pool::Precons => RANDOM_PRECON_LABEL,
            Pool::Mine(_) => RANDOM_OWN_LABEL,
        }
    }
}

/// The seed of the draw for `seat` of `game`: one draw per seat and game, so naming the pool again draws the same deck.
pub(crate) fn seed_for(game: &str, seat: usize) -> u64 {
    let digest = Sha256::new()
        .chain_update(b"random-deck\0")
        .chain_update(game.as_bytes())
        .chain_update(seat.to_le_bytes())
        .finalize();
    let mut bytes = [0; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_le_bytes(bytes)
}

/// The seed of a draw for `seat` of a game not made yet: fresh random bytes, so each new game draws anew.
pub(crate) fn fresh_seed(seat: usize) -> u64 {
    let mut bytes = [0; 16];
    if getrandom::fill(&mut bytes).is_err() {
        tracing::warn!("no random bytes for a random deck; drawing with a fixed seed");
    }
    let text = format!("{bytes:?}");
    seed_for(&text, seat)
}

/// Whether a deck of the format `format` (a deck format key such as `commander` or `modern`) suits a table of `table`.
pub(crate) fn suits(table: FormatKind, format: &str) -> bool {
    (table == FormatKind::Commander) == (format == "commander")
}

/// The candidate `seed` draws: uniformly among those `fits` takes, else among all; none from an empty list.
pub(crate) fn draw<T>(candidates: &[T], fits: impl Fn(&T) -> bool, seed: u64) -> Option<&T> {
    let fitting: Vec<&T> = candidates.iter().filter(|c| fits(c)).collect();
    let pool: Vec<&T> = if fitting.is_empty() {
        candidates.iter().collect()
    } else {
        fitting
    };
    let count = u64::try_from(pool.len()).ok().filter(|count| *count > 0)?;
    let at = usize::try_from(seed % count).ok()?;
    pool.get(at).copied()
}

/// The id of the deck `pool` draws with `seed` for a table of `format`: `precon:<code>` or a stored deck's id. An
/// empty pool, a group of another account or a caller who is not signed in for their own decks is
/// [`Refused::Unknown`].
pub(crate) async fn drawn_id(
    app: &AppState,
    owner: Option<Uuid>,
    pool: Pool,
    format: FormatKind,
    seed: u64,
) -> Result<String, Refused> {
    let Some(source) = app.cards() else {
        return Err(Refused::Unknown);
    };
    match pool {
        Pool::Precons => {
            let index = source
                .precon_index()
                .await
                .map_err(|error| Refused::Failed(crate::api::ApiError::from(error)))?;
            let playable: Vec<_> = index.items().filter(|item| item.playable).collect();
            let item = draw(&playable, |item| suits(format, &item.format), seed)
                .ok_or(Refused::Unknown)?;
            Ok(format!(
                "{}{}",
                mtg_protocol::rest::PRECON_DECK_PREFIX,
                item.code
            ))
        }
        Pool::Mine(group) => {
            let owner = owner.ok_or(Refused::Unknown)?;
            let database = source.database();
            let mut decks = database
                .decks()
                .list(owner)
                .await
                .map_err(|error| Refused::Failed(failed(&error)))?;
            if let Some(group) = group {
                let groups = database
                    .deck_groups()
                    .list(owner)
                    .await
                    .map_err(|error| Refused::Failed(failed(&error)))?;
                let members = groups
                    .into_iter()
                    .find(|stored| stored.id == group)
                    .ok_or(Refused::Unknown)?
                    .decks;
                decks.retain(|deck| members.contains(&deck.id));
            }
            let deck =
                draw(&decks, |deck| suits(format, &deck.format), seed).ok_or(Refused::Unknown)?;
            Ok(deck.id.0.to_string())
        }
    }
}

/// The deck `pool` draws for the caller at a table of `format`, hidden behind the pool's label until the start.
pub(crate) async fn resolve(
    app: &AppState,
    headers: &HeaderMap,
    pool: Pool,
    format: FormatKind,
    seed: u64,
) -> Result<ChosenDeck, Refused> {
    let owner = app
        .auth()
        .session_user(headers)
        .await?
        .map(|user| user.id.uuid());
    let id = drawn_id(app, owner, pool, format, seed).await?;
    let deck = match owner {
        Some(owner) => resolve_as(app, owner, &id).await?,
        None => resolve_as(app, Uuid::nil(), &id).await?,
    };
    Ok(deck.hidden_as(pool.label()))
}

#[cfg(test)]
mod tests {
    //! Goal: a random deck id names its pool (a group only by a real id); the draw is uniform over the decks that suit the table and falls back to all when none does, the same seed draws the same deck, every seat and game seeds its own draw, and a hidden deck shows its label until it is revealed.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::precon("random:precon", Some(Pool::Precons))]
    #[case::mine("random:mine", Some(Pool::Mine(None)))]
    #[case::spaced(" random:mine ", Some(Pool::Mine(None)))]
    #[case::group(
        "random:mine:67e55044-10b1-426f-9247-bb680e5fe0c8",
        Some(Pool::Mine(Some(Uuid::from_u128(0x67e5_5044_10b1_426f_9247_bb68_0e5f_e0c8))))
    )]
    #[case::bad_group("random:mine:nope", None)]
    #[case::no_colon("random:minez", None)]
    #[case::precon_code("precon:c18-adaptive", None)]
    #[case::other("random", None)]
    fn ids_name_their_pool(#[case] id: &str, #[case] pool: Option<Pool>) {
        assert_eq!(Pool::of(id), pool);
    }

    #[rstest]
    #[case::commander_takes_commander(FormatKind::Commander, "commander", true)]
    #[case::commander_skips_modern(FormatKind::Commander, "modern", false)]
    #[case::normal_takes_modern(FormatKind::Normal, "modern", true)]
    #[case::normal_takes_freeform(FormatKind::Normal, "freeform", true)]
    #[case::normal_skips_commander(FormatKind::Normal, "commander", false)]
    fn decks_suit_their_table(#[case] table: FormatKind, #[case] format: &str, #[case] fits: bool) {
        assert_eq!(suits(table, format), fits);
    }

    #[rstest]
    #[case::only_fitting(&[1, 2, 3, 4, 5, 6], &[2, 4, 6])]
    #[case::falls_back_to_all(&[1, 3, 5], &[1, 3, 5])]
    #[case::one(&[4], &[4])]
    fn the_draw_is_uniform_over_the_fitting(#[case] candidates: &[u32], #[case] expected: &[u32]) {
        let mut counts = std::collections::BTreeMap::new();
        for seed in 0..600 {
            let drawn = *draw(candidates, |n| n % 2 == 0, seed).unwrap();
            *counts.entry(drawn).or_insert(0) += 1;
        }
        assert_eq!(counts.keys().copied().collect::<Vec<_>>(), expected);
        let each = 600 / expected.len();
        assert!(counts.values().all(|n| *n == each), "{counts:?}");
    }

    #[test]
    fn an_empty_pool_draws_nothing() {
        assert_eq!(draw::<u32>(&[], |_| true, 7), None);
    }

    #[test]
    fn each_seat_and_game_has_its_own_stable_seed() {
        assert_eq!(seed_for("game", 1), seed_for("game", 1));
        let seeds: std::collections::BTreeSet<u64> = (0..8)
            .flat_map(|seat| ["a", "b"].map(|game| seed_for(game, seat)))
            .collect();
        assert_eq!(seeds.len(), 16);
    }

    #[rstest]
    #[case::precon(Pool::Precons, "Random precon")]
    #[case::mine(Pool::Mine(None), "Random deck")]
    fn a_hidden_deck_shows_its_label_until_revealed(#[case] pool: Pool, #[case] label: &str) {
        let deck = crate::deck_choice::built_in("mono-red").unwrap();
        let name = deck.note().name.clone();
        assert_eq!(deck.revealed(), None);
        let hidden = deck.hidden_as(pool.label());
        assert_eq!(hidden.note().name, label);
        assert_eq!(hidden.revealed(), Some(name.as_str()));
    }
}
