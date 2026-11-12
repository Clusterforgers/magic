import { useCallback, useState } from 'react'
import { teamLabel, teamsProblem, withSeatTeam } from '@/lib/teams'
import type { SeatCredential } from '@/net/api'
import { describeLobbyError, LobbyError } from '@/net/lobbyErrors'
import type { Teams } from '@/protocol/generated'
import type { LobbyApi, Outcome } from './useLobbyControls'

/** The host's control of the teams as the seat rows draw it. */
export interface TeamsControl {
  /** What to draw: the teams being asked for, else what the server last said; null without teams. */
  shown: Teams | null
  /** A request is out. */
  pending: boolean
  /** A sentence about the request now out or the last one that went through, for the polite live region. */
  note: string | null
  /** Why the last change was refused, with the teams that are back in force. */
  problem: string | null
  /** Moves `seat` to `team`. */
  choose(seat: number, team: number, name: string): void
}

interface Options {
  api: LobbyApi
  game: string
  credential: SeatCredential
  /** The teams the roster (or, before it, the lookup) says are in force; null without teams. */
  known: Teams | null
  run<T>(call: () => Promise<T>): Promise<Outcome<T>>
  /** The server says the game has started; the lobby stops offering changes. */
  stop(words: string): void
}

const same = (a: Teams | null, b: Teams | null): boolean => JSON.stringify(a) === JSON.stringify(b)

/**
 * Applies the host's change of a seat's team through `setTeams`. The new teams show at once while the request is out;
 * teams the server would refuse are not sent, and a refusal puts the old ones back with the reason. After a success
 * the answer holds until the roster says the same, as the visibility toggle does.
 */
export function useTeams({ api, game, credential, known, run, stop }: Options): TeamsControl {
  const [pending, setPending] = useState<Teams | null>(null)
  const [confirmed, setConfirmed] = useState<Teams | null | undefined>(undefined)
  const [result, setResult] = useState<string | null>(null)
  const [problem, setProblem] = useState<string | null>(null)

  // The roster has caught up with the answer: let go of it, so a change made elsewhere shows from now on.
  if (confirmed !== undefined && same(confirmed, known)) setConfirmed(undefined)

  const shown = pending ?? (confirmed === undefined ? known : confirmed)

  const choose = useCallback(
    (seat: number, team: number, name: string): void => {
      if (shown === null) return
      const next = withSeatTeam(shown, seat, team)
      if (next === shown) return
      setResult(null)
      const refused = teamsProblem(next)
      if (refused !== null) {
        setProblem(`The teams stay as they are. ${refused}`)
        return
      }
      setProblem(null)
      setPending(next)
      void run(() => api.setTeams(game, next, credential)).then((outcome) => {
        setPending(null)
        if (outcome.kind === 'busy') return
        if (outcome.kind === 'done') {
          setConfirmed(outcome.value)
          setResult(`${name} is now in ${teamLabel(next.seats[seat] ?? team)}. Everyone presses Ready again.`)
          return
        }
        const words = describeLobbyError(outcome.error, 'teams')
        if (outcome.error instanceof LobbyError && outcome.error.code === 'started') stop(words)
        else setProblem(`The teams stay as they were. ${words}`)
      })
    },
    [shown, run, api, game, credential, stop],
  )

  return {
    shown,
    pending: pending !== null,
    note: pending === null ? result : 'Changing the teams…',
    problem,
    choose,
  }
}
