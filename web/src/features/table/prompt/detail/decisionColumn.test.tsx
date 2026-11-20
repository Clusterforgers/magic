// Goal: in every game mode and window a card's question has one home, under the asking card in the Stack panel of a column, in the dock on a rail and in the bar on a phone, and the strip asks nothing; the panel holds the asking card with the asked clause marked, the run it is one of with what was answered, the triggers to order as rows that move or are pressed in order, a payment as circles captioned with their sources over rows grouped by what they make (on a phone in the sheet the bar grows into); the column gives its tab back afterwards, and a source the view does not hold is named and not pointed at.
import { act, fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import type { Answer } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { ASK_PROMPTS } from '../../dev/askScene'
import { fixtureGame } from '../../dev/devGame'
import type { FixtureGameOptions } from '../../dev/devGame'
import { GameTable } from '../../GameTable'
import { DETAIL_HOLD_MS } from '../../sideState'
import { TableOptionsContext } from '../../tableOptions'
import { forgetOutcomes } from './runMemory'

type Viewport = { width: number; height: number }
const DESKTOP: Viewport = { width: 1400, height: 900 }
const LAPTOP: Viewport = { width: 1280, height: 720 }
const RAIL: Viewport = { width: 1100, height: 800 }
const PHONE: Viewport = { width: 390, height: 844 }

/** The game modes, as the dev table builds them. */
const MODES: Array<[string, Partial<FixtureGameOptions>]> = [
  ['a duel', { seats: 2, board: 'duel' }],
  ['a grid of four', { seats: 4, permanents: 16 }],
  ['teams', { seats: 4, permanents: 16, teams: 'teams' }],
  ['Two-Headed Giant', { seats: 4, permanents: 16, teams: 'two_headed_giant' }],
  ['a boss fight', { seats: 4, permanents: 16, boss: 1 }],
]
const WINDOWS: Array<[string, Viewport, 'panel' | 'dock' | 'bar']> = [
  ['1400x900', DESKTOP, 'panel'],
  ['1280x720', LAPTOP, 'panel'],
  ['a rail at 1100', RAIL, 'dock'],
  ['a phone', PHONE, 'bar'],
]

/** The table of the dev prompt `prompt`, every answer going to a spy that accepts it and clears the decision. */
function owe(prompt: string, options: Partial<FixtureGameOptions> = {}, viewport: Viewport = DESKTOP) {
  const game = fixtureGame({ seats: 2, board: 'duel', ...options, prompt })
  const store = game.session.store
  const answer = vi.fn((_answer: Answer) => {
    store.setState({ pending: null })
    return true
  })
  const view = render(
    <GameContext value={{ ...game, session: { ...game.session, answer } }}>
      <TableOptionsContext value={{ viewport }}>
        <GameTable />
      </TableOptionsContext>
    </GameContext>,
  )
  const one = (selector: string): HTMLElement => must(view.container.querySelector<HTMLElement>(selector))
  const all = (selector: string): HTMLElement[] => [...view.container.querySelectorAll<HTMLElement>(selector)]
  return { ...view, store, answer, one, all, user: userEvent.setup() }
}

/** Says the main pointer is a finger for the test's length. */
function coarse(): void {
  vi.stubGlobal(
    'matchMedia',
    (query: string): MediaQueryList =>
      ({
        matches: query === '(pointer: coarse)',
        media: query,
        onchange: null,
        addEventListener: () => undefined,
        removeEventListener: () => undefined,
        addListener: () => undefined,
        removeListener: () => undefined,
        dispatchEvent: () => false,
      }) as MediaQueryList,
  )
}

beforeEach(() => forgetOutcomes())
afterEach(() => vi.unstubAllGlobals())

describe('a "you may" in every mode and window', () => {
  describe.each(MODES)('%s', (_mode, options) => {
    it.each(WINDOWS)('at %s', (_window, viewport, home) => {
      const table = owe('ask-may', options, viewport)
      // One home holds the question, Yes and No; the strip between the boards asks nothing.
      const [line, ...others] = table.all('[data-slot="decision-line"]')
      expect(others).toHaveLength(0)
      expect(line).toHaveAttribute('data-home', home)
      expect(table.one('[data-slot="prompt-text"]')).toHaveTextContent('Pay {1}?')
      const actions = table.one('[data-slot="prompt-actions"]')
      expect(
        within(actions)
          .getAllByRole('button')
          .map((button) => button.getAttribute('aria-label')),
      ).toEqual(['No', 'Yes'])
      expect(table.one('[data-region="shared"]').querySelector('[data-slot="prompt-actions"]')).toBeNull()
      expect(table.all('[data-slot="decision-sheet"]')).toHaveLength(0)
      // Off the panel the asking card's chip leads the question; in the panel the card stands above it.
      if (home !== 'panel') {
        const chip = within(must(line)).getByRole('button', { name: /^Mentor of the Meek asks/ })
        expect(chip).toHaveTextContent('Mentor of the Meek')
        expect(table.all('[data-slot="decision-detail"]')).toHaveLength(0)
        return
      }
      const [detail, ...more] = table.all('[data-slot="decision-detail"]')
      expect(more).toHaveLength(0)
      expect(detail).toHaveAttribute('data-where', 'column')
      expect(must(detail).contains(must(line))).toBe(true)
      const entry = within(must(detail))
      expect(entry.getByText('Resolving now')).toBeInTheDocument()
      expect(entry.getByText('Asking you')).toBeInTheDocument()
      expect(must(detail).querySelector('[data-slot="asked-clause"]')).toHaveTextContent(
        'you may pay {1}. If you do, draw a card.',
      )
      expect(must(detail).querySelector('[data-slot="detail-rules"]')).toHaveTextContent(
        /^Whenever another creature you control with power 2 or less enters, you may pay/,
      )
      expect(entry.getByText('Nothing else is on the stack.')).toBeInTheDocument()
    })
  })
})

describe('the Stack column', () => {
  beforeEach(() => vi.useFakeTimers({ shouldAdvanceTime: true }))
  afterEach(() => vi.useRealTimers())

  const selected = (): string | null =>
    screen.getAllByRole('tab').find((tab) => tab.getAttribute('aria-selected') === 'true')?.textContent ??
    null

  it('comes forward for a decision and gives the tab back after it', () => {
    const table = owe('ask-may')
    expect(selected()).toMatch(/^Stack/)
    act(() => table.store.setState({ pending: null }))
    // The next question of a run arrives within the hold: the column stays.
    act(() => void vi.advanceTimersByTime(DETAIL_HOLD_MS - 50))
    expect(selected()).toMatch(/^Stack/)
    act(() => void vi.advanceTimersByTime(100))
    expect(selected()).toMatch(/^Log/)
  })

  it('keeps a tab the viewer chose meanwhile', async () => {
    const table = owe('ask-may')
    await table.user.click(screen.getByRole('tab', { name: /^Chat/ }))
    act(() => table.store.setState({ pending: null }))
    act(() => void vi.advanceTimersByTime(DETAIL_HOLD_MS + 50))
    expect(selected()).toMatch(/^Chat/)
  })

  it('stays as it is for priority', () => {
    owe('priority')
    expect(selected()).toMatch(/^Log/)
    expect(document.querySelector('[data-slot="decision-detail"]')).toBeNull()
  })

  it('gives a card`s question to the dock while another tab is on show, and takes it back on the Stack tab', async () => {
    const table = owe('ask-may')
    expect(table.one('[data-slot="decision-line"]')).toHaveAttribute('data-home', 'panel')
    await table.user.click(screen.getByRole('tab', { name: /^Log/ }))
    expect(table.one('[data-slot="decision-line"]')).toHaveAttribute('data-home', 'dock')
    expect(table.one('[data-slot="decision-dock"]').contains(table.one('[data-slot="decision-line"]'))).toBe(
      true,
    )
    await table.user.click(screen.getByRole('tab', { name: /^Stack/ }))
    expect(table.one('[data-slot="decision-line"]')).toHaveAttribute('data-home', 'panel')
    expect(table.all('[data-slot="answer-jump"]')).toHaveLength(1)
  })
})

describe('a run of triggers', () => {
  it.each(WINDOWS)('names its place, what is done and what comes at %s', (_window, viewport, home) => {
    const table = owe('ask-chain', {}, viewport)
    expect(table.one('[data-slot="run-dots"]')).toHaveAccessibleName('Trigger 2 of 3')
    if (home !== 'panel') return
    const detail = table.one('[data-slot="decision-detail"]')
    expect(detail.querySelector('[data-slot="detail-kicker"]')).toHaveTextContent(
      'Brago, King Eternal · trigger 2 of 3',
    )
    const rows = [...detail.querySelectorAll<HTMLElement>('[data-slot="run"] > li')]
    expect(rows.map((row) => row.textContent)).toEqual([
      expect.stringMatching(/✓.*Soul's Attendant.*Done/),
      expect.stringMatching(/Asking · 2 of 3.*Suture Priest/),
      expect.stringMatching(/3.*Mentor of the Meek.*Next · You may pay \{1\}\. If you do, draw a card\./),
    ])
    expect(rows[0]).toHaveAttribute('data-done')
    // The triggers of the run are not listed again under it.
    expect(within(detail).getByText('Nothing else is on the stack.')).toBeInTheDocument()
  })

  it('remembers what was answered for the next question', async () => {
    const table = owe('ask-chain-first')
    await table.user.keyboard('y')
    expect(table.answer).toHaveBeenCalledWith({ YesNo: true })
    const fixture = { view: must(table.store.getState().view), log: [], chat: [], names: [] }
    act(() => table.store.setState({ pending: must(ASK_PROMPTS['ask-chain'])(fixture) }))
    const [done] = table.all('[data-slot="run-row"]')
    expect(done).toHaveTextContent('Yes · Gain 1 life')
  })

  it('offers the rule for the rest of the game under the asking card, off the band', () => {
    const table = owe('ask-may')
    const always = table.one('[data-slot="always-answer"]')
    expect(always.closest('[data-slot="decision-detail"]')).not.toBeNull()
    expect(always.closest('[data-slot="prompt-actions"]')).toBeNull()
  })
})

describe('ordering triggers', () => {
  const names = (table: ReturnType<typeof owe>): string[] =>
    table.all('[data-slot="trigger-row"]').map((row) => row.querySelector('.truncate')?.textContent ?? '')

  it('lists them as rows with their effect, the first tagged, and moves and sends them', async () => {
    const table = owe('ask-order')
    expect(names(table)).toEqual([
      'Suture Priest',
      "Soul's Attendant",
      'Mentor of the Meek',
      'Welcoming Vampire',
    ])
    const rows = table.all('[data-slot="trigger-row"]')
    expect(rows[0]).toHaveTextContent('first')
    expect(rows[1]).not.toHaveTextContent('first')
    expect(rows[3]).toHaveTextContent('Draw a card.')
    expect(rows[3]).not.toHaveTextContent('Whenever')
    expect(table.one('[data-slot="decision-detail"]')).toHaveTextContent('Below them on the stack')
    const band = table.one('[data-slot="decision-line"]')
    expect(table.one('[data-slot="decision-detail"]')).toHaveTextContent('Order 4 triggers')
    await table.user.click(screen.getByRole('button', { name: /^Move Welcoming Vampire.* up$/ }))
    await table.user.click(screen.getByRole('button', { name: /^Move Welcoming Vampire.* up$/ }))
    expect(names(table)[1]).toBe('Welcoming Vampire')
    // The sources carry their numbers on the board, and what entered its badge.
    const marks = table.one('[data-slot="ask-marks"]').textContent ?? ''
    expect(marks).toContain("content:'2'")
    expect(marks).toContain("content:'just entered'")
    await table.user.click(within(band).getByRole('button', { name: 'Confirm order' }))
    // The list is the order they resolve; the answer the order they go on the stack.
    expect(table.answer).toHaveBeenCalledWith({ OrderTriggers: [1, 2, 0, 3] })
  })

  it('drops a dragged row in its new place', () => {
    const table = owe('ask-order')
    const rows = table.all('[data-slot="trigger-row"]')
    const data = { effectAllowed: '', setData: () => undefined }
    fireEvent.dragStart(must(rows[3]), { dataTransfer: data })
    fireEvent.dragOver(must(rows[0]), { dataTransfer: data })
    fireEvent.drop(must(rows[0]), { dataTransfer: data })
    expect(names(table)[0]).toBe('Welcoming Vampire')
  })

  it('on a touch screen takes them in the order pressed, the last by itself', async () => {
    coarse()
    const table = owe('ask-order', {}, PHONE)
    const band = table.one('[data-slot="decision-line"]')
    const confirm = within(band).getByRole('button', { name: 'Confirm order' })
    expect(confirm).toBeDisabled()
    expect(table.one('[data-slot="trigger-order"]')).toHaveAttribute('data-mode', 'press')
    const list = within(table.one('[data-slot="trigger-order"]'))
    const press = (name: RegExp) => table.user.click(list.getByRole('button', { name }))
    await press(/^Welcoming Vampire/)
    await press(/^Mentor of the Meek/)
    expect(confirm).toBeDisabled()
    await press(/^Soul's Attendant/)
    // Three pressed: the fourth has one place left.
    const numbers = table.all('[data-slot="trigger-row"] .detail-number').map((n) => n.textContent)
    expect(numbers).toEqual(['1', '2', '3', '4'])
    expect(confirm).toBeEnabled()
    await table.user.click(within(band).getByRole('button', { name: 'Start over' }))
    expect(table.all('[data-slot="trigger-row"][data-picked]')).toHaveLength(0)
  })
})

describe('paying', () => {
  const captions = (table: ReturnType<typeof owe>): string[] =>
    table.all('[data-slot="pay-caption"]').map((caption) => caption.textContent ?? '')

  it.each(WINDOWS.filter(([, , home]) => home !== 'dock'))(
    'draws the circles, the groups and the meter at %s',
    (_window, viewport) => {
      const table = owe('ask-pay', {}, viewport)
      expect(captions(table)).toEqual(['any', 'any', 'white', 'blue'])
      expect(
        table.all('[data-slot="pay-group-label"]').map((label) => label.getAttribute('aria-label')),
      ).toEqual([
        'Makes white',
        'Makes blue',
        'Makes white or blue, you choose',
        'Makes white and blue, costs 1 generic',
      ])
      const band = table.one('[data-slot="decision-line"]')
      expect(within(band).getByRole('img', { name: '0 of 4 paid' })).toBeInTheDocument()
      expect(within(band).getByRole('button', { name: 'Confirm payment' })).toBeDisabled()
      // The sentence is read aloud and not drawn: the meter says it.
      expect(table.one('[data-slot="prompt-sentence"]')).toHaveClass('sr-only')
    },
  )

  it('captions each circle with the source that paid it, a Signet`s own cost after the divider', async () => {
    const table = owe('ask-pay')
    const tap = (name: RegExp) => table.user.click(screen.getByRole('button', { name }))
    // A Signet's own cost is paid from mana made before it.
    expect(screen.getByRole('button', { name: /^Tap Azorius Signet/ })).toHaveAttribute(
      'aria-disabled',
      'true',
    )
    expect(table.one('[data-slot="row-why"]')).toHaveTextContent('Tap another source for its cost first')
    await tap(/^Tap Plains for white/)
    expect(captions(table)).toEqual(['any', 'any', 'Plains', 'blue'])
    await tap(/^Tap Hallowed Fountain for blue/)
    await tap(/^Tap Azorius Signet for white and blue/)
    expect(captions(table)).toEqual([
      'Azorius Signet',
      'Azorius Signet',
      'Plains',
      'Hallowed Fountain',
      'for Azorius Signet',
    ])
    expect(table.one('[data-slot="pay-slot"][data-extra]')).toHaveTextContent('for Azorius Signet')
    await tap(/^Tap Island for blue/)
    expect(captions(table).at(-1)).toBe('Island')
    const band = table.one('[data-slot="decision-line"]')
    expect(within(band).getByRole('img', { name: '5 of 5 paid' })).toBeInTheDocument()
    await table.user.click(within(band).getByRole('button', { name: 'Confirm payment' }))
    expect(table.answer).toHaveBeenCalledTimes(1)
    const sent = must(table.answer.mock.calls[0])[0] as { PayMana: { Manual: { taps: unknown[] } } }
    expect(sent.PayMana.Manual.taps).toHaveLength(4)
  })

  it('untaps a used row, takes a circle back and undoes the last', async () => {
    const table = owe('ask-pay')
    await table.user.click(screen.getByRole('button', { name: /^Tap Plains for white/ }))
    await table.user.click(screen.getByRole('button', { name: /^Tap Island for blue/ }))
    expect(table.one('[data-slot="pay-source-row"][data-used]')).toHaveTextContent('paying')
    await table.user.click(screen.getByRole('button', { name: 'Untap Plains' }))
    expect(captions(table)).toEqual(['any', 'any', 'white', 'Island'])
    await table.user.click(screen.getByRole('button', { name: /^Remove blue from Island/ }))
    expect(captions(table)).toEqual(['any', 'any', 'white', 'blue'])
    await table.user.click(screen.getByRole('button', { name: /^Tap Plains for white/ }))
    await table.user.click(screen.getByRole('button', { name: 'Undo the last choice' }))
    expect(captions(table)).toEqual(['any', 'any', 'white', 'blue'])
  })

  it('lights the row of a land of two colours pressed on the board, which its row then answers', async () => {
    const table = owe('ask-pay')
    const fountain = must(
      table
        .all('button[data-object-id][data-size]')
        .find((card) => /^Hallowed Fountain/.test(card.getAttribute('aria-label') ?? '')),
    )
    await table.user.click(fountain)
    expect(table.one('[data-slot="pay-source-row"][data-lit]')).toHaveTextContent('Hallowed Fountain')
    await table.user.click(screen.getByRole('button', { name: 'Add white mana' }))
    expect(captions(table)).toContain('Hallowed Fountain')
    expect(table.all('[data-slot="pay-source-row"][data-lit]')).toHaveLength(0)
  })

  it('lets the game pay', async () => {
    const table = owe('ask-pay')
    await table.user.click(screen.getByRole('button', { name: 'Pay for me' }))
    expect(table.answer).toHaveBeenCalledWith({ PayMana: 'Auto' })
  })
})

describe("a phone's payment sheet", () => {
  it('grows the bar into a sheet for a payment, named, that folds by its handle', async () => {
    const table = owe('ask-pay', {}, PHONE)
    const handle = table.one('[data-slot="sheet-grab"]')
    expect(handle).toHaveAttribute('aria-expanded', 'true')
    expect(handle).toHaveTextContent('Stack · Pay for Brago, King Eternal')
    expect(table.one('[data-slot="decision-sheet"]').querySelector('[data-slot="pay-slot"]')).not.toBeNull()
    await table.user.click(handle)
    expect(handle).toHaveAttribute('aria-expanded', 'false')
    expect(table.one('[data-slot="decision-detail"]')).not.toBeVisible()
  })

  it.each(['ask-may', 'targets', 'ask-order', 'x'])('is not there for %s: the bar answers it', (prompt) => {
    const table = owe(prompt, {}, PHONE)
    expect(table.all('[data-slot="decision-sheet"]')).toHaveLength(0)
    expect(table.one('[data-slot="decision-line"]')).toHaveAttribute('data-home', 'bar')
  })

  it('holds short choices in the home: the bar on a phone, the panel on a desktop', () => {
    const phone = owe('x', {}, PHONE)
    expect(phone.one('[data-slot="prompt-choices"]').closest('[data-home="bar"]')).not.toBeNull()
    phone.unmount()
    const desk = owe('x')
    expect(desk.one('[data-slot="prompt-choices"]').closest('[data-home="panel"]')).not.toBeNull()
  })
})

describe('what the table lights', () => {
  it('glows the asking card and badges what entered', () => {
    const table = owe('ask-may', {}, RAIL)
    const chip = table.one('[data-slot="prompt-asker"]')
    const marks = table.one('[data-slot="ask-marks"]').textContent ?? ''
    expect(marks).toContain(`button[data-object-id='${chip.getAttribute('data-asker')}'][data-size]`)
    expect(marks).toContain("content:'asking'")
    expect(marks).toContain("content:'just entered'")
    expect(marks).not.toContain('animation')
    fireEvent.pointerEnter(chip)
    const lit = table.one('[data-slot="ask-marks"]').textContent ?? ''
    // The pulse of the card pointed at respects reduced motion.
    expect(lit).toContain('@media (prefers-reduced-motion:no-preference)')
    fireEvent.pointerLeave(chip)
    expect(table.one('[data-slot="ask-marks"]').textContent).toBe(marks)
  })

  it('names a source the view no longer holds and points at nothing', () => {
    const table = owe('ask-departed', {}, RAIL)
    const chip = table.one('[data-slot="prompt-asker"]')
    expect(chip).toHaveTextContent('Mentor of the Meek')
    const marks = table.one('[data-slot="ask-marks"]').textContent ?? ''
    expect(marks).not.toContain("content:'asking'")
    table.unmount()
    const panel = owe('ask-departed')
    expect(panel.one('[data-slot="ask-entry"]')).toHaveTextContent('Mentor of the Meek')
  })
})
