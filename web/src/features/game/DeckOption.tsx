import { CheckIcon, LoaderCircleIcon, StarIcon } from 'lucide-react'
import type { KeyboardEvent } from 'react'
import { IdentityPips } from '@/features/builder/IdentityPips'
import { DeckStateBadge } from '@/features/decks/DeckStateBadge'
import { ToRead } from '@/features/decks/ToRead'
import { cn } from '@/lib/utils'
import { entryFacts, entrySupport, handWords } from './deckEntries'
import type { DeckEntry } from './deckEntries'
import { DeckPictures } from './DeckPictures'

interface DeckOptionProps {
  entry: DeckEntry
  /** The deck whose preview is shown. */
  selected: boolean
  /** The deck this seat plays. */
  chosen: boolean
  /** The choice of this deck is out. */
  pending: boolean
  /** The roving tab stop: the selected row, or the first while none is. */
  tabStop: boolean
  /** Cards the list had no picture of, from a view that was read. */
  fallback: DeckEntry['pictures']
  /** The list is ordered by cards played by hand, so every row says how many it has. */
  showHand: boolean
  onSelect(entry: DeckEntry): void
  onChoose(entry: DeckEntry): void
  /** An arrow, Home or End key: the next row to select. */
  onMove(key: string): void
}

/**
 * One deck of the chooser's list: its commanders' pictures, name, facts, colours and the support summary as a chip.
 * A click selects it for the preview, Enter or a double click takes it, and the arrow keys move the selection.
 */
export function DeckOption({
  entry,
  selected,
  chosen,
  pending,
  tabStop,
  fallback,
  showHand,
  onSelect,
  onChoose,
  onMove,
}: DeckOptionProps) {
  const support = entrySupport(entry)
  const hand = handWords(entry, showHand)
  const press = (event: KeyboardEvent<HTMLLIElement>): void => {
    if (event.key === 'Enter') {
      event.preventDefault()
      onChoose(entry)
    } else if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
      event.preventDefault()
      onMove(event.key)
    }
  }
  return (
    <li
      role="option"
      aria-selected={selected}
      tabIndex={tabStop ? 0 : -1}
      data-deck-option={entry.id}
      data-chosen={chosen || undefined}
      data-state={entry.state}
      onClick={() => onSelect(entry)}
      onDoubleClick={() => onChoose(entry)}
      onKeyDown={press}
      className={cn(
        'flex min-w-0 cursor-pointer items-center gap-3 rounded-lg border bg-card p-2 pr-3 transition-colors duration-fast ease-out hover:bg-muted/60 focus-visible:outline-2 focus-visible:outline-offset-2',
        selected && 'border-primary bg-primary/8 hover:bg-primary/12',
      )}
    >
      <DeckPictures entry={entry} fallback={fallback} />
      <span className="grid min-w-0 flex-1 gap-1">
        <span className="flex min-w-0 items-start gap-1">
          <span className="line-clamp-2 text-sm leading-tight font-medium [overflow-wrap:anywhere]">
            {entry.name}
          </span>
          {entry.favourite ? (
            <StarIcon
              role="img"
              aria-label="Favourite"
              data-slot="deck-favourite"
              className="mt-px size-3.5 shrink-0 fill-current text-warning"
            />
          ) : null}
        </span>
        <span className="truncate text-xs text-muted-foreground">
          {entryFacts(entry)}
          {entry.commanderNames.length === 0 ? '' : `, ${entry.commanderNames.join(' and ')}`}
        </span>
        <span className="flex flex-wrap items-center gap-x-2 gap-y-1">
          {entry.identity === null ? null : <IdentityPips identity={entry.identity} />}
          <DeckStateBadge state={support.state} label={support.label} />
          <ToRead count={entry.toRead} className="text-xs" />
          {hand === null ? null : (
            <span data-slot="deck-by-hand" className="text-xs text-muted-foreground">
              {hand}
            </span>
          )}
        </span>
      </span>
      {pending ? (
        <LoaderCircleIcon aria-label="Choosing this deck" className="size-5 shrink-0 animate-spin" />
      ) : chosen ? (
        <span
          data-slot="deck-chosen"
          className="flex shrink-0 items-center gap-1 text-xs font-medium text-primary"
        >
          <CheckIcon aria-hidden="true" className="size-4" />
          Chosen
        </span>
      ) : null}
    </li>
  )
}
