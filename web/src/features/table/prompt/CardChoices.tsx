import { useContext, useMemo, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { cn } from '@/lib/utils'
import type { CSSProperties, FocusEvent, KeyboardEvent, ReactNode, Ref } from 'react'
import type { ObjectId, ObjectView } from '@/protocol/generated'
import { rovingTarget } from '../useRoving'
import { CardSearchField } from './CardSearchField'
import { FILTER_FROM, matchesFilter } from './cardsText'
import { ChoiceAnchorContext } from './choiceAnchor'
import { useChoiceOverlay } from './choiceOverlay'
import { PromptContext } from './PromptContext'
import { numberKey } from './choiceText'
import { useOracleTexts } from './oracleWords'
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
  /** Cards closed while the rest stay open (a second card of a name that a search takes once). */
  shut?: ReadonlySet<ObjectId>
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
  /**
   * What the rest of the prompt takes of the sheet, in rem (the sentence, the buttons, a line of players); the grid has
   * the remainder (`--choice-room`), and its cards are sized so a whole row fits it. 10.5 when left out, 14.5 with a search.
   */
  rest?: number
  /** A search of a library: the search box shows from two cards and takes focus when the prompt appears. */
  search?: boolean
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
 * and a search of a library get a search box over name, type line and rules text (`CardSearchField`). The card under
 * the pointer or focus is large at the side (the table's preview), and so is the last one chosen while none is
 * hovered, which is how a tap on a phone shows its card.
 */
export function CardChoices({
  cards,
  chosen,
  closed,
  shut,
  labels,
  objects,
  onPress,
  label,
  keys = false,
  focusFirst,
  lead,
  rest,
  search = false,
}: CardChoicesProps) {
  const total = cards.length
  const host = useChoiceOverlay(total)
  const name = useContext(PromptContext)?.name
  const [filter, setFilter] = useState('')
  const filtering = total > FILTER_FROM || (search && total > 1)
  const views = useMemo(
    () => (filtering ? cards.map((id) => objects.get(id)) : []),
    [filtering, cards, objects],
  )
  const rules = useOracleTexts(views)
  const visible = useMemo(
    () =>
      filtering
        ? cards.filter((id) => {
            const object = objects.get(id)
            return matchesFilter(object, filter, object === undefined ? '' : rules.get(object))
          })
        : cards,
    [filtering, cards, objects, filter, rules],
  )
  const hiddenChosen = chosen.filter((id) => !visible.includes(id)).length
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
  const body = (
    <ChoiceAnchorContext value={root}>
      {filtering ? (
        <CardSearchField
          value={filter}
          onChange={setFilter}
          shown={visible.length}
          total={total}
          hiddenChosen={hiddenChosen}
          onLeave={() => root.current?.querySelector<HTMLElement>('button[data-candidate]')?.focus()}
          focusOnOpen={search}
          className="min-w-48 flex-1 basis-48"
        />
      ) : null}
      <div
        ref={root}
        role="group"
        aria-label={label}
        data-slot="card-choices"
        data-size={size}
        data-overlay={host !== null || undefined}
        onFocus={onFocus}
        onBlur={onBlur}
        // A pointer that leaves a card for the gap or the edge gives the preview back to the chosen card.
        onPointerOut={restore}
        onKeyDown={onKeyDown}
        style={
          host === null
            ? ({
                '--choice-room': `max(6rem, calc(var(--sheet-max, 20rem) - ${rest ?? (filtering ? 14.5 : 10.5)}rem))`,
              } as CSSProperties)
            : undefined
        }
        className="flex max-w-full min-w-0 basis-full flex-col gap-2"
      >
        {lead}
        {/*
          In the sheet, which is at most `--sheet-max` high, what the stack, the sentence, the buttons and the clock
          leave is the row's (`--choice-room`): its cards stand whole in one row as high as that room, and more cards
          than the width holds scroll sideways over a thin bar, never a row cut in half. In the overlay they wrap.
        */}
        <div
          data-slot="card-row"
          className={cn(
            'flex max-w-full items-start justify-center-safe gap-2 p-1.5',
            host === null
              ? 'overflow-x-auto overflow-y-hidden [scrollbar-width:thin]'
              : 'flex-wrap content-start',
          )}
        >
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
                closed={(closed || shut?.has(id) === true) && !chosen.includes(id)}
                tabbable={id === current}
                hotkey={key}
                onPress={onPress}
              />
            )
          })}
          {visible.length === 0 && total > 0 ? (
            <p data-slot="no-match" className="p-2 text-sm text-muted-foreground">
              {`No card has “${filter.trim()}” in its name, type or text.`}
            </p>
          ) : null}
        </div>
      </div>
    </ChoiceAnchorContext>
  )
  if (host === null) return body
  // The overlay is the prompt's too: it carries the decision, so what finds a prompt's cards finds them here.
  return createPortal(
    <div
      data-slot="choice-overlay"
      data-decision={name}
      className="plaque fade-scroll flex h-full flex-col items-center justify-center-safe gap-2 overflow-y-auto rounded-xl border-2 border-brass p-3 shadow-e3"
    >
      {body}
    </div>,
    host,
  )
}
