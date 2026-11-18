import { useContext, useMemo, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { CSSProperties, FocusEvent, KeyboardEvent, ReactNode, Ref } from 'react'
import type { ObjectId, ObjectView, PlayerId } from '@/protocol/generated'
import { useSeatNamer } from '../names'
import { SEAT_LIFE, SeatDot, seatHueStyle } from '../PodParts'
import { useEdgeFade } from '../useEdgeFade'
import { rovingTarget } from '../useRoving'
import { CardSearchField } from './CardSearchField'
import { FILTER_FROM, matchesFilter } from './cardsText'
import { BAND_HEAD_REM, SEARCH_ROW_REM, stripCardWidth } from './cardPrompts'
import { ChoiceAnchorContext } from './choiceAnchor'
import { useChoiceOverlay, useOverlayRoom } from './choiceOverlay'
import { groupByController, overlayLayout, SCROLLBAR_PX } from './overlayLayout'
import type { CardGroup } from './overlayLayout'
import { PromptContext } from './PromptContext'
import { numberKey } from './choiceText'
import { useOracleTexts } from './oracleWords'
import { tileSizeFor } from './PromptCard'
import { PromptCard } from './PromptCard'
import type { PromptCardSize } from './PromptCard'
import { useRestingPreview } from './restingPreview'
import { usePromptKeys } from './usePromptKeys'

/** What the search box and the players' row take of the panel, in px, for the layout to leave room for. */
const SEARCH_PX = 44
const LEAD_PX = 52

/** How many of the whole list's cards `seat` controls; null where the list is not grouped. */
function wholeSize(whole: readonly CardGroup[], seat: PlayerId | null): number | undefined {
  return whole.find((group) => group.seat === seat)?.cards.length
}

/** A seat's name in its colour over the run of cards it controls. Decoration: each card's label names its controller. */
function GroupLabel({ seat }: { seat: PlayerId }) {
  const seatName = useSeatNamer()
  const me = usePlay((state) => state.seat)
  return (
    <div
      aria-hidden="true"
      data-slot="card-group-label"
      data-seat={seat}
      style={seatHueStyle(seat)}
      className={cn('mb-2 flex items-center gap-1.5 text-xs font-medium', SEAT_LIFE)}
    >
      <SeatDot seat={seat} />
      <span className="truncate">{seat === me ? `${seatName(seat)} (you)` : seatName(seat)}</span>
    </div>
  )
}

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
   * What the prompt draws above the cards inside its choices, in rem (a row of quick buttons, `QUICK_ROW_REM`); the
   * band's first line and a search box are counted here. The cards have what is left of the sheet (`--choice-room`).
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
  rest = 0,
  search = false,
}: CardChoicesProps) {
  const total = cards.length
  const fit = useMemo(() => stripCardWidth(total), [total])
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
  // In the overlay the cards stand in runs by the seat that controls them, so that order is the one keys and Tab follow.
  const room = useOverlayRoom()
  const groups = useMemo(
    () =>
      host === null ? [{ seat: null, cards: [...visible] }] : groupByController(visible, cards, objects),
    [host, visible, cards, objects],
  )
  const shown = useMemo(() => groups.flatMap((group) => group.cards), [groups])
  // The panel is laid out for the whole list, so it keeps its size while a search narrows the cards in it.
  const whole = useMemo(
    () => (host === null ? [] : groupByController(cards, cards, objects)),
    [host, cards, objects],
  )
  const layout = useMemo(
    () =>
      host === null
        ? null
        : overlayLayout(
            room,
            whole.map((group) => group.cards.length),
            (filtering ? SEARCH_PX : 0) + (lead === undefined || lead === null ? 0 : LEAD_PX),
          ),
    [host, room, whole, filtering, lead],
  )
  const hiddenChosen = chosen.filter((id) => !visible.includes(id)).length
  // Tab reaches one card of the list (the last one focused, else the first); the arrow keys move along it.
  const [last, setLast] = useState<ObjectId | null>(null)
  const current = last !== null && shown.includes(last) ? last : (shown[0] ?? null)
  const root = useRef<HTMLDivElement>(null)
  const row = useRef<HTMLDivElement>(null)
  useEdgeFade(row)
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
      ? Object.fromEntries(shown.slice(0, 10).map((id, place) => [numberKey(place) ?? '', () => onPress(id)]))
      : {},
    keys,
  )

  // The strip draws a long list compact; the overlay draws whole cards at the width its layout picked.
  const size: PromptCardSize = host === null ? tileSizeFor(total) : 'normal'
  const card = (id: ObjectId, place: number) => {
    const key = keys ? numberKey(place) : undefined
    return (
      <PromptCard
        key={id}
        ref={place === 0 ? focusFirst : undefined}
        id={id}
        label={labels.get(id) ?? 'A card'}
        size={size}
        overlay={host !== null}
        chosen={chosen.includes(id)}
        closed={(closed || shut?.has(id) === true) && !chosen.includes(id)}
        tabbable={id === current}
        hotkey={key}
        onPress={onPress}
      />
    )
  }
  let place = 0
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
          className={host === null ? 'min-w-48 flex-1 basis-48' : 'shrink-0'}
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
                '--choice-room': `max(6rem, calc(var(--sheet-max, 20rem) - ${BAND_HEAD_REM + rest + (filtering ? SEARCH_ROW_REM : 0)}rem))`,
              } as CSSProperties)
            : undefined
        }
        className={
          host === null
            ? // In the strip the players stand in columns before the cards, as high as their line (`promptBand.css`).
              'flex max-w-full min-w-0 basis-full items-center gap-3'
            : 'flex max-h-full min-h-0 min-w-0 flex-col gap-2 [&>[data-slot=player-targets]]:border-b [&>[data-slot=player-targets]]:pb-2'
        }
      >
        {lead}
        {/*
          In the sheet, which is at most `--sheet-max` high, what the sentence, the buttons and the clock leave is the
          cards' (`--choice-room`): they stand whole in one row, from nine cards in two where that makes them larger,
          as large as that room and the row's width allow (`--choice-fit`), so no card is cut or scrolled away. In the
          overlay the cards stand in a grid of the columns and width the layout picked, a run per seat where several
          control cards, and the grid alone scrolls when it must, under the search box and the players.
        */}
        <div
          ref={row}
          data-slot="card-row"
          data-columns={layout?.columns}
          data-card-width={layout?.card}
          data-scrolls={layout?.scrolls || undefined}
          style={
            layout === null
              ? ({
                  '--choice-fit': fit,
                } as CSSProperties)
              : ({
                  '--overlay-card-w': `${layout.card}px`,
                  width: layout.width + 12 + SCROLLBAR_PX,
                } as CSSProperties)
          }
          className={cn(
            'gap-2 p-1.5',
            layout === null
              ? 'flex max-w-full min-w-0 flex-1 flex-wrap items-start justify-start overflow-x-auto overflow-y-hidden [container-type:inline-size] [scrollbar-width:thin]'
              : 'fade-scroll flex min-h-0 max-w-full flex-wrap content-start justify-center gap-x-4 gap-y-4',
          )}
        >
          {groups.map((group) => {
            const cards = group.cards.map((id) => card(id, place++))
            if (layout === null) return cards
            // A block keeps the columns the whole list gave it, so a search that thins it does not move the rest.
            const columns = Math.min(layout.columns, wholeSize(whole, group.seat) ?? group.cards.length)
            return (
              <div key={group.seat ?? 'all'} data-slot="card-group" data-seat={group.seat ?? undefined}>
                {group.seat === null ? null : <GroupLabel seat={group.seat} />}
                <div
                  className="grid gap-2"
                  style={{ gridTemplateColumns: `repeat(${columns}, ${layout.card}px)` }}
                >
                  {cards}
                </div>
              </div>
            )
          })}
          {visible.length === 0 && total > 0 ? (
            <p data-slot="no-match" className="col-span-full p-2 text-sm text-muted-foreground">
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
      className={cn(
        'plaque flex min-h-0 w-fit min-w-[min(100%,22rem)] flex-col gap-2 rounded-xl border-2 border-brass p-3 shadow-e3',
        room.phone ? 'max-h-full max-w-full' : 'max-h-[80%] max-w-[80%]',
      )}
    >
      {body}
    </div>,
    host,
  )
}
