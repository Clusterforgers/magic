/** Fewest seats in a game (`MIN_SEATS` in `crates/mtg-server/src/api.rs`); the server has no most. */
export const MIN_SEATS = 2
/** Most seats the create form builds a row for; a bigger count typed in would freeze the page. */
export const MAX_FORM_SEATS = 256
/** Longest seat name, in characters (`MAX_NAME_CHARS` in `crates/mtg-server/src/api.rs`). */
export const MAX_NAME_CHARS = 32
/** Seats a new Commander game starts with: a pod of four. */
export const COMMANDER_POD = 4
