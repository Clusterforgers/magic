import { expect } from '@playwright/test'
import type { Page } from '@playwright/test'
import { WEB_URL } from './ports'

/** A window the course is played at. */
export interface Layout {
  name: string
  width: number
  height: number
  touch: boolean
}

/** The three layouts of `docs/tutorial.md`, Tests. */
export const LAYOUTS: readonly Layout[] = [
  { name: 'desktop', width: 1920, height: 1080, touch: false },
  { name: 'laptop', width: 1366, height: 768, touch: false },
  { name: 'phone', width: 390, height: 844, touch: true },
]

/** The lessons of the course, in order (`web/src/features/learn/course.ts`). */
export const LESSONS = ['seat', 'mana', 'combat', 'stack', 'turn', 'commander', 'help'] as const

/** What the coach shows now, from its root's attributes. */
export interface CoachState {
  step: string
  kind: string
  lit: string
  press: string
  moment: string
}

const coach = (page: Page) => page.locator('[data-slot="coach"]')

/** The coach's state now, or null while it is not drawn. */
export async function coachState(page: Page): Promise<CoachState | null> {
  return page.evaluate(() => {
    const root = document.querySelector<HTMLElement>('[data-slot="coach"]')
    if (root === null) return null
    return {
      step: root.dataset['coachStep'] ?? '',
      kind: root.dataset['coachKind'] ?? '',
      lit: root.dataset['coachLit'] ?? '',
      press: root.dataset['coachPress'] ?? '',
      moment: root.dataset['coachMoment'] ?? '',
    }
  })
}

const same = (a: CoachState | null, b: CoachState | null): boolean =>
  a !== null &&
  b !== null &&
  a.step === b.step &&
  a.kind === b.kind &&
  a.lit === b.lit &&
  a.press === b.press &&
  a.moment === b.moment

/** Waits until the coach shows something other than `from`. */
async function changed(page: Page, from: CoachState, timeout = 20_000): Promise<CoachState> {
  let now: CoachState | null = from
  await expect
    .poll(
      async () => {
        now = await coachState(page)
        return !same(now, from)
      },
      { timeout, message: `the coach stays at ${JSON.stringify(from)}` },
    )
    .toBe(true)
  return now ?? from
}

/**
 * Waits until the page is still: each lit part is one drawn element, its ring lies on it, and the lit boxes, the rings
 * and the coach card are where they were a moment ago (cards that fly in, a sheet that opens, the card moving to its
 * place). The walker presses and the checks measure only then, so a loaded machine changes how long it takes, never what
 * is pressed.
 */
export async function settle(page: Page, timeout = 20_000): Promise<void> {
  let last = ''
  let before = ''
  try {
    await expect
      .poll(
        async () => {
          const now = await page.evaluate(() => {
            const root = document.querySelector<HTMLElement>('[data-slot="coach"]')
            if (root === null) return 'none'
            const selectors = JSON.parse(root.dataset['coachSelectors'] ?? '[]') as string[]
            const round = (rect: DOMRect | { left: number; top: number; width: number; height: number }) =>
              [rect.left, rect.top, rect.width, rect.height].map((value) => Math.round(value)).join(' ')
            const boxes = selectors.map((selector) => {
              const all = [...document.querySelectorAll<HTMLElement>(selector)].filter((element) => {
                const rect = element.getBoundingClientRect()
                return rect.width > 0 && rect.height > 0
              })
              const outer = all.filter(
                (element) => !all.some((other) => other !== element && other.contains(element)),
              )
              return outer.length === 1 && outer[0] !== undefined
                ? round(outer[0].getBoundingClientRect())
                : `count ${outer.length}`
            })
            const rings = [...document.querySelectorAll<HTMLElement>('[data-slot="coach-ring"]')].map(
              (ring) =>
                round({
                  left: parseFloat(ring.style.left) + 4,
                  top: parseFloat(ring.style.top) + 4,
                  width: parseFloat(ring.style.width) - 8,
                  height: parseFloat(ring.style.height) - 8,
                }),
            )
            const card = document.querySelector('[data-slot="coach-card"]')
            const lying =
              rings.length === boxes.length &&
              rings.every((ring, at) => {
                const box = boxes[at] ?? ''
                const a = ring.split(' ').map(Number)
                const b = box.split(' ').map(Number)
                return b.length === 4 && a.every((value, index) => Math.abs(value - (b[index] ?? NaN)) <= 1)
              })
            return `${lying ? 'on' : 'off'}|${boxes.join(',')}|${card === null ? '' : round(card.getBoundingClientRect())}`
          })
          const still = now === last && now === before && now.startsWith('on|')
          before = last
          last = now
          return still
        },
        { timeout, intervals: [150], message: 'the page settles under the coach' },
      )
      .toBe(true)
  } catch {
    throw new Error(
      `the page does not settle under the coach: ${last} ${(ERRORS.get(page) ?? []).join('; ')}`,
    )
  }
}

/** Opens the Learn page in a window of `layout` and starts lesson `lesson` from its row. */
const ERRORS = new WeakMap<Page, string[]>()

