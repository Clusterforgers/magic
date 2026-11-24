/** Pure sizing of the hand dock: the card width its height allows, the rows a large hand wraps into, and how far a card rises. */
import { CARD_ASPECT, FIT_LIMITS } from './fit'

/** A hovered or focused card rises by this share of its height. */
export const HAND_LIFT = 0.4

/** The narrowest card of a hand in px: its name (up to three lines of 9 letters) and its cost still read. */
export const HAND_FLOOR_PX = 68

/** The space between neighbours of a row in px. */
export const HAND_GAP_PX = 4

/** The most rows a hand wraps into. */
export const HAND_MAX_ROWS = 3

/**
 * The top of a card that always shows, in px: two lines of name and the cost row. A row that another row lies over
 * shows this much, so the name and the cost of every card read without a hover; it is also over the 44 px target of a
 * touch screen.
 */
export const HAND_HEAD_PX = 54

/** The widest card the hand dock of this height (px) holds in one line: its height over the card's shape, between the fit limits and `cap`. */
export function handCardWidth(height: number, cap: number = FIT_LIMITS.maxWidth): number {
  const fits = Number.isFinite(height) ? Math.floor(height / CARD_ASPECT) : FIT_LIMITS.minWidth
  return Math.max(FIT_LIMITS.minWidth, Math.min(cap, fits))
}

/** What the hand dock draws round its cards in px: its top edge, its padding and the line's padding. */
export const HAND_DOCK_CHROME_PX = 17

/** What `handDockNeed` needs. */
export interface DockNeedInput {
  /** The room for the line in px, less the padding kept round it. */
  width: number
  /** The cards of the hand. */
  count: number
  /** The commanders that stand at the head of the dock, one card wide each and a divider after them. */
  commanders: number
  /** The tallest the dock may grow, in px. */
  most: number
  /** A decision picks among the hand's cards: the head keeps one card's width and scrolls (`dockLayouts`). */
  pick?: boolean
}

/**
 * How tall the hand dock must be (px, with its chrome) to hold the hand in two or three rows of `HAND_FLOOR_PX` cards
 * without scrolling: 0 while one row of them fits the width, and 0 when the rows it needs would be taller than `most`
 * (the hand then scrolls in one row). It reads the width only, so a dock that grows to it does not change it.
 */
export function handDockNeed({ width, count, commanders, most, pick = false }: DockNeedInput): number {
  const cardHeight = Math.round(HAND_FLOOR_PX * CARD_ASPECT)
  const shown = pick && count > 0 ? Math.min(1, commanders) : commanders
  const head = shown > 0 ? shown * (HAND_FLOOR_PX + HAND_GAP_PX) + COMMAND_GAP_PX : 0
  const room = width - head
  const cards = Math.max(1, count)
  for (let rows = 1; rows <= HAND_MAX_ROWS && rows <= cards; rows += 1) {
    const perRow = Math.ceil(cards / rows)
    if (perRow * HAND_FLOOR_PX + (perRow - 1) * HAND_GAP_PX > room) continue
    if (rows === 1) return 0
    const need = cardHeight + (rows - 1) * HAND_HEAD_PX + HAND_DOCK_CHROME_PX
    return need <= most ? need : 0
  }
  return 0
}

/** The dock height a hand dock grew to and when: the settle point it was taken at and the room it was taken for. */
export interface SettledNeed {
  need: number
  /** The settle point: a step of a turn (`settleKey`). */
  at: string
  /** The dock's line width and its most, in px: a new window size settles at once. */
  width: number
  most: number
}

/** The settle point of a view: the turn and its step, so the dock changes its rows once a step, not on every draw. */
export const settleKey = (turn: number, step: string): string => `${turn}:${step}`

/**
 * The height the hand dock draws at: `need` taken at the first draw, at a new settle point (a new step) or a new room
 * (a window resized), and otherwise the height taken last, so a draw or a play within a step neither grows the dock
 * by a row nor gives one back; the hand fits the dock it has meanwhile (smaller cards, or one row that scrolls).
 */
