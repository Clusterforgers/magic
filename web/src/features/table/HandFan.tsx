import {
  closestCenter,
  DndContext,
  DragOverlay,
  MouseSensor,
  TouchSensor,
  useDraggable,
  useDroppable,
  useSensor,
  useSensors,
} from '@dnd-kit/core'
import type { Announcements, DragEndEvent, DragOverEvent, DragStartEvent } from '@dnd-kit/core'
import { Crown } from 'lucide-react'
import { useLayoutEffect, useRef, useState } from 'react'
import type { CSSProperties, KeyboardEvent, ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { Card } from './Card'
import { CardFace } from './CardFace'
import type { CardSize } from './cardSize'
import { HAND_HEAD_PX } from './handFit'
import type { HandLayout } from './handFit'
import { ManaPips } from './ManaPips'
import { CardWidthContext } from './ImageFace'
import { objectIdOf, rovingTarget, useRoving } from './useRoving'

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
  /** Cards that wait in the command zone, by id: the tax their cast costs more and the words that say so. */
  commanders?: ReadonlyMap<ObjectId, HandCommander>
  /** Lets the viewer reorder the cards by drag and drop and by Alt with an arrow key, Home or End. */
  onMove?: ((from: number, to: number) => void) | undefined
}

/** A commander at the head of the hand: the tax on its cast and the words its spoken name gets. */
export interface HandCommander {
  tax: number
  words: string
}

/** How far a mouse moves with the button down before a drag starts; a click moves less. */
const MOUSE_DRAG_PX = 6
/** How long a finger rests on a card before it drags, and how far it may stray meanwhile; a swipe scrolls instead. */
const TOUCH_HOLD_MS = 250
const TOUCH_STRAY_PX = 8

// The dock announces a move itself, by the card's name; dnd-kit's own words name only the id.
const SILENT: Announcements = {
  onDragStart: () => undefined,
  onDragMove: () => undefined,
  onDragOver: () => undefined,
  onDragEnd: () => undefined,
  onDragCancel: () => undefined,
}
const INSTRUCTIONS = {
  draggable: 'Press Alt with the left or right arrow to move the focused card in your hand.',
}

/**
 * The cards of the hand: one row while they fit, else two or three, each row lying over the one before it so that its
 * top (name and cost) shows. A hovered or keyboard-focused card comes to the front and a card of a row another row
 * lies over rises by its lift (a fraction of its height, `HAND_LIFT`, at least what shows it whole). The list has one
 * tab stop and the arrow keys, Home and End move along it in reading order. It is as tall as the rows plus the lift and
 * stands on the bottom of the dock: the part above the cards takes no pointer, so the table under it stays clickable,
 * and a line that scrolls keeps the lifted card inside its own box. With `onMove` the viewer drags a card to another
 * place (a mouse after a few pixels, a finger after a short hold, so a click still plays it and a swipe still scrolls)
 * or moves the focused card with Alt and an arrow key.
 */
