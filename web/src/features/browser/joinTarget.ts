import type { LobbyGame } from '@/protocol/generated'

/** What a visitor asked to join: a game of the list, or the game a typed code names (already normalised). */
export type JoinTarget = { kind: 'game'; game: LobbyGame } | { kind: 'code'; code: string }

/** The page's handler for a join; a page without one has every Join disabled. */
export type OnJoin = (target: JoinTarget) => void

/** The id of the note shown while joining is not built; every disabled Join is described by it. */
export const JOIN_NOTE_ID = 'join-note'

/** The words of that note. */
export const JOIN_NOTE = 'Joining is the next piece'
