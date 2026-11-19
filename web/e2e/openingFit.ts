// The game modes of the table as rows for the fit specs, and the rules the floating opening hand and the empty boards keep (docs/web.md, Opening hands, Board split).
import type { Page } from '@playwright/test'

/** A mode of the table on `/dev/table`: its name for the messages and the address parameters that make it. */
export interface TableMode {
  name: string
  query: string
}

/**
 * Every mode a game can open in (the dev table is a Commander game; a real duel and a tournament game draw the same
 * table as `a duel`, which `fit.spec.ts` also plays for real).
 */
export const TABLE_MODES: readonly TableMode[] = [
  { name: 'a duel', query: 'seats=2' },
  { name: 'three seats', query: 'seats=3' },
  { name: 'four seats', query: 'seats=4' },
  { name: 'six seats', query: 'seats=6' },
  { name: 'two teams of two', query: 'seats=4&teams=2v2' },
  { name: 'Two-Headed Giant', query: 'seats=4&teams=2hg' },
  { name: 'twelve seats', query: 'seats=12' },
  { name: 'a boss fight of one hero', query: 'seats=2&boss=0&rework=1' },
  { name: 'a boss fight of two heroes', query: 'seats=3&boss=0&rework=1' },
  { name: 'a boss fight of four heroes', query: 'seats=5&boss=0&rework=1' },
  { name: 'a boss with two minions', query: 'seats=4&boss=2&rework=1' },
]

/** The least share of the boards' room the opponents' row keeps with every board empty (`MIN_ROW_SHARE`). */
export const EMPTY_ROW_LEAST_SHARE = 0.25

interface Box {
  left: number
  top: number
  right: number
  bottom: number
}

/** What the floating opening hand and the table around it measure, read in one step. */
interface OpeningLook {
  phone: boolean
  panel: Box | null
  /** Where the host stands: over the whole table (`table`) or over the opponents and the strip (`top`). */
  place: string | null
  /** The table between the top bar and the hand dock, across the main area. */
  table: Box
  /** The opponents' and the strip's rows together. */
  top: Box
  cards: Box[]
  sentence: Box | null
  answers: Box[]
  scrolls: boolean
  lifeFree: boolean
  window: { width: number; height: number }
}

function readOpening(): OpeningLook {
  const box = (element: Element | null | undefined): Box | null => {
    if (element === null || element === undefined) return null
    const { left, top, right, bottom } = element.getBoundingClientRect()
    return { left, top, right, bottom }
  }
  const must = (selector: string): Box =>
    box(document.querySelector(selector)) ?? { left: 0, top: 0, right: 0, bottom: 0 }
  const panel = document.querySelector<HTMLElement>('[data-opening]')
  const main = must('[data-slot="table-main"]')
  const bar = must('[data-area="bar"]')
  const hand = must('[data-area="hand"]')
  const shared = must('[data-area="shared"]')
  const life = document.querySelector('[data-you] [data-slot="life"]')
  const at = box(life)
  const hit =
    at === null ? null : document.elementFromPoint((at.left + at.right) / 2, (at.top + at.bottom) / 2)
  const answers = [...(panel?.querySelectorAll('[data-slot="prompt-actions"] button') ?? [])]
  return {
    phone: document.querySelector('[data-slot="table-grid"]')?.getAttribute('data-breakpoint') === 'xs',
    panel: box(panel),
    place: document.querySelector('[data-slot="choice-host"]')?.getAttribute('data-opening-place') ?? null,
    table: { left: main.left, right: main.right, top: bar.bottom, bottom: hand.top },
    top: { left: main.left, right: main.right, top: bar.bottom, bottom: shared.bottom },
    cards: [...document.querySelectorAll('[data-opening] [data-slot="opening-card"]')].flatMap(
      (card) => box(card) ?? [],
    ),
    sentence: box(panel?.querySelector('[data-slot="prompt-text"]')),
    answers: answers.flatMap((each) => box(each) ?? []),
    scrolls: panel === null || panel.scrollHeight > panel.clientHeight + 1,
    lifeFree: life !== null && hit !== null && life.contains(hit),
    window: { width: window.innerWidth, height: window.innerHeight },
  }
}

const overlaps = (a: Box, b: Box): boolean =>
  Math.min(a.right, b.right) - Math.max(a.left, b.left) > 0.5 &&
  Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top) > 0.5

const middle = (box: Box): { x: number; y: number } => ({
  x: (box.left + box.right) / 2,
  y: (box.top + box.bottom) / 2,
})

/**
 * What is wrong with the floating opening hand (off a phone): it stands centred over the table between the top bar and
 * the dock (over the opponents and the strip in a narrow window), every card whole in one row inside it, the answers
 * under the cards and clear of the sentence, the panel not scrolling, your life uncovered. Nothing on a phone.
 */
export async function openingProblems(page: Page, least = 64): Promise<string[]> {
  const look = await page.evaluate(readOpening)
  if (look.phone) return []
  const problems: string[] = []
  const panel = look.panel
  if (panel === null) return ['the opening panel is missing']
  const area = look.place === 'top' ? look.top : look.table
  if (look.place !== 'table' && look.place !== 'top')
    problems.push(`the host stands in no known place (${look.place})`)
  const [centre, room] = [middle(panel), middle(area)]
  if (Math.abs(centre.x - room.x) > 2)
    problems.push(`the panel is ${Math.round(centre.x - room.x)} px off the table's centre across`)
  if (Math.abs(centre.y - room.y) > 2)
    problems.push(`the panel is ${Math.round(centre.y - room.y)} px off the table's centre down`)
  if (panel.top < area.top - 0.5 || panel.bottom > area.bottom + 0.5)
    problems.push('the panel leaves the table')
  if (look.scrolls) problems.push('the panel scrolls')
  if (look.cards.length === 0) problems.push('no card is drawn')
  for (const card of look.cards) {
    if (
      card.top < panel.top - 0.5 ||
      card.bottom > panel.bottom + 0.5 ||
      card.left < panel.left - 0.5 ||
      card.right > panel.right + 0.5
    ) {
      problems.push('a card is cut by the panel')
      break
    }
  }
  if (new Set(look.cards.map((card) => Math.round(card.top))).size > 1)
    problems.push('the cards are not one row')
  const width = Math.min(...look.cards.map((card) => card.right - card.left))
  if (width < least - 0.5) problems.push(`the cards are ${Math.round(width)} px wide, under ${least}`)
  const row = look.cards.reduce((most, card) => Math.max(most, card.bottom), 0)
  for (const answer of look.answers) {
    if (answer.top < row - 0.5) problems.push('an answer stands beside the cards')
    if (look.sentence !== null && overlaps(answer, look.sentence))
      problems.push('an answer lies over the sentence')
  }
  if (look.answers.length === 0) problems.push('Keep and Mulligan are missing')
  if (!look.lifeFree) problems.push('your life is covered')
  return problems
}

/** The heights of the opponents' row and yours in px. */
export async function boardRows(page: Page): Promise<{ opponents: number; own: number; phone: boolean }> {
  return page.evaluate(() => {
    const height = (name: string): number =>
      document.querySelector(`[data-area="${name}"]`)?.getBoundingClientRect().height ?? 0
    return {
      opponents: height('opponents'),
      own: height('own'),
      phone: document.querySelector('[data-slot="table-grid"]')?.getAttribute('data-breakpoint') === 'xs',
    }
  })
}
