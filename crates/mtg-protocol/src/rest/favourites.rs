//! Bodies of the favourite-precon endpoints of `mtg-server`: `GET /api/favourites/precons`,
//! `PUT /api/favourites/precons/{code}` and `DELETE /api/favourites/precons/{code}`. A favourite is a
//! pointer to a precon, never a copy; the errors use `DeckErrorCode`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The answer of `GET /api/favourites/precons`: the codes of the precons the account starred that the
/// server still has, the newest star first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct FavouritePrecons {
    /// The precon codes, each as in `PreconListItem::code`.
    pub codes: Vec<String>,
}
