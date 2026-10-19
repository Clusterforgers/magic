import type { PlayState } from '@/play/store'

// The socket keeps retrying past this (a wrong token looks like a lost connection), so a later Welcome still moves on.
/** Failed attempts in a row after which the page says it cannot reach the game instead of "reconnecting". */
export const UNREACHABLE_AFTER = 5

/** The connection as a screen needs it. */
export type Connection =
  | { kind: 'connecting' }
  | { kind: 'open' }
  | { kind: 'reconnecting'; attempt: number }
  | { kind: 'unreachable'; attempt: number }
  | { kind: 'replaced'; reason: string | null }
  | { kind: 'stopped'; reason: string | null }
  | { kind: 'closed' }

/** Reads the connection out of the store: the socket's status and failed attempts, and why it stopped, if it did. */
export function connectionOf(state: Pick<PlayState, 'status' | 'attempt' | 'notice'>): Connection {
  switch (state.status) {
    case 'idle':
    case 'connecting':
      return { kind: 'connecting' }
    case 'open':
      return { kind: 'open' }
    case 'reconnecting':
      return state.attempt >= UNREACHABLE_AFTER
        ? { kind: 'unreachable', attempt: state.attempt }
        : { kind: 'reconnecting', attempt: state.attempt }
    case 'replaced':
      return { kind: 'replaced', reason: state.notice }
    case 'stopped':
      return { kind: 'stopped', reason: state.notice }
    case 'closed':
      return { kind: 'closed' }
  }
}
