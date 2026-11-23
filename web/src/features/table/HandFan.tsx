import { anchor } from '@/features/learn/anchors'
import {
  closestCenter,
  pointerWithin,
  DndContext,
  DragOverlay,
  MouseSensor,
  TouchSensor,
  useDraggable,
  useDroppable,
  useSensor,
  useSensors,
} from '@dnd-kit/core'
import type {
  Announcements,
  CollisionDetection,
  DragEndEvent,
  DragMoveEvent,
  DragOverEvent,
  DragStartEvent,
  DropAnimation,
} from '@dnd-kit/core'
import { Backpack, Crown, Library } from 'lucide-react'
import { useLayoutEffect, useRef, useState } from 'react'
import type { CSSProperties, KeyboardEvent, ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { usePlay } from '@/features/game/gameContext'
import { prefersReducedMotion } from '@/hooks/useReducedMotion'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { Card } from './Card'
import { CardFace } from './CardFace'
import type { CardSize } from './cardSize'
import { dragOutcome, dragZone, dropCard, eventY, playableBy } from './handDrag'
import type { DragZone } from './handDrag'
import { HAND_HEAD_PX } from './handFit'
import type { HandLayout } from './handFit'
import { leavingCard } from './handLeaving'
import { ManaPips } from './ManaPips'
import { CardWidthContext } from './ImageFace'
import { holdPreview, previewHeld, usePreviewStore } from './preview'
import { useMarksStore } from './prompt/marks'
import { objectIdOf, rovingTarget, useRoving } from './useRoving'
import { useWheelSideways } from './useWheelSideways'

interface HandFanProps {
  /** The cards of the hand, in order. */
  ids: readonly ObjectId[]
  /** Names the list for a screen reader. */
  label: string
  /** Which parts of a face are drawn; the box too, unless `layout` fits it. */
  size: CardSize
  /** The face draws the mana cost even when `size` is compact. */
  cost?: boolean
  /** The box, gap, rows and lift from the dock's room; without it the cards take their usual size, side by side. */
  layout?: HandLayout | undefined
  /** Cards that stand at the head of the hand from another zone, by id: a commander in the command zone (the tax its cast
   * costs more) or the top card of the library you may see, with the words that say so. */
  commanders?: ReadonlyMap<ObjectId, HandCommander>
  /** Lets the viewer reorder the cards by drag and drop and by Alt with an arrow key, Home or End. */
  onMove?: ((from: number, to: number) => void) | undefined
  /** Lets the viewer play a card by dragging it up out of the dock onto the table, as a click on it would. */
  plays?: boolean
  /** A line that scrolls takes the pointer, so its scrollbar drags: while a decision picks among the hand, where the table
   * under the room above the cards is not what is clicked. */
  grabs?: boolean
}

/** A card at the head of the hand: the tax on its cast, the words its spoken name gets and, for the top card of the
 * library, where it is. */
export interface HandCommander {
  tax: number
  words: string
  /** The top card of your library (Sigarda, Future Sight), not a commander. */
  top?: boolean
  /** A card of your Satchel (Delves), not a commander. */
  satchel?: boolean
}

/** How far a mouse moves with the button down before a drag starts; a click moves less. */
const MOUSE_DRAG_PX = 6
/** How long a finger rests on a card before it drags, and how far it may stray meanwhile; a swipe scrolls instead. */
const TOUCH_HOLD_MS = 250
const TOUCH_STRAY_PX = 8

/** A card let go where it cannot be played glides back to its place in the hand: 200 ms, eased out. */
const SNAP_BACK: DropAnimation = { duration: 200, easing: 'cubic-bezier(0.2, 0, 0, 1)' }

/** A dragged card and where it is. */
interface Drag {
  id: ObjectId
  zone: DragZone
  /** A drop on the table plays it. */
  playable: boolean
  /** Why it cannot be played, in the words on its foot. */
  hint: string | undefined
  /** The viewport y the pointer went down at, and of the dock's top edge. */
  startY: number
  dockTop: number
  /** The viewer's board on screen, which lights as the place to drop. */
  target: DOMRect | null
}

/** The box of the viewer's own battlefield, else of the table above the dock; null when neither is drawn. */
function boardBox(seat: number | null, dock: Element | null): DOMRect | null {
  const own = seat === null ? null : document.querySelector(`[data-battlefield-seat="${seat}"]`)
  const box = own?.getBoundingClientRect()
  if (box !== undefined && box.width > 0 && box.height > 0) return box
  const main = dock?.closest('[data-slot="table-main"]')?.getBoundingClientRect()
  const top = dock?.getBoundingClientRect().top
  if (main === undefined || top === undefined || top <= main.top) return null
  return new DOMRect(main.left, main.top, main.width, top - main.top)
}

/** The card under the pointer or finger is where the dragged card goes; between cards, the nearest one. */
const underPointer: CollisionDetection = (args) => {
  const hits = pointerWithin(args)
  return hits.length > 0 ? hits : closestCenter(args)
}

// The dock announces a move itself, by the card's name; dnd-kit's own words name only the id. Its live region and
// instructions go to an element outside the page, so the table keeps one status region and no second voice.
const SILENT: Announcements = {
  onDragStart: () => undefined,
  onDragMove: () => undefined,
  onDragOver: () => undefined,
  onDragEnd: () => undefined,
  onDragCancel: () => undefined,
}
const INSTRUCTIONS = { draggable: '' }
let offPage: HTMLElement | undefined
function offPageElement(): HTMLElement {
  offPage ??= document.createElement('div')
  return offPage
}

/**
 * The cards of the hand: one row while they fit, else two or three, each row lying over the one before it so that its
 * top (name and cost) shows. A hovered or keyboard-focused card comes to the front and a card of a row another row
 * lies over rises by its lift (a fraction of its height, `HAND_LIFT`, at least what shows it whole). The list has one
 * tab stop and the arrow keys, Home and End move along it in reading order. It is as tall as the rows plus the lift and
 * stands on the bottom of the dock: the part above the cards takes no pointer, so the table under it stays clickable,
 * and a line that scrolls keeps the lifted card inside its own box. With `onMove` the viewer drags a card to another
 * place (a mouse after a few pixels, a finger after a short hold, so a click still plays it and a swipe still scrolls)
 * or moves the focused card with Alt and an arrow key. With `plays` a card carried up out of the dock onto the table
 * (`handDrag.ts`) is played on the drop, as a click plays it; the viewer's board lights as the place to drop, the card
 * says whether it can be played, and one that cannot glides back.
 */
export function HandFan({
  ids,
  label,
  size,
  cost = false,
  layout,
  commanders,
  onMove,
  plays = false,
  grabs = false,
}: HandFanProps) {
  const roving = useRoving(ids)
  const leaving = usePlay(leavingCard)
  const list = useRef<HTMLUListElement>(null)
  // A line that scrolls: a wheel moves it sideways, and while `grabs` its own box takes the pointer so its scrollbar drags.
  useWheelSideways(list)
  const refocus = useRef<ObjectId | null>(null)
  const marks = useMarksStore()
  const preview = usePreviewStore()
  const seat = usePlay((state) => state.seat)
  const [drag, setDrag] = useState<Drag | null>(null)
  const [over, setOver] = useState<ObjectId | null>(null)
  // A card let go where it does nothing glides home; one that plays or takes a new place is there at once.
  const [snaps, setSnaps] = useState(false)
  const heldPreview = useRef(false)
  const dragging = drag?.id ?? null
  const onBoard = drag?.zone === 'board'
  const sortable = onMove !== undefined
  const sensors = useSensors(
    useSensor(MouseSensor, { activationConstraint: { distance: MOUSE_DRAG_PX } }),
    useSensor(TouchSensor, { activationConstraint: { delay: TOUCH_HOLD_MS, tolerance: TOUCH_STRAY_PX } }),
  )
  // React moves a reordered card's node, which drops its focus; the card the keys moved takes it back.
  useLayoutEffect(() => {
    const id = refocus.current
    if (id === null) return
    refocus.current = null
    list.current?.querySelector<HTMLElement>(`button[data-object-id="${id}"]`)?.focus()
  }, [ids])
  const rows = layout?.rows ?? 1
  const perRow = layout?.perRow ?? ids.length
  const style =
    layout === undefined
      ? undefined
      : ({
          '--card-w': `${layout.box.width}px`,
          '--card-h': `${layout.box.height}px`,
          '--card-gap': `${layout.gap}px`,
          '--hand-lift': `${rows > 1 ? Math.max(layout.lift, layout.overlap) : layout.lift}px`,
          ...(rows > 1 ? { width: `${perRow * layout.box.width + (perRow - 1) * layout.gap + 8}px` } : null),
        } as CSSProperties)
  const onKeyDown = (event: KeyboardEvent<HTMLUListElement>) => {
    if (onMove !== undefined && event.altKey && !event.ctrlKey && !event.metaKey) {
      const id = objectIdOf(event.target as Element)
      const from = id === null ? -1 : ids.indexOf(id)
      const to = from < 0 ? null : rovingTarget(event.key, from, ids.length)
      if (id === null || to === null) return
      event.preventDefault()
      if (to === from) return
      refocus.current = id
      onMove(from, to)
      return
    }
    roving.onKeyDown(event)
  }
  const start = (event: DragStartEvent) => {
    const id = Number(event.active.id)
    const dock = list.current?.closest('[data-region="hand"]') ?? list.current
    const playable = plays && playableBy(marks, id)
    // The dragged card is in hand: its preview closes, and the cards it passes over open none.
    preview.hide(id)
    heldPreview.current = !previewHeld() && preview.get() === null
    if (heldPreview.current) holdPreview(true)
    setSnaps(false)
    setDrag({
      id,
      zone: 'hand',
      playable,
      hint: marks.noteOf(id)?.hint,
      startY: eventY(event.activatorEvent) ?? Number.POSITIVE_INFINITY,
      dockTop: dock?.getBoundingClientRect().top ?? 0,
      target: playable ? boardBox(seat, dock ?? null) : null,
    })
  }
  /** The zone of the pointer after it moved by `dy` since it went down. */
  const zoneAt = (dy: number): DragZone =>
    drag === null || !plays ? 'hand' : dragZone(drag.startY, drag.startY + dy, drag.dockTop)
  const move = (event: DragMoveEvent) => {
    const zone = zoneAt(event.delta.y)
    if (drag !== null && zone !== drag.zone) setDrag({ ...drag, zone })
  }
  const end = (snap: boolean) => {
    // The glide is the Web Animations API's; without it, and under reduced motion, the card is back at once.
    setSnaps(snap && !prefersReducedMotion() && typeof Element.prototype.animate === 'function')
    setDrag(null)
    setOver(null)
    if (heldPreview.current) holdPreview(false)
    heldPreview.current = false
  }
  const drop = (event: DragEndEvent) => {
    const id = Number(event.active.id)
    const outcome = dragOutcome({
      zone: zoneAt(event.delta.y),
      playable: plays && playableBy(marks, id),
      sortable,
      from: ids.indexOf(id),
      to: event.over === null ? -1 : ids.indexOf(Number(event.over.id)),
    })
    end(outcome.kind === 'cancel')
    if (outcome.kind === 'reorder') onMove?.(outcome.from, outcome.to)
    else if (outcome.kind === 'play')
      dropCard(
        marks,
        id,
        list.current?.querySelector<HTMLElement>(`button[data-object-id="${id}"]`) ?? undefined,
      )
  }
  const fan = (
    <ul
      ref={list}
      role="list"
      aria-label={label}
      data-slot="hand-fan"
      data-scrolls={layout?.scrolls || undefined}
      data-rows={rows}
      data-dragging={dragging !== null || undefined}
      onFocus={roving.onFocus}
      onKeyDown={onKeyDown}
      aria-keyshortcuts={onMove === undefined ? undefined : 'Alt+ArrowLeft Alt+ArrowRight'}
      style={style}
      className={cn(
        'flex max-w-full items-end overflow-x-auto overflow-y-hidden px-1 pb-1 [scrollbar-width:thin]',
        layout === undefined ? 'gap-1.5 pt-1' : 'gap-x-(--card-gap) pt-(--hand-lift)',
        rows > 1 && 'flex-wrap content-start justify-center',
        // The empty room above the cards lets presses through to the table; a scrollbar takes them only from a box
        // that takes the pointer, so a line that scrolls does when `grabs` (it would cover the table under it otherwise).
        layout?.scrolls === true && 'pb-3',
        layout?.scrolls === true && grabs ? 'pointer-events-auto' : 'pointer-events-none',
      )}
    >
      {ids.map((id, index) => {
        const commander = commanders?.get(id)
        const from = dragging === null ? -1 : ids.indexOf(dragging)
        return (
          <HandSlot
            key={id}
            id={id}
            draggable={sortable || plays}
            sortable={sortable}
            // A row another row lies over keeps only its top as its slot, so the slots of the row below start right under it.
            back={rows > 1 && Math.floor(index / perRow) < rows - 1}
            lifts={rows === 1 || Math.floor(index / perRow) < rows - 1}
            fan={rows === 1 ? fanPlace(index, ids.length, layout?.box) : undefined}
            commander={commander !== undefined && commander.top !== true && commander.satchel !== true}
            top={commander?.top === true}
            satchel={commander?.satchel === true}
            leaving={leaving === id}
            drop={
              !onBoard && over === id && from >= 0 && from !== index
                ? from < index
                  ? 'after'
                  : 'before'
                : undefined
            }
          >
            <Card
              id={id}
              size={size}
              cost={cost}
              fit={layout !== undefined}
              tabbable={id === roving.current}
              words={commander?.words}
            />
            {commander === undefined ? null : commander.top === true ? (
              <TopPill />
            ) : commander.satchel === true ? (
              <SatchelPill />
            ) : (
              <TaxPill tax={commander.tax} />
            )}
          </HandSlot>
        )
      })}
    </ul>
  )
  return (
    <CardWidthContext.Provider value={layout?.box.width ?? null}>
      {!sortable && !plays ? (
        fan
      ) : (
        <DndContext
          sensors={sensors}
          collisionDetection={underPointer}
          accessibility={{
            announcements: SILENT,
            screenReaderInstructions: INSTRUCTIONS,
            container: offPageElement(),
          }}
          onDragStart={start}
          onDragMove={move}
          onDragOver={(event: DragOverEvent) => setOver(event.over === null ? null : Number(event.over.id))}
          onDragCancel={() => end(true)}
          onDragEnd={drop}
        >
          {fan}
          {createPortal(
            <>
              {drag === null || drag.target === null ? null : <PlayTarget box={drag.target} over={onBoard} />}
              <DragOverlay dropAnimation={snaps ? SNAP_BACK : null} zIndex={60}>
                {drag === null ? null : (
                  <DragFace
                    id={drag.id}
                    size={size}
                    cost={cost}
                    fit={layout !== undefined}
                    style={style}
                    play={onBoard ? (drag.playable ? 'can' : 'cannot') : undefined}
                    hint={drag.hint}
                  />
                )}
              </DragOverlay>
            </>,
            document.body,
          )}
        </DndContext>
      )}
    </CardWidthContext.Provider>
  )
}

/** A slot's place in the arc of the hand: its turn in degrees and how far it sits below the middle, in px. */
export interface Fan {
  turn: number
  drop: number
}

/** The most a card at the end of the arc turns, in degrees, and sits lower, in px. */
export const FAN_TURN_MOST = 6
export const FAN_DROP_MOST = 8

/**
 * The place of card `index` of `count` in the arc: the middle card upright and highest, the ends turned outwards and a
 * little lower, a wider hand turning each card less; one card stands straight.
 */
export function fanOf(index: number, count: number): Fan {
  if (count <= 1) return { turn: 0, drop: 0 }
  const middle = (count - 1) / 2
  const share = (index - middle) / middle
  const turn = Math.round(share * Math.min(FAN_TURN_MOST, 1.2 * middle) * 10) / 10
  return { turn, drop: Math.round(share * share * FAN_DROP_MOST * 10) / 10 }
}

/** How far a box turned `turn` degrees about its centre reaches past its upright sides (`x`) and its top and foot (`y`), in px. */
export function turnedReach(turn: number, box: { width: number; height: number }): { x: number; y: number } {
  const angle = (Math.abs(turn) * Math.PI) / 180
  const sin = Math.sin(angle)
  const cos = Math.cos(angle)
  return {
    x: (box.height / 2) * sin + (box.width / 2) * cos - box.width / 2,
    y: (box.width / 2) * sin + (box.height / 2) * cos - box.height / 2,
  }
}

/** A slot's card in the arc: its turn in degrees and its offset across and down from the slot, in px. */
export interface FanPlace {
  turn: number
  x: number
  y: number
}

/** Rounds a distance up to the next tenth of a pixel, so a rounded place never reaches out by a sliver. */
const ceilTenth = (px: number): number => Math.ceil(Math.round(px * 1000) / 100) / 10

/**
 * Where card `index` of `count` lies in a slot of `box`: the arc of `fanOf` raised so the lowest corner of the hand rests
 * on the floor of the slots, and each turned card moved inwards by what its corners reach past its sides, so the arc
 * stays inside the fan at rest (the room above the cards is the lift's). Without a box only the drop is raised.
 */
export function fanPlace(index: number, count: number, box?: { width: number; height: number }): FanPlace {
  const { turn, drop } = fanOf(index, count)
  const end = fanOf(count - 1, count)
  const reach = box === undefined ? 0 : turnedReach(turn, box).x
  const endReach = box === undefined ? 0 : turnedReach(end.turn, box).y
  const rise = ceilTenth(end.drop + endReach)
  return {
    turn,
    x: turn === 0 ? 0 : -Math.sign(turn) * ceilTenth(reach),
    y: Math.round((drop - rise) * 10) / 10,
  }
}

/**
 * One place of the hand: the card on its backing, which rises on hover and focus. A sortable slot is a drag source
 * and a drop target; while a card is dragged over it a bar marks the side the card goes to.
 */
function HandSlot({
  id,
  draggable,
  sortable,
  back,
  lifts,
  commander,
  top,
  satchel,
  leaving,
  drop,
  fan,
  children,
}: {
  id: ObjectId
  /** The card is picked up by a drag: to reorder the hand, to play it on the table, or both. */
  draggable: boolean
  /** The slot takes a dragged card: the hand's own cards, not those at the head of the dock. */
  sortable: boolean
  back: boolean
  lifts: boolean
  /** The card's cast or play went out and the server has not answered yet: it rises out of the hand (`handLeaving`). */
  leaving: boolean
  /** The slot's place in the arc of a one-row hand (`fanPlace`); none in a hand of several rows. */
  fan?: FanPlace | undefined
  commander: boolean
  top: boolean
  satchel: boolean
  drop: 'before' | 'after' | undefined
  children: ReactNode
}) {
  const drag = useDraggable({ id, disabled: !draggable })
  const target = useDroppable({ id, disabled: !sortable })
  return (
    <li
      ref={(node) => {
        drag.setNodeRef(node)
        target.setNodeRef(node)
      }}
      data-slot="hand-slot"
      data-back={back || undefined}
      data-commander={commander || undefined}
      data-library-top={top || undefined}
      data-satchel={satchel || undefined}
      data-leaving={leaving || undefined}
      data-drag={drag.isDragging || undefined}
      data-drop={drop}
      style={back ? { height: HAND_HEAD_PX } : undefined}
      {...(draggable ? drag.listeners : null)}
      // The slot stays put under the pointer while its card rises, so the hover never lets go of it.
      className={cn(
        'group/slot pointer-events-auto relative flex items-start hover:z-10 focus-within:z-10',
        // A held finger drags the card, so the phone's text selection and callout stay away.
        draggable && 'select-none [-webkit-touch-callout:none]',
        drag.isDragging && 'opacity-40',
      )}
    >
      {drop === undefined ? null : (
        <span
          aria-hidden="true"
          data-slot="hand-drop-mark"
          className={cn(
            'pointer-events-none absolute inset-y-0 z-20 w-1 rounded-full bg-primary',
            drop === 'before' ? '-left-1' : '-right-1',
          )}
        />
      )}
      {/* Faces are tinted and see-through; the backing keeps an overlapped neighbour from showing through. */}
      <div
        style={
          fan === undefined
            ? undefined
            : ({
                '--fan-turn': `${fan.turn}deg`,
                '--fan-x': `${fan.x}px`,
                '--fan-y': `${fan.y}px`,
              } as CSSProperties)
        }
        className={cn(
          // A lifted card rises with a little spring and casts a deeper shadow on the felt (docs/ui-design.md, Feel).
          'relative rounded-md bg-table group-hover/slot:shadow-lg group-has-focus-visible/slot:shadow-lg motion-safe:transition-[translate,rotate,box-shadow] motion-safe:duration-base motion-safe:ease-(--ease-spring)',
          // The arc of the hand; a card that rises stands straight.
          fan !== undefined &&
            'translate-x-(--fan-x) translate-y-(--fan-y) rotate-(--fan-turn) group-hover/slot:rotate-0 group-has-focus-visible/slot:rotate-0',
          // The row nothing lies over is whole already; a lift there would only cover the row above.
          lifts &&
            'group-hover/slot:-translate-y-(--hand-lift) group-has-focus-visible/slot:-translate-y-(--hand-lift) pointer-coarse:group-focus-within/slot:-translate-y-(--hand-lift)',
        )}
      >
        {children}
      </div>
    </li>
  )
}

/** What the dragged card says while it is over the table. */
const CAN_PLAY = 'Release to play'
const CANNOT_PLAY = 'Can’t play now'

/**
 * The viewer's board while a playable card is dragged: a dashed edge says where it can go, and once the card is over
 * the table the edge closes and the board glows. It takes no pointer and stands over the page, so nothing moves.
 */
function PlayTarget({ box, over }: { box: DOMRect; over: boolean }) {
  return (
    <div
      aria-hidden="true"
      data-slot="hand-play-target"
      data-over={over || undefined}
      style={{ left: box.left, top: box.top, width: box.width, height: box.height }}
      className={cn(
        'pointer-events-none fixed z-50 rounded-xl outline-2 -outline-offset-2 motion-safe:transition-[background-color,box-shadow,outline-color] motion-safe:duration-fast motion-safe:ease-out',
        over
          ? 'bg-primary/10 shadow-[inset_0_0_32px_color-mix(in_oklch,var(--primary)_45%,transparent)] outline-primary'
          : 'outline-primary/60 outline-dashed',
      )}
    />
  )
}

/**
 * The card under the pointer while it is dragged: its face only, so no second button with its id is on the page. Over
 * the table (`play`) it stands straight and says what a drop does: lit with "Release to play", or dimmed with why not.
 */
function DragFace({
  id,
  size,
  cost,
  fit,
  style,
  play,
  hint,
}: {
  id: ObjectId
  size: CardSize
  cost: boolean
  fit: boolean
  style: CSSProperties | undefined
  play?: 'can' | 'cannot' | undefined
  hint?: string | undefined
}) {
  const object = usePlay((state) => state.objectsById[id])
  if (object === undefined) return null
  return (
    <div
      aria-hidden="true"
      data-slot="hand-drag-face"
      data-play={play}
      style={style}
      className={cn(
        'pointer-events-none relative rounded-md bg-table shadow-lg motion-safe:transition-[rotate,scale,box-shadow] motion-safe:duration-fast motion-safe:ease-out',
        play === undefined && 'rotate-2',
        play === 'can' &&
          'scale-105 shadow-[0_0_16px_color-mix(in_oklch,var(--primary)_60%,transparent)] ring-2 ring-primary',
      )}
    >
      <div className={cn(play === 'cannot' && 'opacity-60 saturate-50')}>
        <CardFace object={object} role={null} size={size} cost={cost} fit={fit} />
      </div>
      {play === undefined ? null : (
        <span
          data-slot="hand-drag-word"
          className="absolute top-full left-1/2 mt-1 -translate-x-1/2 rounded-full bg-background/95 px-2 py-0.5 text-xs leading-4 font-semibold whitespace-nowrap text-foreground shadow-sm ring-1 ring-foreground/20"
        >
          {play === 'can' ? CAN_PLAY : (hint ?? CANNOT_PLAY)}
        </span>
      )}
    </div>
  )
}

/**
 * The mark on a commander at the head of the hand: a crown, and the tax its cast costs more ("+{2}") while there is one.
 * It sits in the middle of the art, clear of the name, cost, type line and power and toughness box.
 */
function TaxPill({ tax }: { tax: number }) {
  return (
    <span
      aria-hidden="true"
      data-slot="hand-commander-tax"
      {...anchor('command.tax')}
      className="pointer-events-none absolute top-1/2 left-1/2 flex -translate-1/2 items-center gap-0.5 rounded-full bg-background/90 px-1 py-px text-xs leading-4 font-semibold whitespace-nowrap text-foreground shadow-sm ring-1 ring-foreground/20"
    >
      <Crown className="size-3 shrink-0 text-primary" />
      {tax === 0 ? null : (
        <>
          +
          <ManaPips
            pips={[
              { key: 'tax', tone: 'generic', text: `${tax}`, symbol: `${tax}`, name: `${tax} generic mana` },
            ]}
            sizeClass="size-4 text-xs"
            label="Tax"
          />
        </>
      )}
    </span>
  )
}

/** The mark on the top card of your library at the head of the hand: the library's icon and "Top". */
/** The mark on a card of your Satchel at the head of the hand: a backpack and the word. */
function SatchelPill() {
  return (
    <span
      aria-hidden="true"
      data-slot="hand-satchel"
      className="pointer-events-none absolute top-1/2 left-1/2 flex -translate-1/2 items-center gap-0.5 rounded-full bg-background/90 px-1 py-px text-xs leading-4 font-semibold whitespace-nowrap text-foreground shadow-sm ring-1 ring-foreground/20"
    >
      <Backpack className="size-3 shrink-0 text-primary" />
      Satchel
    </span>
  )
}

function TopPill() {
  return (
    <span
      aria-hidden="true"
      data-slot="hand-library-top"
      className="pointer-events-none absolute top-1/2 left-1/2 flex -translate-1/2 items-center gap-0.5 rounded-full bg-background/90 px-1 py-px text-xs leading-4 font-semibold whitespace-nowrap text-foreground shadow-sm ring-1 ring-foreground/20"
    >
      <Library className="size-3 shrink-0 text-primary" />
      Top
    </span>
  )
}
