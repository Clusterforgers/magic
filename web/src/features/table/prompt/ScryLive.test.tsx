/** Goal: over a real session, a scry arrives as one decision (the cards looked at), draws one prompt and sends one Answer with the top pile and the bottom pile, each in order, that echoes the decision id, a replacement choice sends its index, and each prompt goes with its decision. */
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
  return { h, ...table, send, prompt }
}

describe('a scry of four cards', () => {
  it('asks one decision for the split and both orders, and answers it once', async () => {
    const [a, b, c, d] = library as [ObjectId, ObjectId, ObjectId, ObjectId]
    const { h, send, user, prompt } = mount()
    const press = (id: ObjectId, move: string) =>
      user.click(
        document.querySelector<HTMLElement>(`button[data-card="${id}"][data-move="${move}"]`) as HTMLElement,
      )

    send(decision(41, { Scry: { source, cards: [a, b, c, d], text: SCRY } }))
    expect(prompt('Scry')?.querySelector('[data-slot="scry-piles"]')).not.toBeNull()
    await press(b, 'cross')
    await press(d, 'cross')
    await press(d, 'up')
    await press(c, 'up')
    await user.dblClick(screen.getByRole('button', { name: 'Confirm' }))
    expect(h.sentOf('Answer')).toEqual([
      { type: 'Answer', decision: 41, answer: { Scry: { top: [c, a], bottom: [d, b] } } },
    ])
    expect(prompt('Scry')).toBeNull()
    expect(h.state().pending).toBeNull()
  })

  it('answers every card on top when nothing is moved', async () => {
    const { h, send, user } = mount()
    const cards = library.slice(0, 3)
    send(decision(44, { Scry: { source, cards, text: SCRY } }))
    await user.click(screen.getByRole('button', { name: 'Confirm' }))
    expect(h.sentOf('Answer')).toEqual([
      { type: 'Answer', decision: 44, answer: { Scry: { top: cards, bottom: [] } } },
    ])
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
