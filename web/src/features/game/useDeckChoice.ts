import { useCallback, useState } from 'react'
import type { SeatCredential } from '@/net/api'
import { describeLobbyError, LobbyError } from '@/net/lobbyErrors'
import type { Issue, SeatDeck } from '@/protocol/generated'
import { deckWords, sameDeck, stillWords } from './deckWords'
import type { LobbyApi, Outcome } from './useLobbyControls'

/** A deck the player pressed: the id the server takes and the name to say while it is asked about. */
export interface DeckPick {
  id: string
  name: string
}

/** Why a choice did not go through: the sentence, and for a refused deck what breaks the rules. */
export interface DeckProblem {
  words: string
  issues: readonly Issue[]
  /** The server refused the deck for what it holds (422), not because the request failed. */
  blocked: boolean
}

/** The deck choice as the chooser draws it. */
export interface DeckChoice {
  /** The deck whose request is out. */
  pending: DeckPick | null
  /** The id of the deck the last answer confirmed, while the roster still says the same deck. */
  chosenId: string | null
  /** A sentence about the request now out or the last one that went through, for the polite live region. */
  note: string | null
  /** Why the last request failed, with the deck that is still in force. */
  problem: DeckProblem | null
  choose(pick: DeckPick): void
}

interface Options {
  api: Pick<LobbyApi, 'chooseDeck'>
  game: string
  /** This page's seat. */
  seat: number
  credential: SeatCredential
  /** The deck the roster shows for this seat: null for none, undefined while no roster has said. */
  current: SeatDeck | null | undefined
  run<T>(call: () => Promise<T>): Promise<Outcome<T>>
  /** The server says the game has started; the lobby stops offering changes. */
  stop(words: string): void
}

interface Chosen {
  id: string
  deck: SeatDeck
}

/**
 * Sends the player's choice through `chooseDeck`. The choice shows at once with the request's pending state; a refusal
 * puts the old choice back and says why (a refused deck also lists its issues). An answer names the deck by the three
 * facts the table is told, and its id marks the pressed option for as long as the roster shows that very deck.
 */
export function useDeckChoice({ api, game, seat, credential, current, run, stop }: Options): DeckChoice {
  const [pending, setPending] = useState<DeckPick | null>(null)
  const [chosen, setChosen] = useState<Chosen | null>(null)
  const [result, setResult] = useState<string | null>(null)
  const [problem, setProblem] = useState<DeckProblem | null>(null)

  // The id marks an option only while it still describes the seat's deck: a roster that shows another one has moved on.
  const chosenId =
    chosen !== null && (current === undefined || (current !== null && sameDeck(current, chosen.deck)))
      ? chosen.id
      : null

  const choose = useCallback(
    (pick: DeckPick): void => {
      setProblem(null)
      setResult(null)
      setPending(pick)
      void run(() => api.chooseDeck(game, seat, pick.id, credential)).then((outcome) => {
        setPending(null)
        if (outcome.kind === 'busy') return
        if (outcome.kind === 'done') {
          setChosen({ id: pick.id, deck: outcome.value })
          setResult(`Your deck is now ${deckWords(outcome.value)}.`)
          return
        }
        const { error } = outcome
        const words = describeLobbyError(error, 'deck')
        if (error instanceof LobbyError && error.code === 'started') {
          stop(words)
          return
        }
        setProblem({
          words: `${stillWords(current)} ${words}`,
          issues: error instanceof LobbyError ? error.issues : [],
          blocked: error instanceof LobbyError && error.code === 'deck_blocked',
        })
      })
    },
    [run, api, game, seat, credential, current, stop],
  )

  return {
    pending,
    chosenId,
    note: pending === null ? result : `Choosing ${pending.name}…`,
    problem,
    choose,
  }
}
