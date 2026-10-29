import { useNavigate } from '@tanstack/react-router'
import { useRef, useState } from 'react'
import { useAuthStore } from '@/features/auth/context'
import { saveInvites } from '@/lib/invites'
import { fragmentOf, seatLinkPath } from '@/lib/seatLink'
import type { Api, CreatedGame } from '@/net/api'
import type { PreconListItem } from '@/protocol/generated'
import { playFailure } from './errors'
import type { PlayFailure } from './errors'
import { playRequest } from './play'

/** A guest being asked for a name before a game is made from `precon`. */
export interface AskingName {
  precon: PreconListItem
  /** The button that asked, which takes focus back if the question is cancelled. */
  opener: HTMLElement | null
  /** False from the moment the question is answered or cancelled until its dialog has finished closing. */
  open: boolean
}

/** A failure the page shows above the list. */
export interface PageFailure extends PlayFailure {
  precon: PreconListItem
}

/** What the page reads and calls to play a precon. */
export interface PlayPrecon {
  /** Starts a game with `precon`; `opener` is the button that asked. */
  start(precon: PreconListItem, opener: HTMLElement | null): void
  /** The code of the precon whose game is being made, or null; that Play spins. */
  playing: string | null
  /** A request is on its way: every Play is disabled while one is. */
  busy: boolean
  /** The guest's question, or null. */
  asking: AskingName | null
  /** A failure the guest can answer by sending the same name again, shown in their dialog. */
  dialogFailure: string | null
  /** A failure for the page: a one-click start that failed, or a precon the game will not take. */
  failure: PageFailure | null
  /** The guest's answer: the name to play under. */
  submit(name: string): void
  /** The guest backs out. */
  cancel(): void
  /** The question's dialog has finished closing. */
  closed(): void
  /** Hides the page's failure. */
  dismiss(): void
}

/**
 * Making a game from a precon in one request, then opening its lobby on seat 1. A signed-in visitor plays as the
 * account in one click; a guest is asked for a name first. One request at a time: a press while one is out does
 * nothing. A failure the player can send again (offline, a busy server) stays in the guest's dialog; one that is the
 * precon's own (the game will not take it, or it is gone) shows above the list.
 */
export function usePlayPrecon(api: Pick<Api, 'createGame'>): PlayPrecon {
  const navigate = useNavigate()
  const auth = useAuthStore()
  const [playing, setPlaying] = useState<PreconListItem | null>(null)
  const [asking, setAsking] = useState<AskingName | null>(null)
  const [dialogFailure, setDialogFailure] = useState<string | null>(null)
  const [failure, setFailure] = useState<PageFailure | null>(null)
  // Set before the first await, so two presses in one tick cannot both send.
  const busy = useRef(false)

  const release = (): void => {
    busy.current = false
    setPlaying(null)
  }

  function openLobby(created: CreatedGame): boolean {
    const mine = created.seats.find((seat) => seat.seat === 0)
    if (mine === undefined) return false
    void navigate({
      to: '/game/$gameId',
      params: { gameId: created.game },
      hash: fragmentOf(seatLinkPath(created.game, mine.seat, mine.token)),
    })
    return true
  }

  // Sends the create. Null once the lobby is opening; otherwise why not, with the buttons free again.
  async function send(precon: PreconListItem, name: string): Promise<PlayFailure | null> {
    const request = playRequest(precon, name)
    let created: CreatedGame
    try {
      created = await api.createGame(request)
    } catch (error) {
      release()
      return playFailure(error, precon.name)
    }
    saveInvites({
      game: created.game,
      ...(created.code === undefined ? {} : { code: created.code }),
      seats: created.seats.map((seat) => ({
        seat: seat.seat,
        name: request.seats[seat.seat]?.name ?? name,
        token: seat.token,
      })),
    })
    // The new game is on the account's list now.
    if (auth.getState().status === 'signed-in') void auth.getState().loadGames()
    // Left busy on purpose: the page is about to be replaced, and no second game should be made meanwhile.
    if (openLobby(created)) return null
    release()
    return {
      text: 'The server made the game but gave no seat for you. Try again.',
      final: false,
      blocked: false,
    }
  }

  async function begin(precon: PreconListItem, opener: HTMLElement | null): Promise<void> {
    // The first answer about the session may still be on its way; a signed-in visitor must not be asked for a name.
    if (auth.getState().status === 'loading') await auth.getState().start()
    const { status, user } = auth.getState()
    if (status !== 'signed-in' || user === null) {
      release()
      setAsking({ precon, opener, open: true })
      return
    }
    const problem = await send(precon, user.username)
    if (problem !== null) setFailure({ ...problem, precon })
  }

  function start(precon: PreconListItem, opener: HTMLElement | null): void {
    if (busy.current) return
    busy.current = true
    setFailure(null)
    setPlaying(precon)
    void begin(precon, opener)
  }

  function submit(name: string): void {
    if (busy.current || asking === null) return
    const { precon } = asking
    busy.current = true
    setDialogFailure(null)
    setPlaying(precon)
    void send(precon, name).then((problem) => {
      if (problem === null) return
      if (!problem.final) {
        setDialogFailure(problem.text)
        return
      }
      setAsking((was) => (was === null ? null : { ...was, open: false }))
      setFailure({ ...problem, precon })
    })
  }

  return {
    start,
    playing: playing?.code ?? null,
    busy: playing !== null,
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
