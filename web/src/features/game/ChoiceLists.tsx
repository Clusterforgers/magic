import { Link } from '@tanstack/react-router'
import { SearchIcon } from 'lucide-react'
import { useId, useState } from 'react'
import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useDecks } from '@/features/create/useDecks'
import { cardsText, formatLabel } from '@/features/decks/format'
import { useDeckList } from '@/features/decks/useDeckList'
import type { GameFormat } from '@/lib/houseRules'
import type { DecksApi } from '@/net/decks'
import type { PreconsApi } from '@/net/precons'
import type { PreconListItem, SeatDeck } from '@/protocol/generated'
import { deckFormat } from './deckWords'
import { DeckRow } from './DeckRow'
import type { DeckChoice } from './useDeckChoice'
import { usePreconChoices } from './usePreconChoices'
import { useVerdicts } from './useVerdicts'
import type { VerdictState } from './useVerdicts'
import type { LobbyApi } from './useLobbyControls'

/** Decks shown before "Show more": every one shown is checked at the table. */
const PAGE = 12

const LIST = 'grid grid-cols-1 gap-2 sm:grid-cols-2'

const RUNS: VerdictState = { status: 'ready', verdict: { kind: 'runs' } }

/** What a list needs of the page. */
interface ListProps {
  game: string
  format: GameFormat
  current: SeatDeck | null | undefined
  busy: boolean
  choice: DeckChoice
}

interface Option {
  id: string
  name: string
  format: string
}

/** The option that marks the seat's deck: the one the last answer named, else the one deck with that name and format. */
function markedId(options: readonly Option[], current: SeatDeck | null | undefined, chosenId: string | null) {
  if (chosenId !== null && options.some((option) => option.id === chosenId)) return chosenId
  if (current === null || current === undefined) return null
  const same = options.filter((option) => option.name === current.name && option.format === current.format)
  return same.length === 1 ? (same[0]?.id ?? null) : null
}

function Failed({ message, onRetry }: { message: string; onRetry(): void }) {
  return (
    <div className="grid justify-items-start gap-2">
      <p data-slot="decks-failed" className="text-sm [overflow-wrap:anywhere]">
        {message}
      </p>
      <Button type="button" variant="outline" size="sm" onClick={onRetry}>
        Try again
      </Button>
    </div>
  )
}

function Note({ slot, children }: { slot: string; children: ReactNode }) {
  return (
    <p data-slot={slot} className="text-sm text-muted-foreground">
      {children}
    </p>
  )
}

interface PagedProps {
  total: number
  shown: number
  onMore(): void
}

function More({ total, shown, onMore }: PagedProps) {
  if (total <= shown) return null
  return (
    <Button type="button" variant="outline" size="sm" className="justify-self-start" onClick={onMore}>
      Show more ({total - shown} left)
    </Button>
  )
}

/** The built-in decks: Normal decks the whole game runs, so no check is asked. */
export function BuiltInList({
  api,
  current,
  busy,
  choice,
  format,
}: ListProps & { api: Pick<LobbyApi, 'listDecks'> }) {
  const { state, reload } = useDecks(api)
  if (format !== 'normal') {
    return (
      <Note slot="decks-wrong-format">The built-in decks are Normal decks. This is a Commander game.</Note>
    )
  }
  if (state.status === 'loading') return <Note slot="decks-loading">Loading the built-in decks…</Note>
  if (state.status === 'failed') return <Failed message={state.message} onRetry={reload} />
  const options = state.decks.map((deck) => ({ id: deck.id, name: deck.name, format: 'normal' }))
  const marked = markedId(options, current, choice.chosenId)
  return (
    <ul aria-label="Built-in decks" className={LIST}>
      {state.decks.map((deck) => (
        <DeckRow
          key={deck.id}
          id={deck.id}
          name={deck.name}
          meta={`${deckFormat('normal')}, ${deck.color}, ${cardsText(deck.size)}`}
          verdict={RUNS}
          selected={choice.pending === null ? marked === deck.id : choice.pending.id === deck.id}
          pending={choice.pending?.id === deck.id}
          waiting={busy}
          onPress={choice.choose}
          onRetry={() => undefined}
        />
      ))}
    </ul>
  )
}

