/**
 * Twins in a choice of cards: offered cards that share a name. Each gets a number, drawn on its picture in the choice
 * and on the same object of the table while the prompt is up, so the two read as one; a pointer or focus on a picture
 * rings its object on the table (`twinStyle`). A screen reader hears the number and what tells them apart (`twinFacts`).
 */
import type { ObjectId, ObjectView } from '@/protocol/generated'
import { counterWords } from '../cardModel'

/** The number of every card of `ids` whose name another card of `ids` shares, counted in list order per name. */
export function twinNumbers(
  ids: readonly ObjectId[],
  objects: ReadonlyMap<ObjectId, ObjectView | undefined>,
): Map<ObjectId, number> {
  const total = new Map<string, number>()
  for (const id of ids) {
    const name = objects.get(id)?.name
    if (name !== undefined) total.set(name, (total.get(name) ?? 0) + 1)
  }
  const seen = new Map<string, number>()
  const numbers = new Map<ObjectId, number>()
  for (const id of ids) {
    const name = objects.get(id)?.name
    if (name === undefined || (total.get(name) ?? 0) < 2) continue
    const nth = (seen.get(name) ?? 0) + 1
    seen.set(name, nth)
    numbers.set(id, nth)
  }
  return numbers
}

/** What a face shows of whose and where an object is: two cards of one name differ here unless they are twins. */
const placeKey = (object: ObjectView): string =>
  `${object.name}|${object.controller}|${JSON.stringify(object.zone)}`

/**
 * The numbers of `numbers` worth drawing: the cards whose name another card shares under the same controller and in
 * the same zone, which nothing else on the face tells apart (a seat's run of cards and its name tell the others).
 */
export function shownTwins(
  numbers: ReadonlyMap<ObjectId, number>,
  objects: ReadonlyMap<ObjectId, ObjectView | undefined>,
): Map<ObjectId, number> {
  const count = new Map<string, number>()
  for (const id of numbers.keys()) {
    const object = objects.get(id)
    if (object !== undefined) count.set(placeKey(object), (count.get(placeKey(object)) ?? 0) + 1)
  }
  const shown = new Map<ObjectId, number>()
  for (const [id, number] of numbers) {
    const object = objects.get(id)
    if (object !== undefined && (count.get(placeKey(object)) ?? 0) > 1) shown.set(id, number)
  }
  return shown
}

/** What tells a permanent from its twin, in words: tapped, damage, counters and what is attached to it or it to. */
export function twinFacts(object: ObjectView, nameOf: (id: ObjectId) => string | undefined): string[] {
  const facts: string[] = []
  const permanent = object.permanent
  if (permanent?.tapped === true) facts.push('tapped')
  if ((permanent?.damage ?? 0) > 0) facts.push(`${permanent?.damage ?? 0} damage`)
  facts.push(...counterWords(object))
  const host = object.attached_to === undefined ? undefined : nameOf(object.attached_to)
  if (host !== undefined) facts.push(`attached to ${host}`)
  const attachments = (object.attachments ?? []).flatMap((id) => {
    const name = nameOf(id)
    return name === undefined ? [] : [name]
  })
  if (attachments.length > 0) facts.push(`with ${attachments.join(' and ')}`)
  return facts
}

/** A table card of the object `id`, for a rule of the page's style. */
const tableCard = (id: ObjectId): string =>
  `:is([data-area='opponents'],[data-area='own'],[data-area='hand']) button[data-object-id='${id}']`

/**
 * The style that numbers the twins' objects on the table (a chip at the middle of its left edge, the same as the picture's) and
 * rings the one whose picture is pointed at or focused; empty when there is neither.
 */
export function twinStyle(numbers: ReadonlyMap<ObjectId, number>, pointed: ObjectId | null): string {
  const rules: string[] = []
  if (numbers.size > 0) {
    rules.push(
      `${[...numbers.keys()].map((id) => `${tableCard(id)}::after`).join(',')}{position:absolute;top:50%;left:-0.5rem;transform:translateY(-50%);z-index:5;min-width:1.25rem;height:1.25rem;padding:0 0.25rem;border-radius:999px;background:var(--primary);color:var(--primary-foreground);font:700 0.75rem/1.25rem var(--font-sans,inherit);text-align:center;box-shadow:0 0 0 2px var(--background);pointer-events:none}`,
      ...[...numbers].map(([id, number]) => `${tableCard(id)}::after{content:'${number}'}`),
    )
  }
  if (pointed !== null) {
    // Lifted over the overlay's scrim, so the object shows wherever the panel leaves it in view.
    rules.push(
      `${tableCard(pointed)}{z-index:16;outline:3px solid var(--selected);outline-offset:3px;filter:none}`,
    )
  }
  return rules.join('')
}