export function settleNeed(
  held: SettledNeed | null,
  need: number,
  at: string,
  width: number,
  most: number,
): SettledNeed {
  const fresh = { need, at, width, most }
  if (held === null || held.at !== at || held.width !== width || held.most !== most) return fresh
  return held
}

/**
 * The room the commanders' box takes besides its cards and one gap, in px: the padding of its line and of the hand's,
 * its divider line and the dock's own gap. Counted as the DOM draws it, so a full row never runs past the dock.
 */
export const COMMAND_GAP_PX = 13

/** What the hand dock keeps free beside its cards in px, both sides: the padding of its section (8 each) and of its line (4 each). */
export const HAND_DOCK_SIDES_PX = 24

/**
 * The width the hand dock needs in px to hold `count` cards in one row of `HAND_FLOOR_PX` cards, with `commanders` and
 * their divider at its head.
 */
export function handRowPx(count: number, commanders: number): number {
  const cards = Math.max(1, count)
  const head = commanders > 0 ? commanders * (HAND_FLOOR_PX + HAND_GAP_PX) + COMMAND_GAP_PX : 0
  return head + cards * HAND_FLOOR_PX + (cards - 1) * HAND_GAP_PX + HAND_DOCK_SIDES_PX
}

/** What `handLayout` needs. */
export interface HandInput {
  /** The room for the line in px, less the padding kept round it. */
  width: number
  /** The height the cards may take in px, less the padding. */
  height: number
  count: number
  /** A touch screen: a hand that does not fit keeps the full card size and scrolls, since a thumb needs the room. */
  coarse?: boolean
  /** The widest card in px; `FIT_LIMITS.maxWidth` when left out. */
  cap?: number
}

/** The size, rows and lift of the cards of a hand. */
export interface HandLayout {
  /** One card's box in px, 5:7. */
  box: { width: number; height: number }
  /** The space between neighbours of a row in px. */
  gap: number
  /** The rows the hand wraps into; the cards are in reading order, row by row. */
  rows: number
  /** The cards of the first (longest) row. */
  perRow: number
  /** How far a row lies over the row before it, in px: the card's height less `HAND_HEAD_PX`; 0 for one row. */
  overlap: number
  /** Even a single row of the smallest card is wider than the dock, so it scrolls sideways. */
  scrolls: boolean
  /** How far a hovered or focused card rises, in px: `HAND_LIFT` of its height. */
  lift: number
}

/**
 * The cards as tall as the dock allows in one row, none under another. A hand that does not fit shrinks the cards down
 * to `HAND_FLOOR_PX` first, then wraps into two or three balanced rows, each lying over the one before it so that its
 * top (name and cost) shows, and scrolls only when even that does not hold it.
 */
export function handLayout({ width, height, count, coarse = false, cap }: HandInput): HandLayout {
  const widest = handCardWidth(height, cap)
  const floor = Math.min(widest, HAND_FLOOR_PX)
  const room = Math.max(0, width)
  const cards = Math.max(1, count)
  const heightOf = (cardWidth: number) => Math.round(cardWidth * CARD_ASPECT)
  const make = (cardWidth: number, rows: number, perRow: number, scrolls: boolean): HandLayout => {
    const boxHeight = heightOf(cardWidth)
    return {
      box: { width: cardWidth, height: boxHeight },
      gap: HAND_GAP_PX,
      rows,
      perRow,
      overlap: rows > 1 ? Math.max(0, boxHeight - HAND_HEAD_PX) : 0,
      scrolls,
      lift: Math.round(boxHeight * HAND_LIFT),
    }
  }
  for (let rows = 1; rows <= HAND_MAX_ROWS && rows <= cards; rows += 1) {
    const perRow = Math.ceil(cards / rows)
    for (let cardWidth = widest; ; cardWidth = Math.max(floor, cardWidth - FIT_LIMITS.step)) {
      const boxHeight = heightOf(cardWidth)
      const stack = boxHeight + (rows - 1) * Math.min(boxHeight, HAND_HEAD_PX)
      const holds = rows === 1 || stack <= height
      if (holds && perRow * cardWidth + (perRow - 1) * HAND_GAP_PX <= room) {
        return make(cardWidth, rows, perRow, false)
      }
      if (cardWidth === floor) break
    }
  }
  return make(coarse ? widest : floor, 1, cards, true)
}

