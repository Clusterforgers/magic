/**
 * A prompt-driven player. Every seat runs its own loop: wait for the next decision prompt (`[data-decision]` with a
 * new `data-decision-id`), read what the page offers, answer it with real clicks, repeat until the game-over screen.
 * Nothing sleeps: the loop waits on the page itself, and a run that stops moving fails with the last thing each seat saw.
 */
import { errors } from '@playwright/test'
import type { Locator, Page } from '@playwright/test'
import type { Player } from './lobby'

/** How a seat plays. */
export interface Style {
  /** Block the first attacker with every creature that can block (a gang block); otherwise answer "No blocks". */
  blocks: boolean
  /** Pay with "Pay automatically"; otherwise tap sources by hand until the cost is covered. */
  autoPay: boolean
  /** In a priority window with nothing to play press "End turn" (the server passes for the rest of the turn); otherwise continue. */
  yields: boolean
}

interface Card {
  id: string
  label: string
  land: boolean
}

interface Control {
  label: string
  disabled: boolean
}

/** What one decision prompt shows, read in a single step so it cannot change halfway. */
interface Seen {
  over: false
  /** The decision kind: `data-decision`. */
  kind: string
  /** The decision id: `data-decision-id`, new for every decision even when the kind repeats. */
  id: string
  /** The turn banner, such as "Turn 5 · Ada · End step": where in the game the decision is asked. */
  banner: string
  text: string
  buttons: Control[]
  /** Cards of this seat's hand the prompt marks legal. */
  hand: Card[]
  /** Cards on this seat's battlefield the prompt marks legal. */
  board: Card[]
}

type Observed = { over: true } | Seen

/**
 * Runs in the page. Gives the game-over screen, or the decision prompt unless its id is `answered`, or null while
 * there is neither. A prompt marks its cards before it is drawn (layout effects), so a read between two tasks is whole.
 */
function look(arg: { me: string; answered: string | null }): Observed | null {
  if (document.querySelector('#over-title') !== null) return { over: true }
  const root = document.querySelector<HTMLElement>('[data-decision]')
  const id = root?.dataset['decisionId']
  if (root === null || id === undefined || id === arg.answered) return null
  const legal = (scope: string): Card[] =>
    Array.from(
      document.querySelectorAll<HTMLElement>(`${scope} button[data-object-id][data-highlight="legal"]`),
    ).map((element) => {
      const label = element.getAttribute('aria-label') ?? ''
      return { id: element.dataset['objectId'] ?? '', label, land: /\bLand\b/.test(label) }
    })
  return {
    over: false,
    kind: root.dataset['decision'] ?? '',
    id,
    banner: document.querySelector('[data-slot="banner"]')?.textContent ?? '',
    text: root.querySelector('[data-slot="prompt-text"]')?.textContent ?? '',
    buttons: Array.from(root.querySelectorAll('button')).map((button) => ({
      label: (button.textContent ?? '').trim(),
      disabled: button.disabled || button.getAttribute('aria-disabled') === 'true',
    })),
    hand: legal(`ul[aria-label="${arg.me}'s hand"]`),
    board: legal(`section[aria-label="${arg.me}'s battlefield"]`),
  }
}

/** The state shared by both seats' loops: the step budget, the stall clock and the trail kept for a failure. */
export class Run {
  steps = 0
  /** Decisions answered, by kind. */
  readonly kinds = new Map<string, number>()
  /** Priority windows each seat (by name) was asked about with no land and no spell to offer; a seat with no stop and Full Control off is asked none. */
  readonly idle = new Map<string, number>()
  /** Where each seat (by name) was asked those windows, as the turn banner says it. */
  readonly idleAt = new Map<string, string[]>()
  /** Times each seat (by name) pressed "End turn" to have the server pass for it until the turn ended. */
  readonly yields = new Map<string, number>()
  readonly trail: string[] = []
  /** Set by the first loop that fails, so the other stops instead of waiting out the stall limit. */
  failure: Error | null = null
  private lastProgress = Date.now()

  readonly maxSteps: number
  readonly stallMs: number

  constructor(maxSteps: number, stallMs: number) {
    this.maxSteps = maxSteps
    this.stallMs = stallMs
  }

