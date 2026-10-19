import { useCallback, useEffect, useState } from 'react'
import type { SeatCredential } from '@/net/api'
import { describeLobbyError, LobbyError } from '@/net/lobbyErrors'
import type { Visibility } from '@/protocol/generated'
import type { LobbyApi, Outcome } from './useLobbyControls'

/** The visibility control as the toggle draws it. */
export interface VisibilityControl {
  /** What to draw: the value being asked for, else the one the server last confirmed, else what the lookup says. */
  shown: Visibility | undefined
  /** The value whose request is out. */
  pending: Visibility | null
  /** A sentence about the request now out or the last one that went through, for the polite live region. */
  note: string | null
  /** Why the last request failed, with the value that is back in force. */
  problem: string | null
  choose(next: Visibility): void
}

interface Options {
  api: LobbyApi
  game: string
  credential: SeatCredential
  /** The visibility from the lookup, or undefined until it has said. */
  known: Visibility | undefined
  run<T>(call: () => Promise<T>): Promise<Outcome<T>>
  /** The server says the game has started; the lobby stops offering changes. */
  stop(words: string): void
}

const PENDING: Record<Visibility, string> = {
  public: 'Listing the game in the game browser…',
  invite: 'Making the game invite only…',
}

const DONE: Record<Visibility, string> = {
  public: 'The game is now listed in the game browser.',
  invite: 'The game is now invite only. Only people with your link or code can join.',
}

const STILL: Record<Visibility, string> = {
  public: 'listed in the game browser',
  invite: 'invite only',
}

/**
 * Applies the host's choice through `setVisibility`. The choice shows at once with the request's pending state; a
 * refusal puts the old value back with the reason. After a success the answer holds until the lookup says the same,
 * so a look that was already on its way cannot flip the toggle back for a moment.
 */
export function useVisibility({ api, game, credential, known, run, stop }: Options): VisibilityControl {
  const [pending, setPending] = useState<Visibility | null>(null)
  const [confirmed, setConfirmed] = useState<Visibility | null>(null)
  const [result, setResult] = useState<string | null>(null)
  const [problem, setProblem] = useState<string | null>(null)

  useEffect(() => {
    if (confirmed !== null && confirmed === known) setConfirmed(null)
  }, [confirmed, known])

  const shown = pending ?? confirmed ?? known

  const choose = useCallback(
    (next: Visibility): void => {
      if (shown === undefined || next === shown) return
      const before = shown
      setProblem(null)
      setResult(null)
      setPending(next)
      void run(() => api.setVisibility(game, next, credential)).then((outcome) => {
        setPending(null)
        if (outcome.kind === 'busy') return
        if (outcome.kind === 'done') {
          setConfirmed(outcome.value)
          setResult(DONE[outcome.value])
          return
        }
        const words = describeLobbyError(outcome.error, 'visibility')
        if (outcome.error instanceof LobbyError && outcome.error.code === 'started') stop(words)
        else setProblem(`The game is still ${STILL[before]}. ${words}`)
      })
    },
    [shown, run, api, game, credential, stop],
  )

  return { shown, pending, note: pending === null ? result : PENDING[pending], problem, choose }
}
