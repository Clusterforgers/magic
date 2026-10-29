import { Link, useNavigate } from '@tanstack/react-router'
import { CircleAlertIcon } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { FormEvent } from 'react'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { useAuth, useAuthStore } from '@/features/auth/context'
import { saveInvites } from '@/lib/invites'
import { fragmentOf, seatLinkPath } from '@/lib/seatLink'
import type { Api, CreatedGame, CreateGameRequest, DeckInfo } from '@/net/api'
import { AdvancedOptions } from './AdvancedOptions'
import { describeApiError } from './apiErrors'
import {
  addSeat,
  chooseFormat,
  chooseLinks,
  chooseVisibility,
  defaultDraft,
  editRule,
  editSeat,
  nameFieldId,
  problemCount,
  problemsOf,
  removeSeat,
  resizeSeats,
  ruleProblems,
  toRequest,
} from './form'
import type { Draft } from './form'
import { GameCreated } from './GameCreated'
import { FormatField } from './FormatField'
import { HostFields } from './HostFields'
import { HouseRulesField } from './HouseRulesField'
import { SeatList } from './SeatList'
import { VisibilityField } from './VisibilityField'

interface CreateFormProps {
  api: Api
  decks: DeckInfo[]
}

interface Made {
  created: CreatedGame
  request: CreateGameRequest
}

/** The form for a new game; once the server has made it, the game's lobby, or the links to every seat. */
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
  const rules = ruleProblems(draft)
  const troubled = problemCount(problems) + Object.keys(rules).length

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

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    if (pending) return
    if (troubled > 0 || (draft.format === 'normal' && decks.length === 0)) {
      setShowProblems(true)
      setFocusProblem((n) => n + 1)
      return
    }
    const request = toRequest(draft)
    setPending(true)
    setFailure(null)
    try {
      const created = await api.createGame(request)
      // The server shows the code once. A game with a link per seat has no open seat to take by it, so none is kept.
      saveInvites({
        game: created.game,
        ...(draft.links || created.code === undefined ? {} : { code: created.code }),
        seats: created.seats.map((seat) => ({
          seat: seat.seat,
          name: request.seats[seat.seat]?.name ?? `Seat ${seat.seat + 1}`,
          token: seat.token,
        })),
      })
      // The new game is on the account's list now; asking also finds out if the session ended, which would have
      // made this a guest's game.
      if (account !== null) void auth.getState().loadGames()
      if (draft.links || !openLobby(created)) setMade({ created, request })
    } catch (error) {
      setFailure(describeApiError(error))
    } finally {
      setPending(false)
    }
  }

  if (made !== null) {
    return (
      // The page is wide for the form's two columns; the links to share read best at the width of a page.
      <div className="mx-auto w-full max-w-page">
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
      </div>
    )
  }

  const shown = showProblems ? problems : null
  const hostName = draft.seats[0]?.name.trim() || account
  return (
    <form
      ref={formRef}
      noValidate
      aria-labelledby="create-title"
      className="grid gap-5 [&_:is(input,select,summary)]:scroll-mb-24"
      onSubmit={(event) => void submit(event)}
    >
      <div>
        <h1 id="create-title" className="text-2xl font-semibold tracking-tight">
          Create a game
        </h1>
        <p className="mt-1 text-base text-muted-foreground">
          {draft.links
            ? 'Choose how many seats there are, then give each a name and a deck. You get one link per seat to send to your friends.'
            : 'Your friends take the other seats from the game browser or with a link or code.'}
        </p>
        {account === null ? null : (
          <p data-slot="signed-in-note" className="mt-2 text-sm break-words">
            Signed in as <strong className="font-medium">{account}</strong>. You will play as{' '}
            <strong className="font-medium">{hostName}</strong> in seat 1
            {draft.links ? '; the other links are for your friends' : ''}. The game will be listed under{' '}
            <Link to="/games" className="font-medium underline underline-offset-3">
              My games
            </Link>
            .
          </p>
        )}
      </div>

      <div className="grid gap-5 md:grid-cols-2 md:items-start md:gap-x-8" data-slot="create-columns">
        <div className="grid content-start gap-5" data-slot="create-game-column">
          <FormatField
            value={draft.format}
            onChange={(format) => setDraft((current) => chooseFormat(current, decks, format))}
          />
          {draft.links ? (
            <SeatList
              draft={draft}
              decks={decks}
              problems={shown}
              onEdit={(id, change) => setDraft((current) => editSeat(current, id, change))}
              onAdd={add}
              onRemove={remove}
            />
          ) : (
            <HostFields
              draft={draft}
              decks={decks}
              problems={shown?.get(draft.seats[0]?.id ?? -1) ?? null}
              onEdit={(id, change) => setDraft((current) => editSeat(current, id, change))}
              onCount={(count) => setDraft((current) => resizeSeats(current, decks, count))}
            />
          )}
        </div>
        <div className="grid content-start gap-5" data-slot="create-options-column">
          {draft.links ? null : (
            <VisibilityField
              value={draft.visibility}
              onChange={(visibility) => setDraft((current) => chooseVisibility(current, visibility))}
            />
          )}
          <HouseRulesField
            draft={draft}
            problems={showProblems ? rules : null}
            onChange={(id, text) => setDraft((current) => editRule(current, id, text))}
          />
          <AdvancedOptions
            links={draft.links}
            linksAllowed={draft.format === 'normal'}
            onLinks={(links) => setDraft((current) => chooseLinks(current, links))}
          />
        </div>
      </div>

      {showProblems && troubled > 0 ? (
        <Alert variant="destructive">
          <CircleAlertIcon aria-hidden="true" />
          <AlertDescription className="text-foreground">
            {draft.links
              ? `${troubled === 1 ? 'One seat needs' : `${troubled} seats need`} fixing before the game can be created.`
              : Object.keys(rules).length > 0 && problemCount(problems) === 0
                ? 'Fix the house rules marked below before the game can be created.'
                : 'Fix your name, deck or house rules before the game can be created.'}
          </AlertDescription>
        </Alert>
      ) : null}
      {failure === null ? null : (
        <Alert ref={failureRef} tabIndex={-1} variant="destructive" className="outline-none">
          <CircleAlertIcon aria-hidden="true" />
          <AlertDescription className="text-foreground">{failure}</AlertDescription>
        </Alert>
      )}

      {/* Sticks to the bottom of the window, so a long form (a phone, an open disclosure) keeps the button in reach. */}
      <div
        data-slot="create-footer"
        className="sticky bottom-0 z-10 -mx-4 border-t bg-background px-4 py-3 sm:-mx-6 sm:px-6"
      >
        <Button type="submit" size="lg" loading={pending} className="w-full sm:w-auto">
          {pending ? 'Creating the game…' : 'Create game'}
        </Button>
      </div>
    </form>
  )
}
