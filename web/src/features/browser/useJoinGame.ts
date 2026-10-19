import { useNavigate } from '@tanstack/react-router'
import { useRef, useState } from 'react'
import { useAuthStore } from '@/features/auth/context'
import { fragmentOf, seatLinkPath } from '@/lib/seatLink'
import type { Api } from '@/net/api'
import type { Joined, LobbyGame } from '@/protocol/generated'
import { joinFailure } from './joinErrors'
import type { JoinFailure } from './joinErrors'

/** Starts joining `game`; `opener` is the button that asked, which takes focus back if the question is cancelled. */
export type OnJoin = (game: LobbyGame, opener: HTMLElement | null) => void

/** A guest being asked for a name before `game` is joined. */
export interface Asking {
  game: LobbyGame
  opener: HTMLElement | null
  /** False from the moment the question is answered or cancelled until its dialog has finished closing. */
  open: boolean
}

/** A failure the page shows beside the list. */
export interface PageFailure {
  game: LobbyGame
  text: string
}

/** What the page reads and calls to join games from the list. */
export interface JoinGame {
  start: OnJoin
  /** The id of the game whose request is on its way; every Join is disabled while this is not null. */
  joining: string | null
  /** The guest's question, or null. */
  asking: Asking | null
  /** A failure the guest can answer by trying again, shown in their dialog. */
  dialogFailure: string | null
  /** A failure for the page: a one-click join that failed, or a seat that is gone. */
  failure: PageFailure | null
  /** The guest's answer: the name to join under. */
  submit(name: string): void
  /** The guest backs out. */
  cancel(): void
  /** The dialog has finished closing. */
  closed(): void
  /** Hides the page's failure. */
  dismiss(): void
}

/**
 * Joining a game of the list. A signed-in visitor joins in one click and the seat binds to the account; a guest is
 * asked for a name first. Success opens the game page; a seat that is gone (taken, started, closed) is shown on the
 * page and the list is asked for again; any other failure leaves the form as it was so it can be sent again. One
 * request at a time: a press while one is out does nothing.
 */
export function useJoinGame(api: Pick<Api, 'joinGame'>, refreshList: () => void): JoinGame {
  const navigate = useNavigate()
  const auth = useAuthStore()
  const [joining, setJoining] = useState<string | null>(null)
  const [asking, setAsking] = useState<Asking | null>(null)
  const [dialogFailure, setDialogFailure] = useState<string | null>(null)
  const [failure, setFailure] = useState<PageFailure | null>(null)
  // Set before the first await, so two presses in one tick cannot both send.
  const busy = useRef(false)

  const release = (): void => {
    busy.current = false
    setJoining(null)
  }

  // Opens the game page: the seat link in the fragment, or, for an account that already held a seat, the bare page.
  function open(game: string, joined: Joined): void {
    const page = { to: '/game/$gameId', params: { gameId: game } } as const
    if (joined.token === null) void navigate(page)
    else void navigate({ ...page, hash: fragmentOf(seatLinkPath(game, joined.seat, joined.token)) })
  }

  // Sends the join. Null once the game page is opening; otherwise why not, with the buttons free again.
  async function send(game: LobbyGame, name: string | undefined): Promise<JoinFailure | null> {
    let joined: Joined
    try {
      joined = await api.joinGame(game.game, name === undefined ? {} : { name })
    } catch (error) {
      release()
      return joinFailure(error)
    }
    // Left busy on purpose: the page is about to be replaced, and no second seat should be taken meanwhile.
    open(game.game, joined)
    return null
  }

  function showOnPage(game: LobbyGame, problem: JoinFailure): void {
    setFailure({ game, text: problem.text })
    if (problem.final) refreshList()
  }

  async function begin(game: LobbyGame, opener: HTMLElement | null): Promise<void> {
    // The first answer about the session may still be on its way; a signed-in visitor must not be asked for a name.
    if (auth.getState().status === 'loading') await auth.getState().start()
    if (auth.getState().status !== 'signed-in') {
      release()
      setAsking({ game, opener, open: true })
      return
    }
    const problem = await send(game, undefined)
    if (problem !== null) showOnPage(game, problem)
  }

  const start: OnJoin = (game, opener) => {
    if (busy.current) return
    busy.current = true
    setFailure(null)
    setJoining(game.game)
    void begin(game, opener)
  }

  function submit(name: string): void {
    if (busy.current || asking === null) return
    const { game } = asking
    busy.current = true
    setDialogFailure(null)
    setJoining(game.game)
    void send(game, name).then((problem) => {
      if (problem === null) return
      if (!problem.final) {
        setDialogFailure(problem.text)
        return
      }
      setAsking((was) => (was === null ? null : { ...was, open: false }))
      showOnPage(game, problem)
    })
  }

  return {
    start,
    joining,
    asking,
    dialogFailure,
    failure,
    submit,
    cancel: () => {
      // A request in flight may already hold the seat, and its token is only ever shown to this page.
      if (!busy.current) setAsking((was) => (was === null ? null : { ...was, open: false }))
    },
    closed: () => {
      setAsking(null)
      setDialogFailure(null)
    },
    dismiss: () => setFailure(null),
  }
}
