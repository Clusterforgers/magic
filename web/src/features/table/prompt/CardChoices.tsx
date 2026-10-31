import { useMemo, useRef, useState } from 'react'
import type { FocusEvent, KeyboardEvent, ReactNode, Ref } from 'react'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'
import type { ObjectId, ObjectView } from '@/protocol/generated'
import { rovingTarget } from '../useRoving'
import { FILTER_FROM, matchesFilter } from './cardsText'
import { ChoiceAnchorContext } from './choiceAnchor'
import { numberKey } from './choiceText'
import { tileSizeFor } from './PromptCard'
import { PromptCard } from './PromptCard'
import type { PromptCardSize } from './PromptCard'
import { useRestingPreview } from './restingPreview'
import { usePromptKeys } from './usePromptKeys'

/** Props of `CardChoices`. */
export interface CardChoicesProps {
  /** The cards on offer, each once, in the order they are drawn. */
  cards: readonly ObjectId[]
  /** The cards of the choice so far, in the order chosen; the last one rests large at the side. */
  chosen: readonly ObjectId[]
  /** The choice is full: the cards outside it are closed. */
  closed: boolean
  /** What a screen reader calls each card; two cards of one name are told apart by the caller. */
  labels: ReadonlyMap<ObjectId, string>
  /** The views of the cards on offer, for the filter by name or type line; a card the view lacks matches nothing. */
  objects: ReadonlyMap<ObjectId, ObjectView | undefined>
  /** A press on a card that is not closed. */
  onPress(id: ObjectId): void
  /** The name of the group for a screen reader. */
  label: string
  /** The keys 1 to 9 and 0 choose the first ten cards in view. */
  keys?: boolean
  /** The first card takes focus when the prompt appears. */
  focusFirst?: Ref<HTMLButtonElement>
  /** Controls drawn before the cards in the same group (a player among the targets). */
  lead?: ReactNode
  /** The sheet's room for the grid, as a Tailwind class; the default leaves what the sentence and the buttons take. */
  className?: string
}

/** The card a focused element belongs to, or null. */
function candidateOf(element: EventTarget | null): ObjectId | null {
  if (!(element instanceof Element)) return null
  const raw = element.closest('[data-candidate]')?.getAttribute('data-candidate')
  if (raw === null || raw === undefined || raw.trim() === '') return null
  const id = Number(raw)
  return Number.isInteger(id) ? id : null
}

/**
 * The cards a prompt offers, as pictures in a grid that scrolls inside the sheet: the one place every choice of
 * cards is drawn (the cards of a search, a discard, the targets in a graveyard or in exile). A card is a toggle; Tab
 * reaches one card, the arrow keys, Home and End move along the grid, the first ten have number keys, and a long list
 * gets a filter box. The card under the pointer or focus is large at the side (the table's preview), and so is the
 * last one chosen while none is hovered, which is how a tap on a phone shows its card.
 */
export function CardChoices({
  cards,
  chosen,
  closed,
  labels,
  objects,
  onPress,
  label,
  keys = false,
  focusFirst,
  lead,
  className,
}: CardChoicesProps) {
  const total = cards.length
  const [filter, setFilter] = useState('')
  const filtering = total > FILTER_FROM
  const visible = useMemo(
    () => (filtering ? cards.filter((id) => matchesFilter(objects.get(id), filter)) : cards),
    [filtering, cards, objects, filter],
  )
  // Tab reaches one card of the list (the last one focused, else the first); the arrow keys move along it.
  const [last, setLast] = useState<ObjectId | null>(null)
  const current = last !== null && visible.includes(last) ? last : (visible[0] ?? null)
  const root = useRef<HTMLDivElement>(null)
  const { restore } = useRestingPreview(chosen, root)

  const onFocus = (event: FocusEvent<HTMLElement>): void => {
    const id = candidateOf(event.target)
    if (id !== null) setLast(id)
  }
  const onBlur = (event: FocusEvent<HTMLElement>): void => {
    // Focus that stays inside the grid is a move to another card, which shows itself.
    if (!event.currentTarget.contains(event.relatedTarget)) restore()
  }
  const onKeyDown = (event: KeyboardEvent<HTMLElement>): void => {
    if (event.altKey || event.ctrlKey || event.metaKey) return
    const buttons = [...event.currentTarget.querySelectorAll<HTMLElement>('button[data-candidate]')]
    const index = buttons.findIndex((card) => card === document.activeElement)
    const to = rovingTarget(event.key, Math.max(index, 0), buttons.length)
    if (index < 0 || to === null) return
    event.preventDefault()
    buttons[to]?.focus()
  }
  usePromptKeys(
    keys
      ? Object.fromEntries(
          visible.slice(0, 10).map((id, place) => [numberKey(place) ?? '', () => onPress(id)]),
        )
      : {},
    keys,
  )

  const size: PromptCardSize = tileSizeFor(total)
  return (
    <ChoiceAnchorContext value={root}>
      {filtering ? (
        <Input
          type="search"
          aria-label="Filter cards"
          placeholder="Filter by name or type"
          autoComplete="off"
          value={filter}
          className="h-9 min-w-40 flex-1 basis-40"
          onChange={(event) => setFilter(event.target.value)}
        />
      ) : null}
      <div
        ref={root}
        role="group"
        aria-label={label}
        data-slot="card-choices"
        data-size={size}
        onFocus={onFocus}
        onBlur={onBlur}
        // A pointer that leaves a card for the gap or the edge gives the preview back to the chosen card.
        onPointerOut={restore}
        onKeyDown={onKeyDown}
        className={cn(
          // The sheet the prompt sits in is at most `--sheet-max` high: what the stack, the sentence, the buttons and the
          // clock leave is the grid's, and it scrolls inside.
          'flex basis-full flex-wrap content-start items-start gap-1.5 overflow-y-auto p-1.5',
          className ??
            (filtering
              ? 'max-h-[max(6rem,calc(var(--sheet-max,20rem)_-_14.5rem))]'
              : 'max-h-[max(6rem,calc(var(--sheet-max,20rem)_-_10.5rem))]'),
        )}
      >
        {lead}
        {visible.map((id, place) => {
          const key = keys ? numberKey(place) : undefined
          return (
            <PromptCard
              key={id}
              ref={place === 0 ? focusFirst : undefined}
              id={id}
              label={labels.get(id) ?? 'A card'}
              size={size}
              chosen={chosen.includes(id)}
              closed={closed && !chosen.includes(id)}
              tabbable={id === current}
              hotkey={key}
              onPress={onPress}
            />
          )
        })}
        {visible.length === 0 && total > 0 ? (
          <p data-slot="no-match" className="p-2 text-sm text-muted-foreground">
            No card matches that filter.
          </p>
        ) : null}
      </div>
    </ChoiceAnchorContext>
  )
}
