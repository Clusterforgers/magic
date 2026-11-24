// Goal: a card choice in the overlay whose cards stand on the table offers Pick on board (V), which hides the overlay and its scrim while the home keeps the question, the count, the answers and Show choices, lets the board's marked cards choose and unchoose, keeps the table's grid as it was and brings the cards back with Show choices; a choice with no card on the table offers none; twins (one name, one controller, one zone) carry the same number on their pictures and on the table, a screen reader tells them apart, and a picture under the pointer rings its object on the table.
import { act, fireEvent, render, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import type { ObjectId, ObjectView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import type { Viewport } from '../layout'
import { GameTable } from '../GameTable'
import { TableOptionsContext } from '../tableOptions'
import { fixtureGame } from '../dev/devGame'
import { shownTwins, twinFacts, twinNumbers, twinStyle } from './twins'

const PHONE: Viewport = { width: 390, height: 844 }
const LANDSCAPE: Viewport = { width: 844, height: 390 }
const WIDE: Viewport = { width: 1400, height: 900 }

function table(prompt: string, viewport: Viewport, seats = 2) {
  const game = fixtureGame({ seats, prompt })
  const view = render(
    <GameContext value={game}>
      <TableOptionsContext value={{ viewport }}>
        <GameTable />
      </TableOptionsContext>
    </GameContext>,
  )
  const main = must(view.container.querySelector<HTMLElement>('[data-slot="table-main"]'))
  const host = must(view.container.querySelector<HTMLElement>('[data-slot="choice-host"]'))
  const overlay = (): HTMLElement => must(host.querySelector<HTMLElement>('[data-slot="choice-overlay"]'))
  const toggle = (): HTMLElement =>
    must(view.container.querySelector<HTMLElement>('[data-slot="pick-on-board"]'))
  const home = (): HTMLElement =>
    must(view.container.querySelector<HTMLElement>('[data-slot="decision-line"]'))
  const objects = Object.values(game.session.store.getState().objectsById)
  // The first opponent's twins of the scene, in the order the table lists them.
  const twins = objects.filter(
    (object) => object.controller === 1 && object.zone === 'Battlefield' && object.types.includes('Creature'),
  )
  const name = twins[0]?.name ?? ''
  const pair = twins.filter((object) => object.name === name).map((object) => object.id)
  const board = (id: ObjectId): HTMLElement =>
    must(
      view.container.querySelector<HTMLElement>(
        `:is([data-area="opponents"], [data-area="own"]) button[data-object-id="${id}"]`,
      ),
    )
  const picture = (id: ObjectId): HTMLElement =>
    must(host.querySelector<HTMLElement>(`button[data-candidate="${id}"]`))
  return { ...view, game, main, host, overlay, home, toggle, pair, board, picture }
}

describe.each([
  ['a desktop window', WIDE],
  ['a phone', PHONE],
  ['a phone on its side', LANDSCAPE],
])('Pick on board in %s', (_, viewport) => {
  it('hides the overlay, keeps the grid and lets the board choose and unchoose', async () => {
    const { main, host, overlay, home, toggle, pair, board, picture } = table('targets-twins', viewport)
    const user = userEvent.setup()
    const second = must(pair[1])
    expect(toggle()).toHaveTextContent('Pick on board')
    expect(toggle()).toHaveAttribute('aria-keyshortcuts', 'V')
    expect(toggle()).toHaveAttribute('aria-pressed', 'false')
    const areas = main.style.gridTemplateAreas
    await user.click(toggle())
    expect(overlay()).toHaveAttribute('data-peek')
    expect(overlay()).not.toBeVisible()
    const text = (): HTMLElement => must(home().querySelector<HTMLElement>('[data-slot="prompt-text"]'))
    expect(text()).toHaveTextContent(/choose one or two target creatures/i)
    const answers = must(home().querySelector<HTMLElement>('[data-slot="prompt-actions"]'))
    expect(answers).toContainElement(toggle())
    expect(within(answers).getByRole('button', { name: 'Confirm targets' })).toBeDisabled()
    expect(toggle()).toHaveTextContent('Show choices')
    expect(toggle()).toHaveAttribute('aria-pressed', 'true')
    // The overlay is laid over the table: hiding its cards moves no area of the grid.
    expect(main.style.gridTemplateAreas).toBe(areas)
    expect(host.hidden).toBe(false)

    expect(board(second)).toHaveAttribute('data-highlight', 'legal')
    await user.click(board(second))
    expect(board(second)).toHaveAttribute('data-highlight', 'selected')
    await act(async () => {})
    expect(text()).toHaveTextContent('1 target chosen')
    expect(within(answers).getByRole('button', { name: 'Confirm targets' })).toBeEnabled()
    await user.click(board(second))
    expect(board(second)).toHaveAttribute('data-highlight', 'legal')
    await user.click(board(second))

    await user.click(toggle())
    expect(overlay()).not.toHaveAttribute('data-peek')
    expect(picture(second)).toHaveAttribute('aria-pressed', 'true')
    expect(picture(must(pair[0]))).toHaveAttribute('aria-pressed', 'false')
  })
})

describe('the key V', () => {
  it('hides and shows the cards, and focus stays on the toggle', async () => {
    const { overlay, toggle } = table('targets-twins', WIDE)
    const user = userEvent.setup()
    act(() => toggle().focus())
    await user.keyboard('v')
    expect(overlay()).toHaveAttribute('data-peek')
    expect(toggle()).toHaveFocus()
    await user.keyboard('v')
    expect(overlay()).not.toHaveAttribute('data-peek')
    expect(toggle()).toHaveFocus()
  })
})

describe('a choice with no card on the table', () => {
  it('offers no Pick on board', () => {
    const { host } = table('cards-graveyard', PHONE, 4)
    expect(host.querySelector('[data-slot="choice-overlay"]')).not.toBeNull()
    expect(host.querySelector('[data-slot="pick-on-board"]')).toBeNull()
  })
})

describe('twins in the overlay', () => {
  it('carry the same number on the picture and on the table, and a reader hears it', () => {
    const { container, pair, picture } = table('targets-twins', WIDE)
    expect(pair).toHaveLength(2)
    const numbers = pair.map((id) =>
      within(picture(id)).getByText(/^\d+$/, { selector: '[data-slot="twin-number"]' }),
    )
    const [one, two] = numbers.map((chip) => chip.textContent)
    expect(one).not.toBe(two)
    const style = must(container.ownerDocument.querySelector('style[data-slot="twin-style"]')).textContent
    pair.forEach((id, at) =>
      expect(style).toContain(`[data-object-id='${id}']::after{content:'${numbers[at]?.textContent}'}`),
    )
    const labels = pair.map((id) => picture(id).getAttribute('aria-label'))
    expect(labels[0]).not.toBe(labels[1])
    for (const [at, label] of labels.entries()) expect(label).toContain(` ${numbers[at]?.textContent} `)
  })

  it('ring the object on the table while the picture is under the pointer', () => {
    const { container, pair, picture } = table('targets-twins', WIDE)
    const second = must(pair[1])
    fireEvent.pointerOver(picture(second))
    const style = must(container.ownerDocument.querySelector('style[data-slot="twin-style"]')).textContent
    expect(style).toContain(`button[data-object-id='${second}']{z-index:16;outline`)
  })
})

const creature = (
  id: ObjectId,
  name: string,
  controller: number,
  over: Partial<ObjectView> = {},
): ObjectView =>
  ({
    id,
    name,
    controller,
    owner: controller,
    zone: 'Battlefield',
    types: ['Creature'],
    permanent: { tapped: false, damage: 0, summoning_sick: false },
    ...over,
  }) as ObjectView

describe('twinNumbers and shownTwins', () => {
  const objects = new Map<ObjectId, ObjectView>([
    [1, creature(1, 'Serra Angel', 0)],
    [2, creature(2, 'Serra Angel', 1)],
    [3, creature(3, 'Serra Angel', 1)],
    [4, creature(4, 'Hill Giant', 1)],
    [5, creature(5, 'Serra Angel', 1, { zone: { Graveyard: 1 } })],
  ])
  it.each([
    [
      [1, 2, 3, 4],
      [
        [1, 1],
        [2, 2],
        [3, 3],
      ],
      [
        [2, 2],
        [3, 3],
      ],
    ],
    [
      [1, 2],
      [
        [1, 1],
        [2, 2],
      ],
      [],
    ],
    [
      [2, 5],
      [
        [2, 1],
        [5, 2],
      ],
      [],
    ],
    [[4], [], []],
  ])('of %j: numbered %j, drawn %j', (ids, numbered, drawn) => {
    const numbers = twinNumbers(ids, objects)
    expect([...numbers]).toEqual(numbered)
    expect([...shownTwins(numbers, objects)]).toEqual(drawn)
  })
})

describe('twinFacts', () => {
  it.each([
    [{}, []],
    [{ permanent: { tapped: true, damage: 2, summoning_sick: false } }, ['tapped', '2 damage']],
    [{ counters: [['+1/+1', 2]] }, ['2 +1/+1 counters']],
    [{ attached_to: 9 }, ['attached to Forest']],
    [{ attachments: [9] }, ['with Forest']],
  ] as [Partial<ObjectView>, string[]][])('of %j: %j', (over, facts) => {
    expect(twinFacts(creature(1, 'Bear', 0, over), (id) => (id === 9 ? 'Forest' : undefined))).toEqual(facts)
  })
})

describe('twinStyle', () => {
  it('is empty with no twins and nothing pointed at', () => {
    expect(twinStyle(new Map(), null)).toBe('')
  })
  it('numbers each twin and rings the pointed object', () => {
    const style = twinStyle(
      new Map([
        [4, 1],
        [7, 2],
      ]),
      7,
    )
    expect(style).toContain(`[data-object-id='4']::after{content:'1'}`)
    expect(style).toContain(`[data-object-id='7']::after{content:'2'}`)
    expect(style).toContain(`[data-object-id='7']{z-index:16`)
  })
})