/** The signed-in account's decks that the game's format takes, each with its verdict at this table. */
export function OwnList({ decks, game, format, current, busy, choice }: ListProps & { decks: DecksApi }) {
  const { state, reload } = useDeckList(decks)
  const [shown, setShown] = useState(PAGE)
  const fits =
    state.status === 'ready'
      ? state.decks.filter((deck) => (format === 'commander') === (deck.format === 'commander'))
      : []
  const visible = fits.slice(0, shown)
  const verdicts = useVerdicts(
    decks,
    game,
    visible.map((deck) => deck.id),
  )
  if (state.status === 'unavailable') return null
  if (state.status === 'loading') return <Note slot="decks-loading">Loading your decks…</Note>
  if (state.status === 'failed') return <Failed message={state.message} onRetry={reload} />
  const hidden = state.decks.length - fits.length
  if (state.decks.length === 0) {
    return (
      <Note slot="decks-empty">
        You have no saved decks yet. Build one on the{' '}
        <Link
          to="/decks"
          target="_blank"
          className="font-medium text-foreground underline underline-offset-3"
        >
          Decks page
        </Link>
        , then come back and choose it.
      </Note>
    )
  }
  const marked = markedId(fits, current, choice.chosenId)
  return (
    <div className="grid gap-2">
      {fits.length === 0 ? (
        <Note slot="decks-none-fit">
          None of your decks is a {format === 'commander' ? 'Commander deck' : 'Normal-format deck'}.
        </Note>
      ) : (
        <ul aria-label="Your decks" className={LIST}>
          {visible.map((deck) => (
            <DeckRow
              key={deck.id}
              id={deck.id}
              name={deck.name}
              meta={`${formatLabel(deck.format)}, ${cardsText(deck.summary.counts.total)}`}
              verdict={verdicts.of.get(deck.id) ?? { status: 'checking' }}
              selected={choice.pending === null ? marked === deck.id : choice.pending.id === deck.id}
              pending={choice.pending?.id === deck.id}
              waiting={busy}
              onPress={choice.choose}
              onRetry={verdicts.retry}
            />
          ))}
        </ul>
      )}
      <More total={fits.length} shown={shown} onMore={() => setShown((n) => n + PAGE)} />
      {hidden > 0 ? (
        <Note slot="decks-hidden">
          {hidden} {hidden === 1 ? 'deck is' : 'decks are'} in other formats and not shown.
        </Note>
      ) : null}
    </div>
  )
}

const words = (text: string): string[] =>
  text
    .toLowerCase()
    .split(/\s+/)
    .filter((word) => word !== '')

/** The precons that match what was typed (every word in the name, set or a commander) and that the format takes. */
function matchingPrecons(
  precons: readonly PreconListItem[],
  query: string,
  format: GameFormat,
): PreconListItem[] {
  const wanted = words(query)
  return precons.filter((precon) => {
    if ((format === 'commander') !== precon.commanders.length > 0) return false
    const haystack = [precon.name, precon.set, precon.deck_type ?? '', ...precon.commanders]
      .join(' ')
      .toLowerCase()
    return wanted.every((word) => haystack.includes(word))
  })
}

/** Precons with a search box; the format decides which can be taken at all. */
export function PreconList({
  precons,
  game,
  format,
  current,
  busy,
  choice,
  checks,
}: ListProps & { precons: Pick<PreconsApi, 'list'>; checks: Pick<DecksApi, 'check'> }) {
  const { state, reload } = usePreconChoices(precons)
  const [query, setQuery] = useState('')
  const [shown, setShown] = useState(PAGE)
  const searchId = useId()
  const matches = state.status === 'ready' ? matchingPrecons(state.precons, query, format) : []
  const visible = matches.slice(0, shown)
  const verdicts = useVerdicts(
    checks,
    game,
    visible.map((precon) => `precon:${precon.code}`),
  )
  if (state.status === 'loading') return <Note slot="decks-loading">Loading the precons…</Note>
  if (state.status === 'failed') return <Failed message={state.message} onRetry={reload} />
  const options = matches.map((precon) => ({
    id: `precon:${precon.code}`,
    name: precon.name,
    format: precon.format,
  }))
  const marked = markedId(options, current, choice.chosenId)
  return (
    <div className="grid gap-3">
      <div className="grid gap-1">
        <Label htmlFor={searchId}>Search precons</Label>
        <div className="relative">
          <SearchIcon
            aria-hidden="true"
            className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
          />
          <Input
            id={searchId}
            type="search"
            autoComplete="off"
            className="pl-8"
            placeholder="Name, set or commander"
            value={query}
            onChange={(event) => {
              setQuery(event.target.value)
              setShown(PAGE)
            }}
          />
        </div>
        <p data-slot="precon-count" aria-live="polite" className="text-xs text-muted-foreground">
          {matches.length === 0
            ? 'No precon matches.'
            : `${matches.length} ${matches.length === 1 ? 'precon' : 'precons'} for a ${format === 'commander' ? 'Commander' : 'Normal'} game.`}
        </p>
      </div>
      {matches.length === 0 ? null : (
        <ul aria-label="Precons" className={LIST}>
          {visible.map((precon) => {
            const id = `precon:${precon.code}`
            return (
              <DeckRow
                key={precon.code}
                id={id}
                name={precon.name}
                meta={
                  <>
                    {[precon.set.toUpperCase(), precon.deck_type, cardsText(precon.card_count)]
                      .filter((part) => part !== null)
                      .join(', ')}
                    {precon.commanders.length === 0 ? '' : `. ${precon.commanders.join(' and ')}`}
                  </>
                }
                verdict={verdicts.of.get(id) ?? { status: 'checking' }}
                selected={choice.pending === null ? marked === id : choice.pending.id === id}
                pending={choice.pending?.id === id}
                waiting={busy}
                onPress={choice.choose}
                onRetry={verdicts.retry}
              />
            )
          })}
        </ul>
      )}
      <More total={matches.length} shown={shown} onMore={() => setShown((n) => n + PAGE)} />
    </div>
  )
}
