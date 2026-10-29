/** Goal: over a real session, a scry arrives as the three decisions the engine asks (the cards for the bottom, then the order of the cards on top, then the order of the cards for the bottom), each draws its own prompt and sends one Answer of the shape the engine expects that echoes the decision id, a replacement choice sends its index, and each prompt goes with its decision. */
import { act, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { DecisionKind, ObjectId, ServerMessage } from '@/protocol/generated'
import { creaturesOf, fixtureOf } from '@/test-utils/combatTable'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { LOOKED_MOMENTS } from '../dev/lookedPrompts'
import { fixtureLookup } from '../dev/devGame'

const SEATS = 2
const fixture = (LOOKED_MOMENTS['scry'] ?? ((f) => f))(fixtureOf(SEATS))
const library = fixture.view.objects
  .filter((o) => typeof o.zone === 'object' && 'Library' in o.zone)
  .map((o) => o.id)
const [source] = creaturesOf(fixture, 0) as [ObjectId]
const SCRY = 'Put any number of the cards on the bottom of your library.'

const decision = (id: number, kind: DecisionKind): ServerMessage => ({
  type: 'Decision',
  id,
  decision: { player: 0, kind },
})

function mount() {
  const h = startSession()
  h.join(0, SEATS)
  act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
  const table = renderLiveTable(h.session, fixtureLookup(fixture))
  const send = (message: ServerMessage): void => act(() => h.deliver(message))
  const prompt = (name: string): HTMLElement | null =>
    table.container.querySelector(`[data-decision="${name}"]`)
  const card = (id: ObjectId): HTMLElement =>
    table.container.querySelector<HTMLElement>(`button[data-candidate="${id}"]`) as HTMLElement
  return { h, ...table, send, prompt, card }
}

describe('a scry of four cards', () => {
  it('asks the bottom, the order of the top and the order of the bottom, and answers each once', async () => {
    const [a, b, c, d] = library as [ObjectId, ObjectId, ObjectId, ObjectId]
    const { h, send, user, prompt, card } = mount()

    send(decision(41, { ChooseCards: { source, from: [a, b, c, d], min: 0, max: 4, text: SCRY } }))
    expect(prompt('ChooseCards')?.querySelector('[data-slot="look-piles"]')).not.toBeNull()
    await user.click(card(b))
    await user.click(card(d))
    await user.dblClick(screen.getByRole('button', { name: 'Confirm' }))
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 41, answer: { ChooseCards: [b, d] } }])
    expect(prompt('ChooseCards')).toBeNull()

    send(
      decision(42, {
        OrderCards: { source, cards: [a, c], text: 'Put the cards on top of your library in any order.' },
      }),
    )
    expect(prompt('OrderCards')).not.toBeNull()
    await user.click(
      document.querySelector<HTMLElement>(`button[data-card="${c}"][data-move="up"]`) as HTMLElement,
    )
    await user.click(screen.getByRole('button', { name: 'Confirm order' }))

    send(
      decision(43, {
        OrderCards: {
          source,
          cards: [b, d],
          text: 'Put the cards on the bottom of your library in any order.',
        },
      }),
    )
    expect(document.querySelector('[data-slot="order-pile"]')).toHaveTextContent(
      'Going on the bottom of your library',
    )
    await user.click(screen.getByRole('button', { name: 'Confirm order' }))

    expect(h.sentOf('Answer')).toEqual([
      { type: 'Answer', decision: 41, answer: { ChooseCards: [b, d] } },
      { type: 'Answer', decision: 42, answer: { OrderCards: [c, a] } },
      { type: 'Answer', decision: 43, answer: { OrderCards: [b, d] } },
    ])
    expect(h.state().pending).toBeNull()
  })

  it('answers none when every card stays on top', async () => {
    const { h, send, user } = mount()
    send(decision(44, { ChooseCards: { source, from: library.slice(0, 3), min: 0, max: 3, text: SCRY } }))
    await user.click(screen.getByRole('button', { name: 'Confirm' }))
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 44, answer: { ChooseCards: [] } }])
  })
})

describe('a replacement choice from the server', () => {
  it('draws the prompt with focus on the first effect, and the key sends its index', async () => {
    const { h, send, user, prompt } = mount()
    const sources = creaturesOf(fixture, 0).slice(0, 3)
    send(
      decision(51, {
        ChooseReplacement: {
          candidates: sources.map((id) => ({ source: id, ability: 0 })),
          text: 'Create 2 token(s).',
        },
      }),
    )
    expect(prompt('ChooseReplacement')).not.toBeNull()
    expect(document.querySelector('[data-slot="replacement-effect"]')).toHaveFocus()
    await user.keyboard('2')
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 51, answer: { ChooseReplacement: 1 } }])
    expect(prompt('ChooseReplacement')).toBeNull()
    expect(h.state().pending).toBeNull()
  })
})
