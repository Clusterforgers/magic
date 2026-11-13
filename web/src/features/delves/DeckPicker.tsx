/**
 * The Delves deck picker: the Commander decks a hero may bring as cards with their commander's art, colors, size,
 * validity and last delve, in collapsible sections with search and a color filter, beside a large preview of the deck
 * picked. Nothing is sent before "Take".
 */
import { CheckIcon, SearchIcon } from 'lucide-react'
import { useId, useMemo, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Skeleton } from '@/components/ui/skeleton'
import { timeSince } from '@/lib/timeSince'
import { cn } from '@/lib/utils'
import { firstOpen, useFolds } from '@/features/decks/groups/folds'
import { GroupToggle } from '@/features/decks/groups/GroupFolds'
import { groupOf } from '@/features/decks/groups/grouping'
import type { GroupLike } from '@/features/decks/groups/grouping'
import { lastGroup, rememberGroup } from '@/features/decks/groups/lastGroup'
import type { DeckEntry } from '@/features/game/deckEntries'
import { ColourPips, CommanderArt } from './CommanderArt'
import { COLOUR_NAME, COLOURS, deckValidity, pickerSections } from './deckPicker'
import type { LastPlayed } from './deckPicker'
import { lettersOf } from './model'

const TONE = {
  ok: 'border-success/40 text-success',
  warn: 'border-warning/40 text-warning',
  bad: 'border-destructive/40 text-destructive',
} as const

/** A deck's validity as a small chip. */
export function ValidityChip({ entry }: { entry: DeckEntry }) {
  const { tone, label } = deckValidity(entry)
  return (
    <span
      data-validity={tone}
      className={cn('inline-flex items-center rounded-full border px-2 text-xs font-medium', TONE[tone])}
    >
      {label}
    </span>
  )
}

function DeckCard({
  entry,
  picked,
  played,
  now,
  onPick,
}: {
  entry: DeckEntry
  picked: boolean
  played: string | undefined
  now: number
  onPick: () => void
}) {
  const commanders = entry.commanderNames.join(' & ')
  return (
    <li>
      <button
        type="button"
        aria-pressed={picked}
        data-deck={entry.id}
        onClick={onPick}
        className={cn(
          'group grid w-full overflow-hidden rounded-xl border bg-surface text-left shadow-e1 transition-[border-color,box-shadow,transform] duration-fast ease-out hover:border-border-strong motion-safe:hover:-translate-y-0.5',
          picked && 'border-primary ring-2 ring-primary',
        )}
      >
        <span className="relative block">
          <CommanderArt cards={entry.commanders} colours={lettersOf(entry)} className="aspect-[2/1] w-full" />
          {picked ? (
            <span className="absolute top-2 right-2 grid size-6 place-items-center rounded-full bg-primary text-on-primary shadow-e2">
              <CheckIcon aria-hidden="true" className="size-4" />
            </span>
          ) : null}
        </span>
        <span className="grid gap-1 p-3">
          <span className="flex items-start justify-between gap-2">
            <span className="min-w-0 font-medium leading-tight [overflow-wrap:anywhere]">{entry.name}</span>
            <ColourPips colours={lettersOf(entry)} />
          </span>
          {commanders !== '' && commanders !== entry.name ? (
            <span className="truncate text-xs text-muted-foreground">{commanders}</span>
          ) : null}
          <span className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            <span>{entry.cards} cards</span>
            <ValidityChip entry={entry} />
          </span>
          <span className="text-xs text-muted-foreground">
            {played === undefined ? 'Not played on a delve yet' : `Last delve ${timeSince(played, now)}`}
          </span>
        </span>
      </button>
    </li>
  )
}

/** The large preview of the deck picked: its commander's art, name, colors and facts. */
export function DeckPreview({ entry }: { entry: DeckEntry | undefined }) {
  if (entry === undefined) {
    return (
      <div className="grid min-h-48 place-items-center rounded-xl border border-dashed p-6 text-center text-sm text-muted-foreground">
        Pick a deck to see its commander.
      </div>
    )
  }
  return (
    <figure data-slot="deck-preview" className="grid overflow-hidden rounded-xl border bg-surface shadow-e2">
      <CommanderArt
        cards={entry.commanders}
        colours={lettersOf(entry)}
        className="aspect-[3/1] w-full lg:aspect-[4/3]"
        label={entry.commanderNames.length > 0 ? `Art of ${entry.commanderNames.join(' and ')}` : undefined}
      />
      <figcaption className="grid gap-2 p-4">
        <span className="flex items-start justify-between gap-2">
          <span className="text-lg font-semibold leading-tight">{entry.name}</span>
          <ColourPips colours={lettersOf(entry)} size="md" />
        </span>
        {entry.commanderNames.length > 0 ? (
          <span className="text-sm text-muted-foreground">Led by {entry.commanderNames.join(' and ')}</span>
        ) : null}
        <span className="flex flex-wrap items-center gap-2 text-sm">
          <span>{entry.cards} cards</span>
          <ValidityChip entry={entry} />
          {entry.kind === 'precon' ? <span className="text-muted-foreground">Precon</span> : null}
        </span>
      </figcaption>
    </figure>
  )
}

