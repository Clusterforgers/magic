// The measurements of the layout gate. `layout.spec.ts` runs them in Playwright and `scripts/ui-audit.mjs` on any URL.
// It imports nothing, so Node can load this file as it stands.

/** A window size in px. */
export interface Size {
  width: number
  height: number
}

/** The six sizes every page is held to, largest first (docs/ui-design.md, Definition of done). */
export const SIZES: readonly Size[] = [
  { width: 1920, height: 1080 },
  { width: 1440, height: 900 },
  { width: 1366, height: 768 },
  { width: 1280, height: 720 },
  { width: 1024, height: 768 },
  { width: 390, height: 844 },
]

/** The two colour schemes of the app. */
export const THEMES = ['light', 'dark'] as const

/** One of the colour schemes. */
export type Theme = (typeof THEMES)[number]

/** The smallest text in px: the 11 px card label of the type scale. */
export const TEXT_FLOOR = 11

/** A size as the tables print it: `1280x720`. */
export const sizeName = (size: Size): string => `${size.width}x${size.height}`

/** Reads `1280x720,390x844`; throws on anything that is not a list of `WxH`. */
export function parseSizes(text: string): Size[] {
  return text.split(',').map((part) => {
    const match = /^\s*(\d{2,5})x(\d{2,5})\s*$/.exec(part)
    if (match === null) throw new Error(`not a size: "${part}" (use WIDTHxHEIGHT)`)
    return { width: Number(match[1]), height: Number(match[2]) }
  })
}

/** A box in window coordinates. */
export interface Box {
  left: number
  top: number
  right: number
  bottom: number
}

/** What `measure` found in a page; plain data, so it crosses from the page to the test. */
export interface Look {
  /** The window as the page is laid out in it, scrollbars not counted. */
  inner: { width: number; height: number }
  /** How big the document is and where it is scrolled to. */
  scroll: { width: number; height: number; x: number; y: number }
  /** Elements cut off sideways, by the window or by a box that clips them; the outermost of each. */
  cutOff: string[]
  /** Text under the floor: its size and the start of it. */
  small: string[]
  /** The primary actions asked for, by name: where they are and whether nothing lies over them; null when absent. */
  actions: Record<string, { box: Box; free: boolean } | null>
  /** Lanes of a battlefield that lie over another lane of it, or outside a board that does not scroll. */
  lanes: string[]
}

/** What `measure` is asked. */
export interface Ask {
  /** The primary actions of the page, name to CSS selector. */
  actions: Record<string, string>
  /** Scroll an action into view before it is judged (a page that may scroll), then put the scroll back. */
  reveal: boolean
  /** Regions whose contents may be cut off sideways on purpose: a strip that scrolls, the art of a card. */
  exempt: string[]
  /** The smallest text size in px. */
  floor: number
}

/**
 * Runs in the page: reads the layout once, so nothing changes halfway. It cannot use anything outside its own body,
 * because Playwright sends it as source.
 */