export function HandFan({ ids, label, size, cost = false, layout, commanders, onMove }: HandFanProps) {
  const roving = useRoving(ids)
  const list = useRef<HTMLUListElement>(null)
  const refocus = useRef<ObjectId | null>(null)
  const [dragging, setDragging] = useState<ObjectId | null>(null)
  const [over, setOver] = useState<ObjectId | null>(null)
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
  const end = () => {
    setDragging(null)
    setOver(null)
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
        'pointer-events-none flex max-w-full items-end overflow-x-auto overflow-y-hidden px-1 pb-1 [scrollbar-width:thin]',
        layout === undefined ? 'gap-1.5 pt-1' : 'gap-x-(--card-gap) pt-(--hand-lift)',
        rows > 1 && 'flex-wrap content-start justify-center',
        layout?.scrolls === true && 'pb-3',
      )}
    >
      {ids.map((id, index) => {
        const commander = commanders?.get(id)
        const from = dragging === null ? -1 : ids.indexOf(dragging)
        return (
          <HandSlot
            key={id}
            id={id}
            sortable={onMove !== undefined}
            // A row another row lies over keeps only its top as its slot, so the slots of the row below start right under it.
            back={rows > 1 && Math.floor(index / perRow) < rows - 1}
            lifts={rows === 1 || Math.floor(index / perRow) < rows - 1}
            commander={commander !== undefined}
            drop={
              over === id && from >= 0 && from !== index ? (from < index ? 'after' : 'before') : undefined
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
            {commander === undefined ? null : <TaxPill tax={commander.tax} />}
          </HandSlot>
        )
      })}
    </ul>
  )
  return (
    <CardWidthContext.Provider value={layout?.box.width ?? null}>
      {onMove === undefined ? (
        fan
      ) : (
        <DndContext
          sensors={sensors}
          collisionDetection={closestCenter}
          accessibility={{ announcements: SILENT, screenReaderInstructions: INSTRUCTIONS }}
          onDragStart={(event: DragStartEvent) => setDragging(Number(event.active.id))}
          onDragOver={(event: DragOverEvent) => setOver(event.over === null ? null : Number(event.over.id))}
          onDragCancel={end}
          onDragEnd={(event: DragEndEvent) => {
            end()
            if (event.over === null) return
            const from = ids.indexOf(Number(event.active.id))
            const to = ids.indexOf(Number(event.over.id))
            if (from >= 0 && to >= 0 && from !== to) onMove(from, to)
          }}
        >
          {fan}
          {createPortal(
            <DragOverlay dropAnimation={null} zIndex={60}>
              {dragging === null ? null : (
                <DragFace id={dragging} size={size} cost={cost} fit={layout !== undefined} style={style} />
              )}
            </DragOverlay>,
            document.body,
          )}
        </DndContext>
      )}
    </CardWidthContext.Provider>
  )
}

/**
 * One place of the hand: the card on its backing, which rises on hover and focus. A sortable slot is a drag source
 * and a drop target; while a card is dragged over it a bar marks the side the card goes to.
 */
function HandSlot({
  id,
  sortable,
  back,
  lifts,
  commander,
  drop,
  children,
}: {
  id: ObjectId
  sortable: boolean
  back: boolean
  lifts: boolean
  commander: boolean
  drop: 'before' | 'after' | undefined
  children: ReactNode
}) {
  const drag = useDraggable({ id, disabled: !sortable })
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
      data-drag={drag.isDragging || undefined}
      data-drop={drop}
      style={back ? { height: HAND_HEAD_PX } : undefined}
      {...(sortable ? drag.listeners : null)}
      // The slot stays put under the pointer while its card rises, so the hover never lets go of it.
      className={cn(
        'group/slot pointer-events-auto relative flex items-start hover:z-10 focus-within:z-10',
        // A held finger drags the card, so the phone's text selection and callout stay away.
        sortable && 'select-none [-webkit-touch-callout:none]',
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
        className={cn(
          'relative rounded-md bg-table motion-safe:transition-transform motion-safe:duration-fast motion-safe:ease-out',
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

/** The card under the pointer while it is dragged: its face only, so no second button with its id is on the page. */
function DragFace({
  id,
  size,
  cost,
  fit,
  style,
}: {
  id: ObjectId
  size: CardSize
  cost: boolean
  fit: boolean
  style: CSSProperties | undefined
}) {
  const object = usePlay((state) => state.objectsById[id])
  if (object === undefined) return null
  return (
    <div
      aria-hidden="true"
      data-slot="hand-drag-face"
      style={style}
      className="pointer-events-none rotate-2 rounded-md bg-table shadow-lg"
    >
      <CardFace object={object} role={null} size={size} cost={cost} fit={fit} />
    </div>
  )
}

/**
 * The mark on a commander at the head of the hand: a crown, and the tax its cast costs more ("+{2}") while there is one.
 * It sits above the card's foot, clear of the power and toughness box.
 */
function TaxPill({ tax }: { tax: number }) {
  return (
    <span
      aria-hidden="true"
      data-slot="hand-commander-tax"
      className="pointer-events-none absolute bottom-6 left-1/2 flex -translate-x-1/2 items-center gap-0.5 rounded-full bg-background/90 px-1 py-px text-xs leading-4 font-semibold whitespace-nowrap text-foreground shadow-sm ring-1 ring-foreground/20"
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