/** The picker's body; the lobby puts it in a dialog. `entries` is null while the lists load. */
export function DeckPicker({
  heroName,
  entries,
  groups = [],
  current,
  played,
  now,
  onTake,
  onCancel,
}: {
  heroName: string
  entries: readonly DeckEntry[] | null
  /** The player's deck groups, which their decks show in. */
  groups?: readonly GroupLike[]
  /** The deck the hero brings now. */
  current: string | null
  played: LastPlayed
  /** The time "last delve" is counted to. */
  now: number
  onTake: (id: string) => void
  onCancel: () => void
}) {
  const [query, setQuery] = useState('')
  const [colours, setColours] = useState<string[]>([])
  const [picked, setPicked] = useState<string | null>(current)
  const [last] = useState(lastGroup)
  const searchId = useId()
  const listId = useId()
  const sections = useMemo(
    () => pickerSections(entries ?? [], groups, { query, colours }, played, last),
    [entries, groups, query, colours, played, last],
  )
  const searching = query.trim() !== '' || colours.length > 0
  // The section of the hero's deck opens first, else the last used group, else the first section.
  const initial = useMemo(() => {
    const holding = sections.find((section) => section.decks.some((deck) => deck.id === current))
    if (holding !== undefined) return new Set([holding.key])
    const first = firstOpen(
      sections.map((section) => section.key),
      groups,
      current,
    )
    return new Set(first === null ? [] : [first])
  }, [sections, groups, current])
  const { isOpen, toggle } = useFolds(initial, searching)
  const pickedEntry = entries?.find((e) => e.id === picked)
  const shown = sections.reduce((sum, s) => sum + s.decks.length, 0)
  const toggleColour = (letter: string) =>
    setColours((was) => (was.includes(letter) ? was.filter((c) => c !== letter) : [...was, letter]))
  const take = (id: string) => {
    if (pickedEntry?.kind === 'own' && groups.length > 0) rememberGroup(groupOf(groups, id))
    onTake(id)
  }
  return (
    <div className="grid min-h-0 gap-4" data-slot="deck-picker">
      <div className="flex flex-wrap items-center gap-2">
        <label htmlFor={searchId} className="sr-only">
          Search decks
        </label>
        <span className="relative min-w-0 flex-1 basis-48">
          <SearchIcon
            aria-hidden="true"
            className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
          />
          <Input
            id={searchId}
            type="search"
            value={query}
            placeholder="Search by deck or commander"
            onChange={(e) => setQuery(e.target.value)}
            className="pl-8"
          />
        </span>
        <div role="group" aria-label="Filter by color" className="flex gap-1">
          {COLOURS.map((letter) => (
            <Button
              key={letter}
              type="button"
              size="icon-sm"
              variant={colours.includes(letter) ? 'default' : 'outline'}
              aria-pressed={colours.includes(letter)}
              aria-label={COLOUR_NAME[letter]}
              onClick={() => toggleColour(letter)}
            >
              <ColourPips colours={letter} />
            </Button>
          ))}
        </div>
      </div>
      <p aria-live="polite" className="sr-only">
        {entries === null ? 'Loading decks' : `${shown} ${shown === 1 ? 'deck' : 'decks'}`}
      </p>
      <div className="grid min-h-0 gap-4 lg:grid-cols-[minmax(0,1fr)_18rem]">
        <div className="grid min-h-0 content-start gap-4">
          {entries === null ? (
            <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
              <Skeleton className="h-48" />
              <Skeleton className="h-48" />
              <Skeleton className="h-48" />
            </div>
          ) : sections.length === 0 ? (
            <p className="rounded-xl border border-dashed p-6 text-center text-sm text-muted-foreground">
              {entries.length === 0
                ? 'No Commander decks yet. Build one under Decks, or pick a precon once they load.'
                : 'No deck matches. Clear the search or a color.'}
            </p>
          ) : (
            sections.map((section) => {
              const open = isOpen(section.key)
              const body = `${listId}-${section.key}`
              return (
                <section
                  key={section.key}
                  data-group={section.key}
                  aria-label={section.title}
                  className="grid gap-2"
                >
                  <h3>
                    <GroupToggle
                      name={section.title}
                      count={section.decks.length}
                      open={open}
                      controls={body}
                      searching={searching}
                      onToggle={() => toggle(section.key)}
                    />
                  </h3>
                  {open ? (
                    <ul id={body} className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
                      {section.decks.map((entry) => (
                        <DeckCard
                          key={entry.id}
                          entry={entry}
                          picked={entry.id === picked}
                          played={played[entry.id]}
                          now={now}
                          onPick={() => setPicked(entry.id)}
                        />
                      ))}
                    </ul>
                  ) : null}
                </section>
              )
            })
          )}
        </div>
        <aside
          aria-label="Picked deck"
          className="order-first lg:sticky lg:top-0 lg:order-none lg:self-start"
        >
          <DeckPreview entry={pickedEntry} />
        </aside>
      </div>
      <div className="sticky bottom-0 flex flex-wrap justify-end gap-2 border-t bg-popover pt-3">
        <Button type="button" variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <Button
          type="button"
          disabled={pickedEntry === undefined || picked === current}
          onClick={() => picked !== null && take(picked)}
          aria-label={`${pickedEntry === undefined ? 'Take a deck' : `Take ${pickedEntry.name}`} for ${heroName}`}
        >
          {pickedEntry === undefined ? 'Take a deck' : `Take ${pickedEntry.name}`}
        </Button>
      </div>
    </div>
  )
}
