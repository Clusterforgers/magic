// Goal: a phone held upright keeps its table to the essentials: the combat answers keep their words, the controls dock shows the auto-skip switch without its label (the switch keeps its name), the pod drops the untapped sources the lands already show, the hand dock is 104 to 140 px, the boards fold their lands into chips and draw no card under the phone floor, the strip is one line, the decision stands in the bar under the hand, and a row of choice cards never widens its box past the screen. The bar loses no line: priority's Stack chip stands beside a take-back and the stops, the Stops button beside a combat declaration's sentence and its total at the end of the last answers, and a phone on its side lays choice cards out at a phone's sizes.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { act, render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import { fixtureGame } from './dev/devGame'
import { GameTable } from './GameTable'
import { TableOptionsContext } from './tableOptions'
import { renderTable } from '@/test-utils/tableHarness'
import { HAND_PHONE_CLAMP } from './layout'
import { must } from '@/test-utils/batches'

const read = (path: string): string => readFileSync(join(import.meta.dirname, path), 'utf8')

describe('the phone table', () => {
  it('keeps the word of every combat answer on a phone: Attack and No attack, Block and No blocks', () => {
    for (const [file, labels] of [
      ['prompt/kinds/DeclareAttackers.tsx', ['Confirm attack', 'No attack']],
      ['prompt/kinds/DeclareBlockers.tsx', ['Confirm blocks', 'No blocks']],
    ] as const) {
      const source = read(file)
      for (const label of labels) {
        const tag = must(source.split('<BarButton').find((part) => part.includes(`label="${label}"`)))
        expect(tag.slice(0, tag.indexOf('/>'))).toMatch(/\bkeepWord\b/)
      }
    }
  })

  it('names the auto-skip switch for a screen reader and hides its label on a phone', () => {
    const toggle = read('stops/AutoSkipToggle.tsx')
    expect(toggle).toContain("cn('max-sm:sr-only'")
    expect(toggle).toMatch(/aria-label="Auto-skip triggers"/)
  })

  it('drops the untapped sources from a one-line pod on a phone only', () => {
    const { container } = renderTable({ seats: 2, options: { viewport: { width: 390, height: 844 } } })
    const sources = [...container.querySelectorAll('[data-variant="line"] [data-slot="mana-sources"]')]
    expect(sources.length).toBeGreaterThan(0)
    for (const element of sources) expect(element).toHaveClass('max-sm:hidden')
  })

  it('gives the hand dock 104 to 140 px', () => {
    expect(HAND_PHONE_CLAMP).toEqual({ min: 104, vh: 16, max: 140 })
  })

  it('folds the lands of the focused board and of yours into chips, so the boards keep their room for the rest', () => {
    const { container } = renderTable({
      seats: 4,
      options: { viewport: { width: 390, height: 844 }, foldLands: true },
    })
    expect(container.querySelector('[data-slot="board-lands"] [data-slot="land-summary"]')).not.toBeNull()
    const own = must(container.querySelector('[data-seat-area="0"]'))
    expect(own.querySelector('[data-slot="land-summary"]')).not.toBeNull()
    expect(container.querySelector('[data-lane="lands"]')).toBeNull()
  })

  it('keeps the strip one line and answers in the bar under the hand', () => {
    expect(read('layout.ts')).toContain('export const STRIP_PHONE_PX = 28')
    expect(read('ControlsDock.tsx')).toContain("data-slot={phone ? 'decision-bar' : 'decision-dock'}")
    expect(read('layout.ts')).toContain('"hand hand" "controls controls"')
  })

  it('keeps a row of choice cards inside the sheet', () => {
    expect(read('prompt/CardChoices.tsx')).toContain('max-w-full min-w-0 basis-full items-center')
  })
})

describe("the phone's bar loses no line", () => {
  /** The phone's table at the dev prompt `prompt` in a window of `viewport`. */
  function phone(prompt: string, viewport = { width: 390, height: 844 }) {
    const game = fixtureGame({ seats: 4, prompt })
    const view = render(
      <GameContext value={game}>
        <TableOptionsContext value={{ viewport }}>
          <GameTable />
        </TableOptionsContext>
      </GameContext>,
    )
    const one = (selector: string): HTMLElement => must(view.container.querySelector<HTMLElement>(selector))
    return { game, one }
  }

  it('sets the Stack chip of priority on the line of the take-back and the stops, the take-back giving way', () => {
    const { game, one } = phone('priority')
    act(() =>
      game.session.store.setState({
        takeBackOffer: { player: 0, kind: 'Cast', name: 'Lightning Bolt', at: 0 },
      }),
    )
    const line = one('[data-slot="dock-upper"]')
    expect(line).toContainElement(one('[data-slot="bar-stack"]'))
    expect(line).toContainElement(one('[data-slot="stops-button"]'))
    expect(one('[data-slot="take-back-slot"]')).toHaveClass('min-w-0')
    expect(one('[data-slot="take-back"] button')).toHaveTextContent('Take back casting Lightning Bolt')
    expect(
      one('[data-slot="decision-bar"] [data-slot="decision-line"]').querySelector('[data-slot="bar-stack"]'),
    ).toBeNull()
  })

  it.each(['attack', 'block'])(
    'stands the Stops button beside the sentence while %s is declared',
    (prompt) => {
      const { one } = phone(prompt)
      const line = one('[data-slot="decision-bar"] [data-slot="combat-bar"]')
      expect(line).toContainElement(one('[data-slot="home-stops"]'))
      // The sentence, then the Stops button, then the answers: Stops shares the sentence's line.
      expect([...line.children].map((child) => child.getAttribute('data-slot'))).toEqual([
        'prompt-text',
        'home-stops',
        'prompt-actions',
      ])
    },
  )

  it.each(['attack', 'block'])(
    'sets the combat total at the end of the last answers while %s is declared, labelled and drawn',
    (prompt) => {
      const { one } = phone(prompt)
      const meter = one(
        '[data-slot="decision-bar"] [data-slot="prompt-actions"] > [data-slot="combat-meter"]',
      )
      expect(meter).toHaveClass('order-last', 'min-w-0')
      expect(meter).not.toHaveClass('basis-full')
      const total = one('[data-slot="forecast-total"]')
      expect(meter).toContainElement(total)
      expect(total).toHaveAttribute('role', 'group')
      expect(total).toHaveAttribute('aria-label', 'Combat outcome')
    },
  )

  it('lays the choice cards of a phone on its side out at a phone`s sizes', () => {
    const { one } = phone('targets', { width: 844, height: 390 })
    expect(one('[data-slot="choice-overlay"]')).toHaveClass('max-h-full', 'max-w-full')
  })
})
