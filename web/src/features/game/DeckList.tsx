import { SearchIcon, XIcon } from 'lucide-react'
import { useEffect, useId, useMemo, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { Skeleton } from '@/components/ui/skeleton'
import { DISC } from '@/features/builder/discs'
import { cn } from '@/lib/utils'
import { filterEntries, isFiltered, NO_FILTER } from './deckEntries'
import type { DeckEntry, DeckFilter, SupportFilter } from './deckEntries'
import { DeckOption } from './DeckOption'

/** The decks of one list as the chooser sees them. */
export type ListState =
  | { status: 'loading' }
  | { status: 'failed'; message: string }
  | { status: 'ready'; entries: readonly DeckEntry[] }

const SUPPORT_CHOICES: readonly { value: SupportFilter; label: string }[] = [
  { value: 'all', label: 'All decks' },
  { value: 'verified', label: 'Fully validated' },
  { value: 'hand', label: 'Has hand-played cards' },
]

const COLOUR_NAME: Readonly<Record<string, string>> = {
  W: 'White',
  U: 'Blue',
  B: 'Black',
  R: 'Red',
  G: 'Green',
}

interface DeckListProps {
  /** The list's name: "Your decks", "Precons". */
  label: string
  state: ListState
  /** What stands in for the list when there is no deck at all. */
  empty?: ReactNode
  /** A line under the list ("2 decks are in other formats and not shown"). */
  note?: ReactNode
  selectedId: string | null
  chosenId: string | null
  pendingId: string | null
  /** Whether the list is asked to offer colours: some deck of it knows its identity. */
  colours: boolean
  /** Commanders a view that was read knows for a deck whose row has no picture. */
  pictures(id: string): DeckEntry['commanders']
  onSelect(entry: DeckEntry): void
  /** Drops the selection, which the search or the filters hid while no row is left to stand in for it. */
  onClear?(): void
  /** A click on a row, after it selected it (a phone opens the preview). */
  onActivate?(entry: DeckEntry): void
  onChoose(entry: DeckEntry): void
  onRetry(): void
  /** Select the first deck, or the chosen one, as soon as the list has decks and none is selected. */
  autoSelect: boolean
}

function Rows() {
  return (
    <div data-slot="decks-loading" aria-busy="true" className="grid gap-2">
      <p className="sr-only" aria-live="polite">
        Loading the decks…
      </p>
      {Array.from({ length: 5 }, (_, index) => (
        <Skeleton key={index} className="h-18 rounded-lg" />
      ))}
    </div>
  )
}

/**
 * A list of decks to choose among: a search over name, set and commander, colour and support filters, and the rows as a
 * listbox the arrow keys move through (the selected row previews; Enter or a double click takes it). Loading, failure
 * and the lack of any deck are states of their own.
 */
export function DeckList({
  label,
  state,
  empty,
  note,
  selectedId,
  chosenId,
  pendingId,
  colours,
  pictures,
  onSelect,
  onClear,
  onActivate,
  onChoose,
  onRetry,
  autoSelect,
}: DeckListProps) {
  const [filter, setFilter] = useState<DeckFilter>(NO_FILTER)
  const searchId = useId()
  const list = useRef<HTMLUListElement>(null)
  const entries = state.status === 'ready' ? state.entries : null
  const shown = useMemo(() => (entries === null ? [] : filterEntries(entries, filter)), [entries, filter])

  // A selection the search or a filter hides moves to the first row shown, or goes, so the preview and "Use this deck"
  // never stay on a deck the list no longer shows.
  useEffect(() => {
    if (!autoSelect || entries === null) return
    if (selectedId !== null && shown.some((entry) => entry.id === selectedId)) return
    const first = shown.find((entry) => entry.id === chosenId) ?? shown[0]
    if (first !== undefined) onSelect(first)
    else if (selectedId !== null) onClear?.()
  }, [autoSelect, entries, selectedId, shown, chosenId, onSelect, onClear])

  if (state.status === 'loading') return <Rows />
  if (state.status === 'failed') {
    return (
      <div className="grid justify-items-start gap-2">
        <p data-slot="decks-failed" className="text-sm [overflow-wrap:anywhere]">
          {state.message}
        </p>
        <Button type="button" variant="outline" size="sm" onClick={onRetry}>
          Try again
        </Button>
      </div>
    )
  }
  if (state.entries.length === 0) return <>{empty}</>

  const move = (key: string, from: string): void => {
    const at = shown.findIndex((entry) => entry.id === from)
    const next =
      key === 'Home'
        ? 0
        : key === 'End'
          ? shown.length - 1
          : Math.min(shown.length - 1, Math.max(0, at + (key === 'ArrowDown' ? 1 : -1)))
    const entry = shown[next]
    if (entry === undefined) return
    onSelect(entry)
    list.current?.querySelectorAll<HTMLElement>('[role="option"]')[next]?.focus()
  }
  const toggle = (letter: string): void =>
    setFilter((was) => ({
      ...was,
      colours: was.colours.includes(letter)
        ? was.colours.filter((entry) => entry !== letter)
        : [...was.colours, letter],
    }))
  const anchor = shown.some((entry) => entry.id === selectedId) ? selectedId : (shown[0]?.id ?? null)

  return (
    <div data-slot="deck-list" className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="grid gap-2">
        <div className="relative">
          <SearchIcon
            aria-hidden="true"
            className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
          />
          <Input
            id={searchId}
            type="search"
            autoComplete="off"
            aria-label={`Search ${label.toLowerCase()}`}
            className="pl-8"
            placeholder="Name, set or commander"
            value={filter.query}
            onChange={(event) => setFilter((was) => ({ ...was, query: event.target.value }))}
          />
        </div>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
          {colours ? (
            <div role="group" aria-label="Color identity" className="flex items-center gap-1">
              {Object.keys(COLOUR_NAME).map((letter) => {
                const on = filter.colours.includes(letter)
                return (
                  <button
                    key={letter}
                    type="button"
                    aria-pressed={on}
                    aria-label={COLOUR_NAME[letter]}
                    onClick={() => toggle(letter)}
                    className={cn(
                      'grid size-7 place-items-center rounded-full text-xs font-semibold ring-1 ring-foreground/25 transition-opacity pointer-coarse:size-11',
                      DISC[letter],
                      on ? 'ring-2 ring-primary' : 'opacity-50 hover:opacity-100',
                    )}
                  >
                    {letter}
                  </button>
                )
              })}
            </div>
          ) : null}
          <NativeSelect
            aria-label="Support"
            size="sm"
            value={filter.support}
            className="min-w-0 flex-1"
            onChange={(event) =>
              setFilter((was) => ({ ...was, support: event.target.value as SupportFilter }))
            }
          >
            {SUPPORT_CHOICES.map((choice) => (
              <NativeSelectOption key={choice.value} value={choice.value}>
                {choice.label}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </div>
        <p
          data-slot="deck-count"
          aria-live="polite"
          className="flex items-center gap-2 text-xs text-muted-foreground"
        >
          {shown.length === 0
            ? 'No deck matches.'
            : isFiltered(filter)
              ? `${shown.length} of ${state.entries.length} decks`
              : `${state.entries.length} ${state.entries.length === 1 ? 'deck' : 'decks'}`}
          {isFiltered(filter) ? (
            <Button type="button" variant="ghost" size="xs" onClick={() => setFilter(NO_FILTER)}>
              <XIcon aria-hidden="true" />
              Clear filters
            </Button>
          ) : null}
        </p>
      </div>
      <ul
        ref={list}
        role="listbox"
        aria-label={label}
        data-slot="deck-options"
        className="grid min-h-0 flex-1 content-start gap-2 overflow-y-auto overscroll-contain pr-1 pb-1"
      >
        {shown.map((entry) => (
          <DeckOption
            key={entry.id}
            entry={entry}
            selected={entry.id === selectedId}
            chosen={entry.id === chosenId}
            pending={entry.id === pendingId}
            tabStop={entry.id === anchor}
            fallback={pictures(entry.id)}
            onSelect={(picked) => {
              onSelect(picked)
              onActivate?.(picked)
            }}
            onChoose={onChoose}
            onMove={(key) => move(key, entry.id)}
          />
        ))}
      </ul>
      {note}
    </div>
  )
}
