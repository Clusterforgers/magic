import { Link } from '@tanstack/react-router'
import { LibraryIcon } from 'lucide-react'
import { useMemo } from 'react'
import type { ReactNode } from 'react'
import { EmptyState } from '@/components/EmptyState'
import { buttonVariants } from '@/components/ui/button'
import { useDecks } from '@/features/create/useDecks'
import { useDeckList } from '@/features/decks/useDeckList'
import type { GameFormat } from '@/lib/houseRules'
import type { DecksApi } from '@/net/decks'
import type { PreconsApi } from '@/net/precons'
import type { SeatDeck } from '@/protocol/generated'
import { entryOfBuiltIn, entryOfDeck, entryOfPrecon, fitsFormat, markedId } from './deckEntries'
import type { DeckEntry } from './deckEntries'
import { DeckList } from './DeckList'
import type { ListState } from './DeckList'
import type { DeckChoice } from './useDeckChoice'
import type { LobbyApi } from './useLobbyControls'
import { usePreconChoices } from './usePreconChoices'

/** What every source of decks takes from the chooser. */
interface SourceProps {
  format: GameFormat
  current: SeatDeck | null | undefined
  choice: DeckChoice
  selectedId: string | null
  /** This source's tab is the one shown: only that list selects a deck by itself. */
  active: boolean
  pictures(id: string): DeckEntry['commanders']
  onSelect(entry: DeckEntry): void
  /** Drops the selection, which a search or filter hid and no row stands in for. */
  onClear(): void
  onActivate(entry: DeckEntry): void
  onChoose(entry: DeckEntry): void
}

function Note({ slot, children }: { slot: string; children: ReactNode }) {
  return (
    <p data-slot={slot} className="text-sm text-muted-foreground">
      {children}
    </p>
  )
}

function List({
  label,
  state,
  entries,
  retry,
  empty,
  note,
  colours = false,
  ...source
}: Omit<SourceProps, 'format'> & {
  label: string
  state: ListState
  entries: readonly DeckEntry[]
  retry(): void
  empty?: ReactNode
  note?: ReactNode
  colours?: boolean
}) {
  const chosenId = markedId(entries, source.current, source.choice.chosenId)
  return (
    <DeckList
      label={label}
      state={state}
      empty={empty}
      note={note}
      selectedId={source.selectedId}
      chosenId={chosenId}
      pendingId={source.choice.pending?.id ?? null}
      colours={colours}
      pictures={source.pictures}
      onSelect={source.onSelect}
      onClear={source.onClear}
      onActivate={source.onActivate}
      onChoose={source.onChoose}
      onRetry={retry}
      autoSelect={source.active}
    />
  )
}

/** The built-in decks: Normal decks the whole game runs, listed by name since they carry no card data. */
export function BuiltInSource({ api, ...source }: SourceProps & { api: Pick<LobbyApi, 'listDecks'> }) {
  const { state, reload } = useDecks(api)
  const entries = useMemo(() => (state.status === 'ready' ? state.decks.map(entryOfBuiltIn) : []), [state])
  if (source.format !== 'normal') {
    return (
      <Note slot="decks-wrong-format">The built-in decks are Normal decks. This is a Commander game.</Note>
    )
  }
  const list: ListState =
    state.status === 'ready'
      ? { status: 'ready', entries }
      : state.status === 'failed'
        ? { status: 'failed', message: state.message }
        : { status: 'loading' }
  return <List label="Built-in decks" state={list} entries={entries} retry={reload} {...source} />
}

/** The signed-in account's decks that the game's format takes. */
export function OwnSource({ decks, ...source }: SourceProps & { decks: DecksApi }) {
  const { state, reload } = useDeckList(decks)
  const commander = source.format === 'commander'
  const entries = useMemo(
    () =>
      state.status === 'ready'
        ? state.decks.filter((deck) => fitsFormat(deck, commander)).map(entryOfDeck)
        : [],
    [state, commander],
  )
  if (state.status === 'unavailable') return null
  const total = state.status === 'ready' ? state.decks.length : 0
  const list: ListState =
    state.status === 'ready'
      ? { status: 'ready', entries }
      : state.status === 'failed'
        ? { status: 'failed', message: state.message }
        : { status: 'loading' }
  const hidden = total - entries.length
  return (
    <List
      label="Your decks"
      state={list}
      entries={entries}
      retry={reload}
      colours={entries.some((entry) => entry.identity !== null)}
      empty={
        total === 0 ? (
          <EmptyState
            icon={LibraryIcon}
            as="h3"
            title="You have no saved decks yet"
            action={
              <Link
                to="/decks"
                target="_blank"
                className={buttonVariants({ variant: 'outline', size: 'sm' })}
              >
                Build or import a deck
              </Link>
            }
          >
            Build one in the deck builder or import a list, then come back and choose it. The precons are one
            tab away.
          </EmptyState>
        ) : (
          <Note slot="decks-none-fit">
            None of your decks is a {commander ? 'Commander deck' : 'Normal-format deck'}.
          </Note>
        )
      }
      note={
        hidden > 0 && entries.length > 0 ? (
          <Note slot="decks-hidden">
            {hidden} {hidden === 1 ? 'deck is' : 'decks are'} in other formats and not shown.
          </Note>
        ) : null
      }
      {...source}
    />
  )
}

/** The precons that the game's format takes. */
export function PreconSource({ precons, ...source }: SourceProps & { precons: Pick<PreconsApi, 'list'> }) {
  const { state, reload } = usePreconChoices(precons)
  const commander = source.format === 'commander'
  const entries = useMemo(
    () =>
      state.status === 'ready'
        ? state.precons.filter((precon) => fitsFormat(precon, commander)).map(entryOfPrecon)
        : [],
    [state, commander],
  )
  const list: ListState =
    state.status === 'ready'
      ? { status: 'ready', entries }
      : state.status === 'failed'
        ? { status: 'failed', message: state.message }
        : { status: 'loading' }
  return (
    <List
      label="Precons"
      state={list}
      entries={entries}
      retry={reload}
      empty={<Note slot="decks-none-fit">No precon fits a {commander ? 'Commander' : 'Normal'} game.</Note>}
      {...source}
    />
  )
}