export function measure(ask: Ask): Look {
  const root = document.documentElement
  const inner = { width: root.clientWidth, height: root.clientHeight }
  const slack = 0.5
  const boxOf = (element: Element): Box => {
    const { left, top, right, bottom } = element.getBoundingClientRect()
    return { left, top, right, bottom }
  }
  const named = (element: Element): string => {
    const tag = element.tagName.toLowerCase()
    const text = (element.getAttribute('aria-label') ?? element.textContent ?? '')
      .replace(/\s+/g, ' ')
      .trim()
      .slice(0, 24)
    return text === '' ? tag : `${tag} "${text}"`
  }
  // Nothing to see: not drawn, or the 1 px box of a visually hidden label.
  const unseen = (style: CSSStyleDeclaration, rect: DOMRect): boolean =>
    style.display === 'none' || style.visibility === 'hidden' || (rect.width <= 1 && rect.height <= 1)

  // The sideways span a box clips its children to: its own where its overflow is not visible, else the span it is
  // clipped to itself. A fixed box is clipped by no ancestor, an absolute one only down from its containing block.
  type Span = { left: number; right: number }
  const open: Span = { left: -Infinity, right: Infinity }
  const clipping = new Set(['hidden', 'clip', 'scroll', 'auto'])
  const holds = new Map<Element, Span>()
  // The nearest positioned ancestor; a drawing has no `offsetParent`, so it is looked for by hand.
  const containingBlock = (element: Element): Element | null => {
    const parent: Element | null | undefined = (element as HTMLElement).offsetParent
    if (parent !== undefined) return parent
    let up = element.parentElement
    while (up !== null && getComputedStyle(up).position === 'static') up = up.parentElement
    return up
  }
  const clipOf = (element: Element, style: CSSStyleDeclaration): Span => {
    if (style.position === 'fixed') return open
    if (style.position === 'absolute') return holdOf(containingBlock(element))
    return holdOf(element.parentElement)
  }
  const holdOf = (element: Element | null): Span => {
    if (element === null || element === root) return open
    const known = holds.get(element)
    if (known !== undefined) return known
    const style = getComputedStyle(element)
    const outer = clipOf(element, style)
    let span = outer
    if (clipping.has(style.overflowX)) {
      const { left, right } = element.getBoundingClientRect()
      span = { left: Math.max(outer.left, left), right: Math.min(outer.right, right) }
    }
    holds.set(element, span)
    return span
  }

  const cutOff: string[] = []
  const flagged = new Set<Element>()
  const exempt = ask.exempt.join(',')
  for (const element of Array.from(document.body.querySelectorAll('*'))) {
    // The parts of a drawing belong to the drawing.
    if (element.tagName.toLowerCase() !== 'svg' && element.closest('svg') !== null) continue
    if (exempt !== '' && element.parentElement?.closest(exempt)) continue
    const style = getComputedStyle(element)
    const rect = element.getBoundingClientRect()
    if (unseen(style, rect)) continue
    const span = clipOf(element, style)
    const left = Math.max(0, span.left)
    const right = Math.min(inner.width, span.right)
    if (rect.left >= left - slack && rect.right <= right + slack) continue
    const parent = element.parentElement
    const inherited = parent !== null && flagged.has(parent)
    flagged.add(element)
    if (!inherited) cutOff.push(`${named(element)} ${Math.round(rect.left)}..${Math.round(rect.right)}`)
  }

  const small: string[] = []
  const words = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT)
  for (let node = words.nextNode(); node !== null && small.length < 20; node = words.nextNode()) {
    const text = (node.nodeValue ?? '').replace(/\s+/g, ' ').trim()
    const parent = node.parentElement
    // The letters of a mana symbol are part of a picture that has its own spoken name (`role="img"`).
    if (text === '' || parent === null || parent.closest('svg, [role="img"]') !== null) continue
    const style = getComputedStyle(parent)
    if (unseen(style, parent.getBoundingClientRect())) continue
    const size = parseFloat(style.fontSize)
    if (size < ask.floor) small.push(`${Math.round(size * 10) / 10}px "${text.slice(0, 24)}"`)
  }

  const inside = (box: Box): boolean =>
    box.left >= -slack &&
    box.top >= -slack &&
    box.right <= inner.width + slack &&
    box.bottom <= inner.height + slack
  const spots = [
    { x: 0.5, y: 0.5 },
    { x: 0.25, y: 0.25 },
    { x: 0.75, y: 0.25 },
    { x: 0.25, y: 0.75 },
    { x: 0.75, y: 0.75 },
  ]
  // The first match that is drawn. `text:Ready` matches a button or link of the page by its words, for the controls
  // that have no hook of their own.
  const find = (selector: string): Element | null => {
    const wanted = selector.slice('text:'.length)
    const found = selector.startsWith('text:')
      ? Array.from(document.querySelectorAll('main button, main a, main [role="button"]')).filter(
          (control) =>
            (control.textContent ?? '').trim() === wanted || control.getAttribute('aria-label') === wanted,
        )
      : Array.from(document.querySelectorAll(selector))
    return (
      found.find((element) => {
        const { width, height } = element.getBoundingClientRect()
        return width > 0 && height > 0
      }) ?? null
    )
  }
  const actions: Look['actions'] = {}
  const was = { x: window.scrollX, y: window.scrollY }
  for (const [name, selector] of Object.entries(ask.actions)) {
    const element = find(selector)
    if (element === null) {
      actions[name] = null
      continue
    }
    let box = boxOf(element)
    if (ask.reveal && !inside(box)) {
      element.scrollIntoView({ block: 'center', inline: 'nearest' })
      box = boxOf(element)
    }
    // A disabled control has its pointer events off, so the hit test would skip it: it is tested as if they were on.
    const target = element as HTMLElement
    const kept = target.style.pointerEvents
    target.style.pointerEvents = 'auto'
    // A point outside the window has nothing to hit; the action being outside is a finding of its own.
    const free = spots.every((spot) => {
      const top = document.elementFromPoint(
        box.left + (box.right - box.left) * spot.x,
        box.top + (box.bottom - box.top) * spot.y,
      )
      return top === null || element.contains(top)
    })
    target.style.pointerEvents = kept
    actions[name] = { box, free }
  }
  if (ask.reveal) window.scrollTo(was.x, was.y)

  // The lanes of a battlefield never lie over each other, and on a board that does not scroll every lane is inside it.
  const lanes: string[] = []
  for (const board of Array.from(document.querySelectorAll('[data-battlefield-seat]'))) {
    const seat = board.getAttribute('data-battlefield-seat') ?? '?'
    const placed = Array.from(board.querySelectorAll('[data-lane]')).map((lane) => ({
      name: lane.getAttribute('data-lane') ?? 'lane',
      box: boxOf(lane),
    }))
    placed.forEach((one, index) => {
      for (const two of placed.slice(index + 1)) {
        const across = Math.min(one.box.right, two.box.right) - Math.max(one.box.left, two.box.left)
        const down = Math.min(one.box.bottom, two.box.bottom) - Math.max(one.box.top, two.box.top)
        if (across > slack && down > slack) lanes.push(`seat ${seat} ${one.name} over ${two.name}`)
      }
    })
    const zone = board.querySelector('[data-slot="battlefield-zone"]')
    if (zone === null || zone.scrollHeight > zone.clientHeight + 1) continue
    const room = boxOf(zone)
    for (const lane of placed) {
      const out =
        lane.box.left < room.left - slack ||
        lane.box.right > room.right + slack ||
        lane.box.top < room.top - slack ||
        lane.box.bottom > room.bottom + slack
      if (out) lanes.push(`seat ${seat} ${lane.name} outside its board`)
    }
  }

  return {
    lanes,
    inner,
    scroll: { width: root.scrollWidth, height: root.scrollHeight, x: window.scrollX, y: window.scrollY },
    cutOff,
    small,
    actions,
  }
}