/** The whole cards a hand keeps beside a head that does not fit the line (a large Satchel). */
export const HAND_KEPT_BESIDE_HEAD = 2

/** What `dockLayouts` needs: a hand line's room, with `commanders` waiting at its head. */
export interface DockInput extends HandInput {
  commanders: number
  /** A decision picks among the hand's cards: the hand takes the room it needs first, the head what is left. */
  pick?: boolean
}

/** The layouts of a dock's hand and of its commanders, and the width the commanders' list may take. */
export interface DockLayouts {
  hand: HandLayout
  /** The commanders' list, in one row of the hand's card size; scrolls when the hand needs the rest of the line. */
  command?: HandLayout
  /** The widest the commanders' list and its divider may be in px. */
  commandWidth?: number
}

/**
 * The hand and the commanders at its head share the line: the commanders take one card's width each and a divider,
 * but never so much that the hand keeps fewer than `HAND_KEPT_BESIDE_HEAD` whole cards (or all of a smaller hand);
 * the commanders then scroll in what is left, at least one of them in view.
 */
export function dockLayouts({ commanders, pick = false, ...input }: DockInput): DockLayouts {
  if (commanders <= 0) return { hand: handLayout(input) }
  if (pick && input.count > 0) return pickLayouts(commanders, input)
  const all = handLayout({ ...input, count: input.count + commanders })
  const card = all.box.width + all.gap
  const head = commanders * card + COMMAND_GAP_PX
  // A touch screen's hand that scrolls keeps the full card size, so that is the card it must hold.
  const whole = input.coarse === true ? handCardWidth(input.height, input.cap) : all.box.width
  // Two whole cards where the line holds them beside one of the head's, else one.
  const roomy = input.width >= (HAND_KEPT_BESIDE_HEAD + 1) * (whole + all.gap) + COMMAND_GAP_PX
  const kept = Math.min(input.count, roomy ? HAND_KEPT_BESIDE_HEAD : 1)
  const keep = kept > 0 ? kept * whole + (kept - 1) * all.gap : 0
  const most = Math.max(card + COMMAND_GAP_PX, input.width - keep)
  const commandWidth = Math.min(head, most)
  const hand = handLayout({ ...input, width: Math.max(0, input.width - commandWidth) })
  const command: HandLayout = {
    ...hand,
    rows: 1,
    perRow: commanders,
    scrolls: commandWidth < head,
    overlap: 0,
  }
  return { hand, command, commandWidth }
}

/**
 * The line while a decision picks among the hand's cards (bottoming, a discard), where the shared line does not hold
 * both whole: the hand is laid out first, beside one card of the head, and the head (commanders, the Satchel, the top
 * card) takes what the hand leaves and scrolls in it, so every card on offer stays in view.
 */
function pickLayouts(commanders: number, input: HandInput): DockLayouts {
  const shared = dockLayouts({ ...input, commanders })
  if (shared.command?.scrolls !== true && !shared.hand.scrolls) return shared
  // The head keeps one card and its divider: first at the narrowest card, then at the hand's card where that is wider.
  const handBeside = (card: number): HandLayout =>
    handLayout({ ...input, width: Math.max(0, input.width - (card + HAND_GAP_PX + COMMAND_GAP_PX)) })
  const first = handBeside(Math.min(HAND_FLOOR_PX, handCardWidth(input.height, input.cap)))
  const hand = first.scrolls ? first : handBeside(first.box.width)
  const room = Math.max(0, input.width - (hand.box.width + HAND_GAP_PX + COMMAND_GAP_PX))
  const row = hand.scrolls ? room : hand.perRow * hand.box.width + (hand.perRow - 1) * hand.gap
  const card = hand.box.width + hand.gap
  const head = commanders * card + COMMAND_GAP_PX
  const commandWidth = Math.min(head, Math.max(card + COMMAND_GAP_PX, input.width - row))
  const command: HandLayout = {
    ...hand,
    rows: 1,
    perRow: commanders,
    scrolls: commandWidth < head,
    overlap: 0,
  }
  return { hand, command, commandWidth }
}
