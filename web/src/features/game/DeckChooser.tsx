import { Link } from '@tanstack/react-router'
import { CheckIcon, CircleAlertIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { useDecks } from '@/features/create/useDecks'
import { DeckStateBadge } from '@/features/decks/DeckStateBadge'
import { cardsText, checkNote, formatLabel } from '@/features/decks/format'
import { useDeckList } from '@/features/decks/useDeckList'
import { cn } from '@/lib/utils'
import type { DecksApi } from '@/net/decks'
import type { DeckListItem, DeckState, SeatDeck } from '@/protocol/generated'
import { deckFormat, deckWords, issueWords, NO_ISSUES } from './deckWords'
import type { DeckChoice, DeckPick } from './useDeckChoice'
import type { LobbyApi } from './useLobbyControls'

/** What the chooser asks of the page. */
export interface DeckChooserProps {
  /** The built-in decks come from here. */
  api: Pick<LobbyApi, 'listDecks'>
  /** The signed-in account's decks come from here. */
  decks: DecksApi
  /** A signed-in visitor has stored decks to choose among; a guest has only the built-in ones. */
  signedIn: boolean
  /** The deck the roster shows for this seat: null for none, undefined while no roster has said. */
  current: SeatDeck | null | undefined
  /** This seat pressed Ready, so its deck cannot change. */
  ready: boolean
  /** Some request of the lobby is out, so nothing else can be sent. */
  busy: boolean
  choice: DeckChoice
}

/** One deck the player may press. */
interface Option {
  id: string
  name: string
  format: string
}

// A deck is blocked when its marker says so or when the check of the moment does (a deck that changed since its check
// shows as unchecked, but the server will judge it by the check of now).
const isBlocked = (deck: DeckListItem): boolean =>
  deck.state === 'blocked' || deck.summary.state === 'blocked'
const stateOf = (deck: DeckListItem): DeckState => (isBlocked(deck) ? 'blocked' : deck.state)

/** The option that marks the seat's deck: the one the last answer named, else the one deck with that name and format. */
function markedId(options: readonly Option[], current: SeatDeck | null | undefined, chosenId: string | null) {
  if (chosenId !== null && options.some((option) => option.id === chosenId)) return chosenId
  if (current === null || current === undefined) return null
  const same = options.filter((option) => option.name === current.name && option.format === current.format)
  return same.length === 1 ? (same[0]?.id ?? null) : null
}

interface TileProps {
  option: Option
  selected: boolean
  pending: boolean
  /** Why the deck cannot be chosen, or null. */
  blocked: string | null
  /** Another request is out. */
  waiting: boolean
  onPress(pick: DeckPick): void
  children: ReactNode
}

function Tile({ option, selected, pending, blocked, waiting, onPress, children }: TileProps) {
  const reasonId = `deck-reason-${option.id}`
  return (
    <li data-deck-option={option.id} className="grid content-start gap-1">
      <Button
        type="button"
        variant="outline"
        aria-pressed={selected}
        aria-describedby={blocked === null ? undefined : reasonId}
        loading={pending}
        disabled={blocked !== null || (waiting && !pending)}
        focusableWhenDisabled={blocked !== null}
        className={cn(
          'h-auto min-h-11 w-full justify-start gap-3 py-2 text-left whitespace-normal',
          selected && 'border-primary',
        )}
        onClick={() => onPress({ id: option.id, name: option.name })}
      >
        {pending || !selected ? null : <CheckIcon aria-hidden="true" />}
        <span className="grid min-w-0 flex-1 gap-1">
          <span className="font-medium [overflow-wrap:anywhere]">{option.name}</span>
          <span className="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs font-normal text-muted-foreground">
            {children}
          </span>
        </span>
      </Button>
      {blocked === null ? null : (
        <p
          id={reasonId}
          data-slot="deck-blocked"
          className="px-1 text-xs text-destructive [overflow-wrap:anywhere]"
        >
          {blocked}
        </p>
      )}
    </li>
  )
}

interface GroupProps {
  title: string
  children: ReactNode
}

function Group({ title, children }: GroupProps) {
  return (
    <div className="grid gap-2">
      <h3 className="text-sm font-medium text-muted-foreground">{title}</h3>
      {children}
    </div>
  )
}

const LIST = 'grid max-h-72 grid-cols-1 gap-2 overflow-y-auto sm:grid-cols-2'

interface ListProps {
  current: SeatDeck | null | undefined
  busy: boolean
  choice: DeckChoice
}

function StoredDecks({ decks, current, busy, choice }: ListProps & { decks: DecksApi }) {
  const { state, reload } = useDeckList(decks)
  if (state.status === 'unavailable') return null
  return (
    <Group title="Your decks">
      {state.status === 'loading' ? (
        <p className="text-sm text-muted-foreground">Loading your decks…</p>
      ) : state.status === 'failed' ? (
        <div className="grid justify-items-start gap-2">
          <p data-slot="decks-failed" className="text-sm [overflow-wrap:anywhere]">
            {state.message}
          </p>
          <Button type="button" variant="outline" size="sm" onClick={reload}>
            Try again
          </Button>
        </div>
      ) : state.decks.length === 0 ? (
        <p data-slot="decks-empty" className="text-sm text-muted-foreground">
          You have no saved decks yet. Build one on the{' '}
          <Link
            to="/decks"
            target="_blank"
            className="font-medium text-foreground underline underline-offset-3"
          >
            Decks page
          </Link>
          , then come back and choose it.
        </p>
      ) : (
        <ul aria-label="Your decks" className={LIST}>
          {state.decks.map((deck) => {
            const marked = markedId(state.decks, current, choice.chosenId)
            const note = checkNote(deck)
            return (
              <Tile
                key={deck.id}
                option={deck}
                selected={choice.pending === null ? marked === deck.id : choice.pending.id === deck.id}
                pending={choice.pending?.id === deck.id}
                blocked={
                  isBlocked(deck)
                    ? `Blocked${note === null ? '' : `: ${note}`}. Fix it in the deck builder to play it.`
                    : null
                }
                waiting={busy}
                onPress={choice.choose}
              >
                <span>
                  {formatLabel(deck.format)}, {cardsText(deck.summary.counts.total)}
                </span>
                <DeckStateBadge state={stateOf(deck)} />
                {note === null || isBlocked(deck) ? null : <span>{note}</span>}
              </Tile>
            )
          })}
        </ul>
      )}
    </Group>
  )
}

function DemoDecks({ api, current, busy, choice }: ListProps & { api: Pick<LobbyApi, 'listDecks'> }) {
  const { state, reload } = useDecks(api)
  return (
    <Group title="Demo decks">
      {state.status === 'loading' ? (
        <p className="text-sm text-muted-foreground">Loading the demo decks…</p>
      ) : state.status === 'failed' ? (
        <div className="grid justify-items-start gap-2">
          <p data-slot="decks-failed" className="text-sm [overflow-wrap:anywhere]">
            {state.message}
          </p>
          <Button type="button" variant="outline" size="sm" onClick={reload}>
            Try again
          </Button>
        </div>
      ) : (
        <ul aria-label="Demo decks" className={LIST}>
          {state.decks.map((deck) => {
            const options = state.decks.map((other) => ({ id: other.id, name: other.name, format: 'normal' }))
            const marked = markedId(options, current, choice.chosenId)
            return (
              <Tile
                key={deck.id}
                option={{ id: deck.id, name: deck.name, format: 'normal' }}
                selected={choice.pending === null ? marked === deck.id : choice.pending.id === deck.id}
                pending={choice.pending?.id === deck.id}
                blocked={null}
                waiting={busy}
                onPress={choice.choose}
              >
                <span>
                  {deckFormat('normal')}, {deck.color}, {cardsText(deck.size)}
                </span>
              </Tile>
            )
          })}
        </ul>
      )}
    </Group>
  )
}

/**
 * The player's own deck: what the table is told of it, and until the seat is ready the decks to choose among, the
 * account's stored decks (each with its check state; a blocked deck is shown with the reason and cannot be pressed)
 * and the built-in ones. A guest has only the built-in decks. The choice shows at once with a spinner on the deck
 * pressed; a refusal puts the old choice back and says why, and lists the issues of a deck the game will not play.
 */
export function DeckChooser({ api, decks, signedIn, current, ready, busy, choice }: DeckChooserProps) {
  const { problem } = choice
  return (
    <section aria-labelledby="deck-title" data-slot="deck-chooser" className="grid grid-cols-1 gap-3">
      <div className="grid gap-1">
        <h2 id="deck-title" className="text-lg font-semibold">
          Your deck
        </h2>
        <p data-slot="deck-current" className="text-sm [overflow-wrap:anywhere]">
          {current === undefined
            ? null
            : current === null
              ? 'You have no deck yet. Choose one to press Ready.'
              : deckWords(current)}
        </p>
      </div>
      {ready ? (
        <p data-slot="deck-locked" className="text-sm text-muted-foreground">
          Your deck is locked while you are ready. Press Not ready to change it.
        </p>
      ) : (
        <div className="grid gap-4">
          {signedIn ? <StoredDecks decks={decks} current={current} busy={busy} choice={choice} /> : null}
          <DemoDecks api={api} current={current} busy={busy} choice={choice} />
        </div>
      )}
      <p
        data-slot="deck-note"
        aria-live="polite"
        aria-atomic="true"
        className="text-sm text-muted-foreground empty:hidden"
      >
        {choice.note}
      </p>
      {problem === null ? null : (
        <Alert variant="destructive" data-slot="deck-problem" data-blocked={problem.blocked || undefined}>
          <CircleAlertIcon aria-hidden="true" />
          <AlertDescription className="grid gap-2 text-foreground">
            <span className="[overflow-wrap:anywhere]">{problem.words}</span>
            {problem.blocked ? (
              problem.issues.length === 0 ? (
                <span>{NO_ISSUES}</span>
              ) : (
                <ul aria-label="What breaks the rules" className="list-disc pl-5">
                  {problem.issues.map((issue, index) => (
                    <li
                      key={`${issue.code}-${issue.oracle_id ?? ''}-${index}`}
                      className="[overflow-wrap:anywhere]"
                    >
                      {issueWords(issue)}
                    </li>
                  ))}
                </ul>
              )
            ) : null}
          </AlertDescription>
        </Alert>
      )}
    </section>
  )
}
