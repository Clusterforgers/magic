import { anchor, coachZone } from '@/features/learn/anchors'
import { cn } from '@/lib/utils'
import { memo, useCallback, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { useGame, usePlay } from '@/features/game/gameContext'
import type { ObjectId, ObjectView, PlayerId, PlayerView } from '@/protocol/generated'
import { castCostLine } from './commander'
import { HandBacks } from './Hand'
import { dockLayouts, HAND_GAP_PX, handDockNeed, settleKey, settleNeed } from './handFit'
import type { SettledNeed } from './handFit'
import type { HandLayout } from './handFit'
import { HAND_GROW_VAR, handSink } from './layout'
import { headLabel, seenTop, TOP_WORDS } from './libraryTop'
import { HandFan } from './HandFan'
import type { HandCommander } from './HandFan'
import { useSeatNamer } from './names'
import { playerOf } from './selectors'
import { useCoarsePointer } from './useCoarsePointer'
import { useHandOrder } from './useHandOrder'
import { useContentBox } from './useZoneFit'
import { useOpeningInOverlay } from './prompt/choiceOverlay'
import { usePickAmong } from './prompt/marks'

/** Room kept round the line, so the focus ring of the first and last card is not cut by the line's scrolling. */
const PAD_PX = 4

/** A card narrower than this draws the compact face (name and cost); from here it draws the type line and the rest too. */
const NORMAL_FACE_PX = 76

const NO_COMMANDERS: ReadonlyMap<ObjectId, HandCommander> = new Map()
const NO_CARDS: readonly ObjectId[] = []

/**
 * The seat's commanders that wait in its command zone, in command zone order, each with its tax and the words of
 * where it is and what casting it costs ("in the command zone, costs {3}{G}{G} + {2} tax").
 */
export function commandersInHand(
  player: PlayerView | undefined,
  objects: Readonly<Record<ObjectId, ObjectView>>,
): ReadonlyMap<ObjectId, HandCommander> {
  const commanders = player?.commanders ?? []
  const command = player?.command.cards ?? []
  if (commanders.length === 0 || command.length === 0) return NO_COMMANDERS
  const waiting = new Map<ObjectId, HandCommander>()
  for (const id of command) {
    const commander = commanders.find((entry) => entry.object === id)
    if (commander === undefined) continue
    const cost = castCostLine(objects[id]?.cost_now ?? objects[id]?.mana_cost ?? null, commander.tax)
    waiting.set(id, {
      tax: commander.tax,
      words: cost === '' ? 'in the command zone' : `in the command zone, costs ${cost}`,
    })
  }
  return waiting.size === 0 ? NO_COMMANDERS : waiting
}

/**
 * The cards at the head of the dock: the commanders waiting in the command zone, then the top card of your library while
 * you may see it (Sigarda, Future Sight, Courser of Kruphix), so it is cast or played from here like a card of the hand.
 */
export function headOfHand(
  player: PlayerView | undefined,
  objects: Readonly<Record<ObjectId, ObjectView>>,
): ReadonlyMap<ObjectId, HandCommander> {
  const commanders = commandersInHand(player, objects)
  const satchel = satchelInHand(player, objects)
  const top = player === undefined ? undefined : seenTop(player)
  const head = satchel.length === 0 ? commanders : new Map([...commanders, ...satchel])
  if (top === undefined || objects[top] === undefined) return head
  return new Map([...head, [top, { tax: 0, words: TOP_WORDS, top: true }]])
}

/** The cards of your Satchel, each with the words of where it is and what it costs ("in your Satchel, costs {1}{G}"). */
function satchelInHand(
  player: PlayerView | undefined,
  objects: Readonly<Record<ObjectId, ObjectView>>,
): [ObjectId, HandCommander][] {
  return (player?.satchel ?? [])
    .filter((id) => objects[id] !== undefined)
    .map((id) => {
      const cost = castCostLine(objects[id]?.cost_now ?? objects[id]?.mana_cost ?? null, 0)
      const words = cost === '' ? SATCHEL_WORDS : `${SATCHEL_WORDS}, costs ${cost}`
      return [id, { tax: 0, words, satchel: true }]
    })
}

/** Where a Satchel card at the head of the dock is, in the words of its spoken name. */
const SATCHEL_WORDS = 'in your Satchel'

/**
 * The hand dock, the bottom edge of the table: the viewer's cards as a fan as tall as the dock allows, side by side
 * while they fit and overlapping just enough when they do not (at least 62 % of a card shows), a hovered or focused card
 * rising by 40 % of its height and opening the preview. The cards are the same buttons as everywhere on the table, so a
 * click plays or casts them as the active prompt says. The fan stays inside the dock's width, so it never reaches the
 * pass controls beside it. The commanders waiting in the command zone stand in a list of their own before the hand, each
 * with a crown and the tax on its cast,
 * so a commander is cast from here like any card of the hand; the top card of your library you may see stands there too. A hand that one row of the narrowest card does not hold
 * grows the dock to the height of its rows, up to `growMost` px (`HAND_GROW_VAR` on the table's main area). With
 * `peek` (a phone) a dock lower than `PEEK_ROOM_PX` keeps its cards at that size and lets them sink under its bottom
 * edge, a peek row of their heads; a hovered, focused or tapped card rises whole above it.
 */
export const HandDock = memo(function HandDock({
  seat,
  growMost = 0,
  peek = false,
}: {
  seat: PlayerId
  growMost?: number
  peek?: boolean
}) {
  const owner = useSeatNamer()(seat)
  const player = usePlay((state) => playerOf(state, seat))
  const objects = usePlay((state) => state.objectsById)
  const cards = player?.hand.cards ?? null
  const { game } = useGame()
  // The viewer's own order: an auto sort or the order they dragged; the server's hand order stays as it is.
  const order = useHandOrder(game, cards ?? NO_CARDS, objects)
  const [moved, setMoved] = useState('')
  const { ids: ordered, move } = order
  const onMove = useCallback(
    (from: number, to: number) => {
      const id = ordered[from]
      move(from, to)
      const name = id === undefined ? 'Card' : (objects[id]?.name ?? 'Card')
      setMoved(`${name} moved to place ${to + 1} of ${ordered.length}.`)
    },
    [ordered, move, objects],
  )
  const waiting = useMemo(() => headOfHand(player, objects), [player, objects])
  const count = player?.hand.count ?? 0
  const zone = useRef<HTMLDivElement>(null)
  const room = useContentBox(zone)
  const coarse = useCoarsePointer()
  const pick = usePickAmong(cards ?? NO_CARDS)
  // The opening hand floating in the centre overlay is the one copy to read: the dock keeps its place but not its cards.
  const floating = useOpeningInOverlay()
  const layouts = useMemo(() => {
    if (room === null || cards === null) return undefined
    const width = room.width - 2 * PAD_PX
    const height = room.height + handSink(room.height, peek) - 2 * PAD_PX
    // The commanders take one card's width each, and a divider, at the head of the dock; the hand fits the rest.
    // A decision that picks among the hand's cards gives the hand its room first, so every card on offer shows.
    return dockLayouts({ width, height, count: cards.length, commanders: waiting.size, coarse, pick })
  }, [room, cards, waiting, coarse, pick, peek])
  const lineWidth = room === null ? 0 : room.width - 2 * PAD_PX
  const fresh =
    room === null || cards === null || coarse || growMost <= 0
      ? 0
      : handDockNeed({
          width: lineWidth,
          count: cards.length,
          commanders: waiting.size,
          most: growMost,
          pick,
        })
  // The dock takes or gives back a row at a settle point (a new step, or a pick among the hand that comes or goes),
  // never on each draw within one.
  const turnStep = usePlay((state) => settleKey(state.view?.turn.number ?? 0, state.view?.turn.step ?? ''))
  const step = pick ? `${turnStep}:pick` : turnStep
  const known = room !== null && cards !== null
  const settled = useRef<SettledNeed | null>(null)
  const kept = useMemo(
    () => (known ? settleNeed(settled.current, fresh, step, lineWidth, growMost) : null),
    [known, fresh, step, lineWidth, growMost],
  )
  useLayoutEffect(() => {
    settled.current = kept
  }, [kept])
  const need = kept?.need ?? 0
  useLayoutEffect(() => {
    const main = zone.current?.closest<HTMLElement>('[data-slot="table-main"]')
    if (main == null || need <= 0) return undefined
    main.style.setProperty(HAND_GROW_VAR, `${need}px`)
    return () => {
      main.style.removeProperty(HAND_GROW_VAR)
    }
  }, [need])
  const sink = room === null ? 0 : handSink(room.height, peek)
  // A card of a peek row rises by its lift and by what the dock's edge hides of it, so a lifted card shows whole.
  const sunk = (fan: HandLayout | undefined): HandLayout | undefined =>
    fan === undefined || sink === 0 ? fan : { ...fan, lift: fan.lift + sink }
  const layout = sunk(layouts?.hand)
  const size = layout !== undefined && layout.box.width < NORMAL_FACE_PX ? 'compact' : 'normal'
  return (
    <section
      aria-label="Your hand"
      data-region="hand"
      {...anchor('hand')}
      {...coachZone('hand')}
      data-keep-clear=""
      data-fx-anchor={`${seat}:hand`}
      data-peek={sink > 0 || undefined}
      className={cn(
        'dock flex min-h-0 min-w-0 items-center justify-center px-2 py-1',
        // A peek row cuts its cards at its bottom edge only: a lifted card still rises over the board.
        sink > 0 && '[clip-path:inset(-100vh_-100vw_0_-100vw)]',
      )}
    >
      <p aria-live="polite" className="sr-only">
        {moved}
      </p>
      <div
        ref={zone}
        data-floating={floating || undefined}
        aria-hidden={floating || undefined}
        style={sink > 0 ? { position: 'relative', top: sink } : undefined}
        className={cn(
          'flex size-full min-h-0 min-w-0 items-end justify-center gap-1',
          floating && 'invisible',
        )}
      >
        {cards === null || waiting.size === 0 ? null : (
          // As high as the dock and no higher: the fan inside stands taller (the room for a lifted card) and the
          // divider is this box's edge, so it must not follow the fan up into the battlefield.
          <div
            data-slot="hand-command"
            {...anchor('command')}
            {...coachZone('command')}
            className="flex h-full min-w-0 shrink-0 items-end border-r border-foreground/15 pr-1"
            // The width counts the gap after the box (`COMMAND_GAP_PX`), which the line draws between the two.
            style={
              layouts?.command?.scrolls === true && layouts.commandWidth !== undefined
                ? { maxWidth: layouts.commandWidth - HAND_GAP_PX }
                : undefined
            }
          >
            <HandFan
              ids={[...waiting.keys()]}
              label={headLabel(owner, waiting)}
              size={size}
              cost
              layout={sunk(layouts?.command)}
              commanders={waiting}
              plays
              grabs={pick}
            />
          </div>
        )}
        {cards === null ? (
          <HandBacks count={count} />
        ) : cards.length === 0 ? (
          <p className="self-center text-sm text-muted-foreground">Your hand is empty.</p>
        ) : (
          <HandFan
            ids={ordered}
            label={`${owner}'s hand`}
            size={size}
            cost
            layout={layout}
            onMove={onMove}
            plays
            grabs={pick}
          />
        )}
      </div>
    </section>
  )
})
