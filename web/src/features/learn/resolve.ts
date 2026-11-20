import type { ObjectId, ObjectView } from '@/protocol/generated'
import { ANCHORS, cardRef, isAnchor } from './anchors'
import type { AnchorId, Target } from './anchors'

/** A box in window pixels. */
export interface Box {
  x: number
  y: number
  width: number
  height: number
}

/** What a target resolves to: one drawn element, the opener that shows it, or nothing (with why). */
export type Resolved =
  | { kind: 'found'; element: HTMLElement; via: AnchorId | null }
  | { kind: 'none' }
  | { kind: 'many'; count: number }

/** The box of `element`. */
export function boxOf(element: Element): Box {
  const rect = element.getBoundingClientRect()
  return { x: rect.left, y: rect.top, width: rect.width, height: rect.height }
}

/** Whether `element` is drawn: it has a size, lies at least partly inside the window and is not hidden. */
export function drawn(element: HTMLElement, win: Window = window): boolean {
  const box = boxOf(element)
  if (box.width <= 0 || box.height <= 0) return false
  if (box.x + box.width <= 0 || box.y + box.height <= 0) return false
  if (box.x >= win.innerWidth || box.y >= win.innerHeight) return false
  const style = win.getComputedStyle(element)
  return style.visibility !== 'hidden' && style.display !== 'none' && Number(style.opacity) !== 0
}

/** Whether `element` is enabled: not disabled and not marked `aria-disabled`. */
export function enabled(element: HTMLElement): boolean {
  if (element.matches(':disabled')) return false
  return element.getAttribute('aria-disabled') !== 'true'
}

/** Whether `element` is on top at its centre: what the window shows there is the element or inside it. */
export function onTop(element: HTMLElement, doc: Document = document): boolean {
  const box = boxOf(element)
  const x = Math.min(Math.max(box.x + box.width / 2, 0), doc.defaultView?.innerWidth ?? 0)
  const y = Math.min(Math.max(box.y + box.height / 2, 0), doc.defaultView?.innerHeight ?? 0)
  const hit = typeof doc.elementFromPoint === 'function' ? doc.elementFromPoint(x, y) : element
  return hit !== null && (element === hit || element.contains(hit))
}

/** The drawn elements carrying anchor `id`. */
function anchored(id: AnchorId, doc: Document): HTMLElement[] {
  return [...doc.querySelectorAll<HTMLElement>(`[data-coach="${id}"]`)].filter((element) =>
    drawn(element, doc.defaultView ?? window),
  )
}

/** What the card refs read: names of objects, and the cards the pending decision can use. */
export interface CardLookup {
  objects: Readonly<Record<number, ObjectView | undefined>>
  usable: ReadonlySet<ObjectId>
}

/** The drawn card called `name` in `zone`'s element: the first the decision can use, else the first. */
function card(zone: string, name: string, lookup: CardLookup, doc: Document): HTMLElement[] {
  // A prompt that lays its cards out to choose from (a target on a phone) is where that card is pressed.
  const offered = [
    ...doc.querySelectorAll<HTMLElement>('[data-slot="card-choices"] button[data-candidate]'),
  ].filter(
    (each) =>
      lookup.objects[Number(each.dataset['candidate'])]?.name === name &&
      drawn(each, doc.defaultView ?? window),
  )
  if (offered[0] !== undefined && zone !== 'hand' && zone !== 'command') return [offered[0]]
  const zones = [...doc.querySelectorAll<HTMLElement>(`[data-coach-zone="${zone}"]`)]
  const cards = zones.flatMap((element) =>
    [...element.querySelectorAll<HTMLElement>('[data-object-id]')].filter((each) => {
      if (each.closest('[data-coach-zone]') !== element) return false
      // The outermost element of a card: a face inside it carries the same id.
      const outer = each.parentElement?.closest(`[data-object-id="${each.dataset['objectId']}"]`)
      if (outer !== null && outer !== undefined && element.contains(outer)) return false
      const id = Number(each.dataset['objectId'])
      return lookup.objects[id]?.name === name && drawn(each, doc.defaultView ?? window)
    }),
  )
  const usable = cards.filter((each) => lookup.usable.has(Number(each.dataset['objectId'])))
  const first = usable[0] ?? cards[0]
  return first === undefined ? [] : [first]
}

/**
 * `target` as one drawn element: an anchor (or, while it is hidden, its opener, recursively) or a card ref. None or
 * several are reported, never guessed.
 */
export function resolve(target: Target, lookup: CardLookup, doc: Document = document): Resolved {
  const ref = cardRef(target)
  if (ref !== null) {
    const found = card(ref.zone, ref.name, lookup, doc)
    const [element] = found
    return element === undefined ? { kind: 'none' } : { kind: 'found', element, via: null }
  }
  if (!isAnchor(target)) return { kind: 'none' }
  const seen = new Set<string>()
  let id: AnchorId = target
  for (;;) {
    const found = anchored(id, doc)
    if (found.length > 1) return { kind: 'many', count: found.length }
    const [element] = found
    if (element !== undefined) return { kind: 'found', element, via: id === target ? null : id }
    // Of several openers the first drawn (`side.tab.stack` off a phone, `sheet.grab` on one), else the first.
    const openers = [ANCHORS[id].opener ?? []].flat().filter((each): each is AnchorId => isAnchor(each))
    const opener = openers.find((each) => anchored(each, doc).length > 0) ?? openers[0]
    if (opener === undefined || seen.has(opener)) return { kind: 'none' }
    seen.add(opener)
    id = opener
  }
}

/** The anchors drawn now, for a step's page facts. */
export function shownAnchors(ids: readonly AnchorId[], doc: Document = document): Set<AnchorId> {
  return new Set(ids.filter((id) => anchored(id, doc).length > 0))
}
