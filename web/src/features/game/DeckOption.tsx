import { CheckIcon, LayersIcon, LoaderCircleIcon } from 'lucide-react'
import type { KeyboardEvent } from 'react'
import { IdentityPips } from '@/features/builder/IdentityPips'
import { CardPicture } from '@/features/cards/CardPicture'
import { DeckStateBadge } from '@/features/decks/DeckStateBadge'
import { cn } from '@/lib/utils'
import { entryFacts, entrySupport } from './deckEntries'
import type { DeckEntry } from './deckEntries'

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
  /** Commanders the list had no picture of, from a view that was read. */
  fallback: DeckEntry['commanders']
  onSelect(entry: DeckEntry): void
  onChoose(entry: DeckEntry): void
  /** An arrow, Home or End key: the next row to select. */
  onMove(key: string): void
}

/** The pictures of a row's commanders, one or two overlapped; a tile with the set or an icon where there is none. */
function Pictures({ entry, commanders }: { entry: DeckEntry; commanders: DeckEntry['commanders'] }) {
  if (commanders.length === 0) {
    return (
      <span
        aria-hidden="true"
        data-slot="deck-tile"
        className="grid aspect-[5/7] w-11 shrink-0 place-items-center rounded-[4.75%/3.393%] border bg-muted text-center text-label font-semibold tracking-wide text-muted-foreground"
      >
        {entry.set === null ? <LayersIcon className="size-5" /> : entry.set.toUpperCase()}
      </span>
    )
  }
  return (
    <span aria-hidden="true" data-slot="deck-pictures" className="flex shrink-0 -space-x-5">
      {commanders.slice(0, 2).map((card) => (
        <CardPicture
          key={card.oracleId}
          oracleId={card.oracleId}
          name={card.name}
          size="small"
          alt=""
          quiet
          className="w-11 shadow-e1"
        />
      ))}
    </span>
  )
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
  onSelect,
  onChoose,
  onMove,
}: DeckOptionProps) {
  const support = entrySupport(entry)
  const commanders = entry.commanders.length > 0 ? entry.commanders : fallback
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
      <Pictures entry={entry} commanders={commanders} />
      <span className="grid min-w-0 flex-1 gap-1">
        <span className="line-clamp-2 text-sm leading-tight font-medium [overflow-wrap:anywhere]">
          {entry.name}
        </span>
        <span className="truncate text-xs text-muted-foreground">
          {entryFacts(entry)}
          {entry.commanderNames.length === 0 ? '' : `, ${entry.commanderNames.join(' and ')}`}
        </span>
        <span className="flex flex-wrap items-center gap-x-2 gap-y-1">
          {entry.identity === null ? null : <IdentityPips identity={entry.identity} />}
          <DeckStateBadge state={support.state} label={support.label} />
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