  /** Counts one answered decision; fails once the budget is spent. */
  step(who: string, kind: string, line: string): void {
    this.steps += 1
    this.kinds.set(kind, (this.kinds.get(kind) ?? 0) + 1)
    this.lastProgress = Date.now()
    this.trail.push(`${String(this.steps).padStart(4)} ${who} ${kind}: ${line}`)
    if (this.steps > this.maxSteps) {
      throw new Error(`No winner after ${this.maxSteps} decisions. Last steps:\n${this.tail()}`)
    }
  }

  /** Throws when the other loop failed, a page threw, or nobody has answered anything for `stallMs`. */
  check(player: Player): void {
    if (this.failure !== null) throw new Error(`stopping, the other seat failed: ${this.failure.message}`)
    const [thrown] = player.errors
    if (thrown !== undefined) throw new Error(`${player.name}'s page threw: ${thrown}`)
    if (Date.now() - this.lastProgress > this.stallMs) {
      throw new Error(`No decision was answered for ${this.stallMs} ms. Last steps:\n${this.tail()}`)
    }
  }

  /** "364 decisions (Priority 280, PayMana 30, ...), idle windows: Ada 0, Bo 31, turns ended: Bo 12". */
  summary(): string {
    const kinds = [...this.kinds].map(([kind, count]) => `${kind} ${count}`).join(', ')
    const idle = [...this.idle].map(([name, count]) => `${name} ${count}`).join(', ')
    const ended = [...this.yields].map(([name, count]) => `${name} ${count}`).join(', ')
    return `${this.steps} decisions (${kinds})${idle === '' ? '' : `, idle windows: ${idle}`}${
      ended === '' ? '' : `, turns ended: ${ended}`
    }`
  }

  private tail(): string {
    return this.trail.slice(-12).join('\n')
  }
}

/** How long one wait on the page lasts before the loop checks that the run is still healthy. */
const POLL_MS = 2_000

async function nextDecision(player: Player, answered: string | null, run: Run): Promise<Observed> {
  for (;;) {
    run.check(player)
    try {
      const handle = await player.page.waitForFunction(
        look,
        { me: player.name, answered },
        { timeout: POLL_MS },
      )
      return (await handle.jsonValue()) as Observed
    } catch (error) {
      if (!(error instanceof errors.TimeoutError)) throw error
    }
  }
}

/** The decision shown right now, or null; unlike `nextDecision` this does not wait. */
async function seeNow(player: Player): Promise<Seen | null> {
  const seen = await player.page.evaluate(look, { me: player.name, answered: null })
  return seen === null || seen.over ? null : seen
}

interface Turn {
  player: Player
  style: Style
  seen: Seen
  run: Run
}

const named = (card: Card): string => card.label.split(',')[0] ?? card.label
const prompt = (page: Page): Locator => page.locator('[data-decision]')
const button = (page: Page, name: string): Locator => prompt(page).getByRole('button', { name, exact: true })
/** The Priority prompt's primary button: its words depend on the moment ("Next: Combat", "Continue"...), its slot does not. */
const primary = (page: Page): Locator => prompt(page).locator('[data-slot="primary-action"]')
/** "End turn" beside the primary button, which is not drawn in the last window of one's own turn. */
const endTurn = (page: Page): Locator => prompt(page).locator('[data-slot="secondary-action"]')
const card = (page: Page, id: string): Locator => page.locator(`button[data-object-id="${id}"]`)
const enabled = (seen: Seen, label: string): boolean =>
  seen.buttons.some((control) => control.label === label && !control.disabled)