/** What a page is expected to do. */
export interface Expect {
  /** The page is meant to fit the window, so it never scrolls down (every page never scrolls sideways). */
  fits: boolean
}

const within = (box: Box, inner: Look['inner']): boolean =>
  box.left >= -0.5 && box.top >= -0.5 && box.right <= inner.width + 0.5 && box.bottom <= inner.height + 0.5

/** How many findings of one kind a table cell lists before it counts the rest. */
const LISTED = 3

function listed(kind: string, found: readonly string[]): string[] {
  const shown = found.slice(0, LISTED).map((what) => `${kind} ${what}`)
  return found.length > LISTED ? [...shown, `${kind} ${found.length - LISTED} more`] : shown
}

/** Judges what `measure` found: one short sentence for every rule the page breaks; none when it keeps them all. */
export function problemsOf(look: Look, expect: Expect): string[] {
  const { inner, scroll } = look
  const problems: string[] = []
  if (scroll.width > inner.width) problems.push(`scrolls sideways ${scroll.width} > ${inner.width}`)
  if (scroll.x !== 0) problems.push('was scrolled sideways')
  if (expect.fits && scroll.height > inner.height)
    problems.push(`scrolls down ${scroll.height} > ${inner.height}`)
  if (expect.fits && scroll.y !== 0) problems.push('was scrolled down')
  problems.push(...listed('cut off:', look.cutOff))
  problems.push(...listed('small text:', [...new Set(look.small)]))
  problems.push(...listed('lane:', look.lanes))
  for (const [name, action] of Object.entries(look.actions)) {
    if (action === null) problems.push(`${name} is missing`)
    else if (!within(action.box, inner)) problems.push(`${name} is outside the window`)
    else if (!action.free) problems.push(`${name} is covered`)
  }
  return problems
}

/**
 * Measures through `look` until the page keeps the rules or the time is up, and returns what is still wrong. A page
 * lays itself out a frame or more after the window changes, so a first look can show the old layout.
 */
export async function settle(
  look: () => Promise<Look>,
  expect: Expect,
  options: { timeoutMs?: number; stepMs?: number } = {},
): Promise<string[]> {
  const { timeoutMs = 4_000, stepMs = 100 } = options
  const deadline = Date.now() + timeoutMs
  for (;;) {
    const problems = problemsOf(await look(), expect)
    if (problems.length === 0 || Date.now() >= deadline) return problems
    await new Promise((resolve) => setTimeout(resolve, stepMs))
  }
}

/** One line of the audit: a page at a size in a theme and what was wrong. */
export interface Row {
  page: string
  size: string
  theme: string
  problems: readonly string[]
}

/** The rows as an aligned table; a row that keeps every rule says `ok`. */
export function formatTable(rows: readonly Row[]): string {
  const lines = [
    ['page', 'size', 'theme', 'result'],
    ...rows.map((row) => [
      row.page,
      row.size,
      row.theme,
      row.problems.length === 0 ? 'ok' : row.problems.join('; '),
    ]),
  ]
  const widths = [0, 1, 2].map((column) => Math.max(...lines.map((line) => (line[column] ?? '').length)))
  return lines
    .map((line) =>
      [...widths.map((width, column) => (line[column] ?? '').padEnd(width)), line[3] ?? ''].join('  '),
    )
    .join('\n')
}

/** Runs in every page of a context before its own scripts: stores the theme choice that `index.html` reads. */
export function storeTheme(theme: string): void {
  try {
    localStorage.setItem('theme', theme)
  } catch {
    // Storage can be blocked; the colour scheme of the context still applies.
  }
}
