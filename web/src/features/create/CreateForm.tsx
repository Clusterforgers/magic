import { Link, useNavigate } from '@tanstack/react-router'
import { Plus } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { FormEvent } from 'react'
import { Badge } from '@/components/ui/badge'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { useAuth, useAuthStore } from '@/features/auth/context'
import { saveInvites } from '@/lib/invites'
import { MAX_SEATS } from '@/lib/limits'
import { fragmentOf, seatLinkPath } from '@/lib/seatLink'
import type { Api, CreatedGame, CreateGameRequest, DeckInfo } from '@/net/api'
import { describeApiError } from './apiErrors'
import {
  addSeat,
  defaultDraft,
  editSeat,
  nameFieldId,
  problemCount,
  problemsOf,
  removeSeat,
  toRequest,
} from './form'
import type { Draft } from './form'
import { GameCreated } from './GameCreated'
import { SeatFields } from './SeatFields'

interface CreateFormProps {
  api: Api
  decks: DeckInfo[]
}

interface Made {
  created: CreatedGame
  request: CreateGameRequest
}

/** The form for a new game, and, once the server has made it, the links to its seats. */
export function CreateForm({ api, decks }: CreateFormProps) {
  const navigate = useNavigate()
  const auth = useAuthStore()
  const account = useAuth((state) => state.user?.username ?? null)
  // A signed-in creator owns seat 1: it starts out named after the account, and the server saves it to the account.
  const [draft, setDraft] = useState<Draft>(() => defaultDraft(decks, account ?? undefined))
  const [showProblems, setShowProblems] = useState(false)
  const [pending, setPending] = useState(false)
  const [failure, setFailure] = useState<string | null>(null)
  const [made, setMade] = useState<Made | null>(null)
  const [focusProblem, setFocusProblem] = useState(0)
  // The seat whose name field gets focus once the next render has put it on screen.
  const focusAfterRender = useRef<number | null>(null)
  const formRef = useRef<HTMLFormElement>(null)
  const failureRef = useRef<HTMLDivElement>(null)

  const problems = problemsOf(draft, decks)
  const troubled = problemCount(problems)

  useEffect(() => {
    const id = focusAfterRender.current
    if (id === null) return
    focusAfterRender.current = null
    formRef.current?.querySelector<HTMLElement>(`#${nameFieldId(id)}`)?.focus()
  }, [draft])

  useEffect(() => {
    if (focusProblem > 0) formRef.current?.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus()
  }, [focusProblem])

  useEffect(() => {
    if (failure !== null) failureRef.current?.focus()
  }, [failure])

  function add(): void {
    const next = addSeat(draft, decks)
    if (next === draft) return
    focusAfterRender.current = next.nextId - 1
    setDraft(next)
  }

  function remove(id: number): void {
    const index = draft.seats.findIndex((seat) => seat.id === id)
    const next = removeSeat(draft, id)
    if (next === draft) return
    // The removed row's button is gone, so keep keyboard focus on the seat that took its place.
    focusAfterRender.current = next.seats[Math.min(index, next.seats.length - 1)]?.id ?? null
    setDraft(next)
  }

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    if (pending) return
    if (troubled > 0 || decks.length === 0) {
      setShowProblems(true)
      setFocusProblem((n) => n + 1)
      return
    }
    const request = toRequest(draft)
    setPending(true)
    setFailure(null)
    try {
      const created = await api.createGame(request)
      saveInvites({
        game: created.game,
        seats: created.seats.map((seat) => ({
          seat: seat.seat,
          name: request.seats[seat.seat]?.name ?? `Seat ${seat.seat + 1}`,
          token: seat.token,
        })),
      })
      setMade({ created, request })
      // The new game is on the account's list now; asking also finds out if the session ended, which would have
      // made this a guest's game.
      if (account !== null) void auth.getState().loadGames()
    } catch (error) {
      setFailure(describeApiError(error))
    } finally {
      setPending(false)
    }
  }

  if (made !== null) {
    return (
      <GameCreated
        created={made.created}
        request={made.request}
        decks={decks}
        origin={window.location.origin}
        onAnother={() => setMade(null)}
        onOpenSeat={(seat, token) =>
          void navigate({
            to: '/game/$gameId',
            params: { gameId: made.created.game },
            hash: fragmentOf(seatLinkPath(made.created.game, seat, token)),
          })
        }
      />
    )
  }

  return (
    <form
      ref={formRef}
      noValidate
      aria-labelledby="create-title"
      className="grid gap-6"
      onSubmit={(event) => void submit(event)}
    >
      <div>
        <h1 id="create-title" className="text-2xl font-semibold tracking-tight">
          Create a game
        </h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Choose how many seats there are, then give each a name and a deck. You get one link per seat to send
          to your friends.
        </p>
        {account === null ? null : (
          <p data-slot="signed-in-note" className="mt-2 text-sm break-words">
            Signed in as <strong className="font-medium">{account}</strong>. You will play as{' '}
            <strong className="font-medium">{draft.seats[0]?.name.trim() || account}</strong> in seat 1; the
            other links are for your friends. The game will be listed under{' '}
            <Link to="/games" className="font-medium underline underline-offset-3">
              My games
            </Link>
            .
          </p>
        )}
      </div>

      <fieldset className="grid gap-2">
        <legend className="mb-1 text-sm font-medium">Format</legend>
        <div className="flex flex-wrap gap-2">
          <label className="flex items-center gap-2 rounded-lg px-3 py-2 text-sm ring-1 ring-foreground/20 has-checked:bg-muted has-checked:ring-primary">
            <input type="radio" name="format" value="normal" defaultChecked className="accent-primary" />
            Normal
          </label>
          <label className="flex items-center gap-2 rounded-lg px-3 py-2 text-sm text-muted-foreground opacity-70 ring-1 ring-foreground/10">
            <input type="radio" name="format" value="commander" disabled />
            Commander
            <Badge variant="secondary">coming soon</Badge>
          </label>
        </div>
      </fieldset>

      <section aria-labelledby="seats-title" className="grid gap-3">
        <div className="flex items-center justify-between gap-2">
          <h2 id="seats-title" className="text-sm font-medium">
            Seats
          </h2>
          <p className="text-sm text-muted-foreground" aria-live="polite">
            {draft.seats.length} of {MAX_SEATS} seats
          </p>
        </div>
        <ol className="grid gap-3">
          {draft.seats.map((seat, index) => (
            <SeatFields
              key={seat.id}
              position={index + 1}
              seat={seat}
              decks={decks}
              problems={showProblems ? (problems.get(seat.id) ?? null) : null}
              canRemove={draft.seats.length > 2}
              onEdit={(id, change) => setDraft((current) => editSeat(current, id, change))}
              onRemove={remove}
            />
          ))}
        </ol>
        <div>
          <Button type="button" variant="outline" disabled={draft.seats.length >= MAX_SEATS} onClick={add}>
            <Plus aria-hidden="true" />
            Add seat
          </Button>
        </div>
      </section>

      {showProblems && troubled > 0 ? (
        <Alert variant="destructive">
          <AlertDescription className="text-destructive">
            {troubled === 1 ? 'One seat needs' : `${troubled} seats need`} fixing before the game can be
            created.
          </AlertDescription>
        </Alert>
      ) : null}
      {failure === null ? null : (
        <Alert ref={failureRef} tabIndex={-1} variant="destructive" className="outline-none">
          <AlertDescription className="text-destructive">{failure}</AlertDescription>
        </Alert>
      )}

      <div>
        <Button type="submit" size="lg" disabled={pending}>
          {pending ? 'Creating the game…' : 'Create game'}
        </Button>
      </div>
    </form>
  )
}