/** Each decision kind's answer; the string says what was done, for the trail. */
const ANSWERS: Record<string, (turn: Turn) => Promise<string>> = {
  async Mulligan({ player }) {
    await button(player.page, 'Keep').click()
    return 'keeps the opening hand'
  },

  async Priority({ player, style, seen, run }) {
    // Lands first, so the mana is there for the creature that follows; with nothing to play, go on.
    const play = seen.hand.find((candidate) => candidate.land) ?? seen.hand[0]
    if (play === undefined) {
      if (style.yields && (await endTurn(player.page).count()) > 0) {
        await endTurn(player.page).click()
        run.yields.set(player.name, (run.yields.get(player.name) ?? 0) + 1)
        return 'ends the turn'
      }
      await primary(player.page).click()
      return 'continues'
    }
    await card(player.page, play.id).click()
    return `${play.land ? 'plays' : 'casts'} ${named(play)}`
  },

  async PayMana({ player, style, seen }) {
    const { page } = player
    if (style.autoPay && enabled(seen, 'Pay automatically')) {
      await button(page, 'Pay automatically').click()
      return 'pays automatically'
    }
    // By hand: tap the first source the prompt offers until "Confirm payment" lights up.
    let now: Seen | null = seen
    const tapped: string[] = []
    while (now !== null && now.id === seen.id) {
      if (enabled(now, 'Confirm payment')) {
        await button(page, 'Confirm payment').click()
        return `pays by hand with ${tapped.join(', ')}`
      }
      const source = now.board[0]
      if (source === undefined) break
      await card(page, source.id).click()
      await card(page, source.id).and(page.locator('[aria-pressed="true"]')).waitFor()
      tapped.push(named(source))
      now = await seeNow(player)
    }
    // After some picks the prompt offers to pay the rest instead of paying everything automatically.
    const rest =
      now !== null && enabled(now, 'Pay the rest for me') ? 'Pay the rest for me' : 'Pay automatically'
    if (now !== null && enabled(now, rest)) {
      await button(page, rest).click()
      return 'pays automatically (nothing left to tap by hand)'
    }
    await button(page, 'Cancel casting').click()
    return 'cancels casting, the cost could not be met'
  },

  async DeclareAttackers({ player, seen }) {
    if (!enabled(seen, 'Attack with all')) {
      await button(player.page, 'No attack').click()
      return 'has nothing to attack with'
    }
    await button(player.page, 'Attack with all').click()
    await button(player.page, 'Confirm attack').click()
    return 'attacks with everything'
  },

  async DeclareBlockers({ player, style, seen }) {
    if (!style.blocks || seen.board.length === 0) {
      await button(player.page, 'No blocks').click()
      return 'does not block'
    }
    // Every blocker goes on the first attacker; two or more on one attacker make its controller split the damage.
    for (const blocker of seen.board) {
      await card(player.page, blocker.id).click()
      // The picker lists one button per attacker this creature may block, then Cancel.
      await player.page.locator('[data-slot="attacker-picker"]').getByRole('button').first().click()
    }
    await button(player.page, 'Confirm blocks').click()
    return `blocks with ${seen.board.map(named).join(', ')}`
  },

  async AssignDamage({ player }) {
    // The prompt starts from a legal split (lethal damage in order, the rest to the last blocker).
    await button(player.page, 'Confirm').click()
    return 'confirms the suggested damage split'
  },

  Discard: chooseCards,
  BottomCards: chooseCards,
}

/** Discard and BottomCards: toggle as many offered cards as the prompt counts ("0 of 2 chosen"), then Confirm. */
async function chooseCards({ player, seen }: Turn): Promise<string> {
  const need = Number(/(\d+) of (\d+) chosen/.exec(seen.text)?.[2] ?? 0)
  const chosen = seen.hand.slice(0, need)
  if (chosen.length < need)
    throw new Error(`${seen.kind} wants ${need} cards, the hand offers ${chosen.length}`)
  for (const pick of chosen) await card(player.page, pick.id).click()
  await button(player.page, 'Confirm').click()
  return `${seen.kind === 'Discard' ? 'discards' : 'bottoms'} ${chosen.map(named).join(', ') || 'nothing'}`
}

/** Answers this seat's decisions until the game-over screen shows. */
async function drive(player: Player, style: Style, run: Run): Promise<void> {
  let answered: string | null = null
  for (;;) {
    const seen = await nextDecision(player, answered, run)
    if (seen.over) return
    // A priority window that offers no land and no spell can be passed and nothing else; only a stop or Full Control asks for it.
    if (seen.kind === 'Priority' && seen.hand.length === 0) {
      run.idle.set(player.name, (run.idle.get(player.name) ?? 0) + 1)
      run.idleAt.set(player.name, [...(run.idleAt.get(player.name) ?? []), seen.banner])
    }
    const answer = ANSWERS[seen.kind]
    if (answer === undefined)
      throw new Error(`${player.name} faces ${seen.kind}, which the driver cannot answer`)
    run.step(player.name, seen.kind, `#${seen.id} ${await answer({ player, style, seen, run })}`)
    answered = seen.id
  }
}

/** Plays both seats to the game-over screen at once; rejects with the trail if a seat gets stuck or the budget runs out. */
export async function playToTheEnd(
  seats: readonly { player: Player; style: Style }[],
  run: Run,
): Promise<void> {
  await Promise.all(
    seats.map(async ({ player, style }) => {
      try {
        await drive(player, style, run)
      } catch (error) {
        const failure = error instanceof Error ? error : new Error(String(error))
        run.failure ??= failure
        throw failure
      }
    }),
  )
}
