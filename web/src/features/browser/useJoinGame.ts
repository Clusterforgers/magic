import { useNavigate } from '@tanstack/react-router'
import { useRef, useState } from 'react'
import { useAuthStore } from '@/features/auth/context'
import { fragmentOf, seatLinkPath } from '@/lib/seatLink'
import type { Api } from '@/net/api'
import type { Joined, LobbyGame } from '@/protocol/generated'
import { codeFailure, joinFailure } from './joinErrors'
import type { JoinFailure } from './joinErrors'

/** Starts joining `game`; `opener` is the button that asked, which takes focus back if the question is cancelled. */
export type OnJoin = (game: LobbyGame, opener: HTMLElement | null) => void

/** Starts joining the game `code` names (as `normalizeCode` leaves it); `opener` is as for `OnJoin`. */
export type OnJoinCode = (code: string, opener: HTMLElement | null) => void

/** What a join is for: a row of the list, or the game a typed code names. */
export type JoinTarget = { kind: 'game'; game: LobbyGame } | { kind: 'code'; code: string }

/** A guest being asked for a name before `target` is joined. */
export interface Asking {
  target: JoinTarget
  opener: HTMLElement | null
  /** False from the moment the question is answered or cancelled until its dialog has finished closing. */
  open: boolean
}

/** A failure the page shows beside the list. */
export interface PageFailure {
  game: LobbyGame
  text: string
}

/** What the page reads and calls to join games from the list and with a code. */
export interface JoinGame {
  start: OnJoin
  startCode: OnJoinCode
  /** The id of the game whose request is on the way, or null; that row's Join spins. */
  joining: string | null
  /** A request with a typed code is on its way. */
  joiningCode: boolean
  /** Some request is on its way; every Join, a row's and the code's, is disabled while one is. */
  busy: boolean
  /** The guest's question, or null. */
  asking: Asking | null
  /** A failure the guest can answer by trying again, shown in their dialog. */
  dialogFailure: string | null
  /** A failure for the page: a one-click join that failed, or a seat that is gone. */
  failure: PageFailure | null
  /** Why a join with a code failed and could not go on in a dialog: shown under the code field. */
  codeProblem: string | null
  /** The guest's answer: the name to join under. */
  submit(name: string): void
  /** The guest backs out. */
  cancel(): void
  /** The dialog has finished closing. */
  closed(): void
  /** Hides the page's failure. */
  dismiss(): void
  /** Hides the code's failure, as the typed code changes. */
  dismissCode(): void
}

/**
 * Joining a game of the list or the game a typed code names, the same way. A signed-in visitor joins in one click and
 * the seat binds to the account; a guest is asked for a name first. Success opens the game page, whose id the answer
 * gives. A seat that is gone (taken, started, closed) or a code no open game has is shown beside the list or under the
 * code field and, for a row, the list is asked for again; any other failure leaves the form as it was so it can be
 * sent again. One request at a time: a press while one is out does nothing.
 */
export function useJoinGame(api: Pick<Api, 'joinGame' | 'joinByCode'>, refreshList: () => void): JoinGame {
  const navigate = useNavigate()
  const auth = useAuthStore()
  const [joining, setJoining] = useState<JoinTarget | null>(null)
  const [asking, setAsking] = useState<Asking | null>(null)
  const [dialogFailure, setDialogFailure] = useState<string | null>(null)
  const [failure, setFailure] = useState<PageFailure | null>(null)
  const [codeProblem, setCodeProblem] = useState<string | null>(null)
  // Set before the first await, so two presses in one tick cannot both send.
  const busy = useRef(false)

  const release = (): void => {
    busy.current = false
    setJoining(null)
  }

  // Opens the game page: the seat link in the fragment, or, for an account that already held a seat, the bare page.
  function open(joined: Joined): void {
    const page = { to: '/game/$gameId', params: { gameId: joined.game } } as const
    if (joined.token === null) void navigate(page)
    else void navigate({ ...page, hash: fragmentOf(seatLinkPath(joined.game, joined.seat, joined.token)) })
  }

  // Sends the join. Null once the game page is opening; otherwise why not, with the buttons free again.
  async function send(target: JoinTarget, name: string | undefined): Promise<JoinFailure | null> {
    const who = name === undefined ? {} : { name }
    let joined: Joined
    try {
      joined =
        target.kind === 'game'
          ? await api.joinGame(target.game.game, who)
          : await api.joinByCode(target.code, who)
    } catch (error) {
      release()
      return target.kind === 'game' ? joinFailure(error) : codeFailure(error)
    }
    // Left busy on purpose: the page is about to be replaced, and no second seat should be taken meanwhile.
    open(joined)
    return null
  }

  // Said where the player is looking: beside the list for a row, under the field for a code.
  function showOnPage(target: JoinTarget, problem: JoinFailure): void {
    if (target.kind === 'code') {
      setCodeProblem(problem.text)
      return
    }
    setFailure({ game: target.game, text: problem.text })
    if (problem.final) refreshList()
  }

  async function begin(target: JoinTarget, opener: HTMLElement | null): Promise<void> {
    // The first answer about the session may still be on its way; a signed-in visitor must not be asked for a name.
    if (auth.getState().status === 'loading') await auth.getState().start()
    if (auth.getState().status !== 'signed-in') {
      release()
      setAsking({ target, opener, open: true })
      return
    }
    const problem = await send(target, undefined)
    if (problem !== null) showOnPage(target, problem)
  }

  function startOn(target: JoinTarget, opener: HTMLElement | null): void {
    if (busy.current) return
    busy.current = true
    setFailure(null)
    setCodeProblem(null)
    setJoining(target)
    void begin(target, opener)
  }

  function submit(name: string): void {
    if (busy.current || asking === null) return
    const { target } = asking
    busy.current = true
    setDialogFailure(null)
    setJoining(target)
    void send(target, name).then((problem) => {
      if (problem === null) return
      if (!problem.final) {
        setDialogFailure(problem.text)
        return
      }
      setAsking((was) => (was === null ? null : { ...was, open: false }))
      showOnPage(target, problem)
    })
  }

  return {
    start: (game, opener) => startOn({ kind: 'game', game }, opener),
    startCode: (code, opener) => startOn({ kind: 'code', code }, opener),
    joining: joining?.kind === 'game' ? joining.game.game : null,
    joiningCode: joining?.kind === 'code',
    busy: joining !== null,
    asking,
    dialogFailure,
    failure,
    codeProblem,
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
    dismissCode: () => setCodeProblem(null),
  }
}
