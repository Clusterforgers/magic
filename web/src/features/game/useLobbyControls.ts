import { useNavigate } from '@tanstack/react-router'
import { useCallback, useMemo, useRef, useState } from 'react'
import { showFlash } from '@/lib/flash'
import type { Api, SeatCredential } from '@/net/api'
import { describeLobbyError, LobbyError } from '@/net/lobbyErrors'
import { CLOSED_NOTICE, LEFT_NOTICE, removedNotice, STARTED_NOTICE } from './lobbyQuestions'
import type { Question } from './lobbyQuestions'

/** The REST calls the lobby's controls make; `listDecks` and `chooseDeck` are the deck chooser's and `listLobbies` finds the game's house rules. */
export type LobbyApi = Pick<
  Api,
  'closeGame' | 'leaveGame' | 'releaseSeat' | 'setVisibility' | 'listDecks' | 'chooseDeck' | 'listLobbies'
>

/** Where focus goes when a question has closed: back to its button, to the seat list, or to the words that the game began. */
export type FocusAfter = 'opener' | 'seats' | 'note'

/** The question on screen. It is kept while the dialog closes, so its words do not vanish mid-fade. */
export interface Asking {
  question: Question
  open: boolean
  /** The button that asked. */
  opener: HTMLElement | null
  /** Why the last yes failed; the question stays up with it. */
  problem: string | null
  after: FocusAfter
}

/** What a request that runs alone came to. `busy` means another request was out and this one was not sent. */
export type Outcome<T> = { kind: 'done'; value: T } | { kind: 'failed'; error: unknown } | { kind: 'busy' }

/** The lobby's controls: one request at a time, the question before Close, Leave and Remove, and the end of the lobby. */
export interface LobbyControls {
  /** A request is out; every control waits. */
  busy: boolean
  /** Who the requests speak for: the seat's token, or the signed-in session when there is none. */
  credential: SeatCredential
  /** Runs `call` unless another request is out. */
  run<T>(call: () => Promise<T>): Promise<Outcome<T>>
  /** The words that say the lobby can no longer be changed because the game started, or null while it can. */
  stopped: string | null
  /** A request found that out, so focus should go to the words. */
  stoppedByRequest: boolean
  /** Records that the game started, with the words to show. */
  stop(words: string): void
  /** What the last removal came to, in words, for the polite live region. */
  removed: string | null
  asking: Asking | null
  ask(question: Question, opener: HTMLElement | null): void
  /** Says yes to the question on screen. */
  confirm(): void
  /** Says no. Ignored while the yes is on its way. */
  cancel(): void
  /** The dialog finished closing. */
  closed(): void
}

interface Options {
  api: LobbyApi
  game: string
  /** The seat's token, or undefined to act through the signed-in session. */
  token: string | undefined
  /** The lookup says the game has started. */
  started: boolean
}

function send(api: LobbyApi, game: string, credential: SeatCredential, question: Question): Promise<void> {
  switch (question.kind) {
    case 'close':
      return api.closeGame(game, credential)
    case 'leave':
      return api.leaveGame(game, credential)
    case 'remove':
      return api.releaseSeat(game, question.seat, credential)
  }
}

/**
 * The state behind the lobby's buttons. Close and Leave end in a notice on Play and a trip there; Remove ends in words
 * on this page. A refused yes keeps the question up with the reason, except "started", which ends the lobby for good.
 */
export function useLobbyControls({ api, game, token, started }: Options): LobbyControls {
  const navigate = useNavigate()
  const [busy, setBusy] = useState(false)
  const flight = useRef(false)
  const [late, setLate] = useState<string | null>(null)
  const [stoppedByRequest, setStoppedByRequest] = useState(false)
  const [removed, setRemoved] = useState<string | null>(null)
  const [asking, setAsking] = useState<Asking | null>(null)
  const credential = useMemo<SeatCredential>(() => (token === undefined ? {} : { token }), [token])

  const run = useCallback(async <T>(call: () => Promise<T>): Promise<Outcome<T>> => {
    if (flight.current) return { kind: 'busy' }
    flight.current = true
    setBusy(true)
    try {
      return { kind: 'done', value: await call() }
    } catch (error) {
      return { kind: 'failed', error }
    } finally {
      flight.current = false
      setBusy(false)
    }
  }, [])

  const stop = useCallback((words: string): void => {
    setLate(words)
    setStoppedByRequest(true)
  }, [])

  const ask = useCallback((question: Question, opener: HTMLElement | null): void => {
    setRemoved(null)
    setAsking({ question, open: true, opener, problem: null, after: 'opener' })
  }, [])

  const confirm = useCallback((): void => {
    if (asking === null || !asking.open) return
    const { question } = asking
    setAsking({ ...asking, problem: null })
    void (async () => {
      const outcome = await run(() => send(api, game, credential, question))
      if (outcome.kind === 'busy') return
      if (outcome.kind === 'failed') {
        const words = describeLobbyError(outcome.error, question.kind)
        if (outcome.error instanceof LobbyError && outcome.error.code === 'started') {
          stop(words)
          setAsking((now) => (now === null ? null : { ...now, open: false, problem: null, after: 'note' }))
        } else {
          setAsking((now) => (now === null ? null : { ...now, problem: words }))
        }
        return
      }
      if (question.kind === 'remove') {
        setRemoved(removedNotice(question.name, question.seat))
        setAsking((now) => (now === null ? null : { ...now, open: false, problem: null, after: 'seats' }))
        return
      }
      // The server has ended this seat's socket or the whole game; the page leaves with it. A page that has already
      // been replaced by the "connection ended" screen still gets here, since nothing below needs the component.
      showFlash(question.kind === 'close' ? CLOSED_NOTICE : LEFT_NOTICE)
      await navigate({ to: '/' })
    })()
  }, [asking, run, api, game, credential, stop, navigate])

  const cancel = useCallback((): void => {
    if (flight.current) return
    setAsking((now) => (now === null ? null : { ...now, open: false }))
  }, [])

  const closed = useCallback((): void => setAsking(null), [])

  return {
    busy,
    credential,
    run,
    stopped: late ?? (started ? STARTED_NOTICE : null),
    stoppedByRequest,
    stop,
    removed,
    asking,
    ask,
    confirm,
    cancel,
    closed,
  }
}