export async function startLesson(page: Page, layout: Layout, lesson: string): Promise<void> {
  const errors: string[] = []
  ERRORS.set(page, errors)
  page.on('pageerror', (error) => errors.push(`${error.message}\n${error.stack ?? ''}`))
  // An error the app's error boundary catches reaches only the console.
  page.on('console', (message) => {
    if (message.type() === 'error' && /The above error|Error:/.test(message.text()))
      errors.push(message.text().slice(0, 600))
  })
  await page.setViewportSize({ width: layout.width, height: layout.height })
  await page.goto(`${WEB_URL}/learn`)
  // Straight to the lesson asked for, as its row's button would: the course path offers only the next one as its primary button.
  const reply = await page.request.post(`${WEB_URL}/api/tutorial`, { data: { lesson } })
  expect(reply.status()).toBe(201)
  const seat = (await reply.json()) as { game: string; seat: number; token: string }
  await page.evaluate(([game, id]) => window.sessionStorage.setItem(`tutorial:${game}`, id), [
    seat.game,
    lesson,
  ] as const)
  await page.goto(`${WEB_URL}/game/${seat.game}#seat=${seat.seat}&token=${seat.token}`)
  await expect(coach(page)).toBeAttached({ timeout: 30_000 })
}

/** Presses the centre of the first ring with the mouse, or taps it on a touch layout. */
async function pressRing(page: Page, layout: Layout): Promise<void> {
  const box = await page.locator('[data-slot="coach-ring"]').first().boundingBox()
  expect(box, 'a do step lights a ring').not.toBeNull()
  if (box === null) return
  const x = box.x + box.width / 2
  const y = box.y + box.height / 2
  if (layout.touch) await page.touchscreen.tap(x, y)
  else await page.mouse.click(x, y)
}

/** Tries the inputs a do step holds back and requires the coach to stay and say why. */
async function probe(page: Page, state: CoachState): Promise<void> {
  const allow = await coach(page).getAttribute('data-coach-allow')
  // A look or a manual step holds back no game input of its own; pressing elsewhere only moves the page.
  if (allow === 'look' || allow === 'manual') return
  const refused = page.locator('[data-slot="coach-refused"]')
  const tries: { name: string; run: () => Promise<boolean> }[] = [
    {
      name: 'the primary button',
      run: async () => {
        const primary = page.locator('[data-coach="pass.primary"]')
        if (state.lit === 'pass.primary' || !(await primary.isVisible())) return false
        if (await primary.evaluate((button) => (button as HTMLButtonElement).disabled)) return false
        // A held button only looks off (aria-disabled): a press still reaches the gate, which says why.
        await primary.click({ force: true })
        return true
      },
    },
    {
      name: 'End turn',
      run: async () => {
        const end = page.locator('[data-coach="pass.end-turn"]')
        if (state.lit === 'pass.end-turn' || !(await end.isVisible()) || !(await end.isEnabled()))
          return false
        await end.click({ force: true })
        return true
      },
    },
    {
      name: 'Space',
      run: async () => {
        const primary = page.locator('[data-coach="pass.primary"]')
        if (state.lit === 'pass.primary' || !(await primary.isVisible())) return false
        if (await primary.evaluate((button) => (button as HTMLButtonElement).disabled)) return false
        // Space on a focused button presses that button; the probe is the table's own key.
        await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur())
        await page.keyboard.press('Space')
        return true
      },
    },
  ]
  const card = page.locator('[data-slot="coach-card"]')
  for (const each of tries) {
    const before = (await card.getAttribute('data-shake')) ?? ''
    if (!(await each.run())) continue
    // The gate answers every held input with a new refusal (the card's shake stamp): wait for it, not for a time.
    await expect
      .poll(async () => (await card.getAttribute('data-shake')) ?? '', {
        message: `${each.name} at ${state.step} says why`,
      })
      .not.toBe(before)
    await expect(refused).not.toHaveText('')
    const after = await coachState(page)
    expect(after?.step, `${each.name} at ${state.step} moves the lesson on`).toBe(state.step)
  }
}

/** What a walk does at each step besides moving on. */
export interface WalkOptions {
  /** Try the held inputs before each do step. */
  probe?: boolean
  /** Checks run at every step the coach shows. */
  check?: (page: Page, state: CoachState) => Promise<void>
  /** Saves a screenshot of every step the coach shows under this path prefix. */
  shots?: string | undefined
}

/**
 * Plays the lesson on show to its end with no per-step code: a read step presses Next, a do step presses the centre of
 * the ring, a watch step waits. The lost-place notice fails the walk. Returns the steps seen in order.
 */
export async function walkLesson(page: Page, layout: Layout, options: WalkOptions = {}): Promise<string[]> {
  const seen: string[] = []
  const probed = new Set<string>()
  const shot = new Set<string>()
  let state = await coachState(page)
  for (let moves = 0; moves < 200; moves += 1) {
    expect(ERRORS.get(page) ?? [], 'the page has no errors').toEqual([])
    if (state === null) throw new Error('the coach is gone')
    expect(state.step, 'the lesson lost its place').not.toBe('lost')
    if (seen.at(-1) !== state.step) seen.push(state.step)
    if (state.kind === 'end') {
      await expect(page.locator('[data-slot="coach-back-to-lessons"]')).toBeVisible()
      return seen
    }
    if (options.shots !== undefined && seen.at(-1) === state.step && !shot.has(state.step)) {
      shot.add(state.step)
      await settle(page)
      await page.screenshot({
        path: `${options.shots}-${String(shot.size).padStart(2, '0')}-${state.step}.png`,
      })
    }
    await options.check?.(page, state)
    if (state.kind === 'read') {
      const next = page.locator('[data-slot="coach-next"]')
      await expect(next).toBeEnabled({ timeout: 20_000 })
      await settle(page)
      if (layout.touch) await next.tap()
      else await next.click()
    } else if (state.kind === 'do') {
      if (options.probe === true && !probed.has(state.step)) {
        probed.add(state.step)
        await probe(page, state)
      }
      await settle(page)
      await pressRing(page, layout)
    }
    state = await changed(page, state)
  }
  throw new Error(`the walk did not end: ${seen.join(', ')}`)
}
