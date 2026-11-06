/** Goal: a payment decision addressed to a seat that is not casting (the helper of an assist spell) draws the prompt for that seat, has no Cancel, and Pay automatically or a manual payment goes out as one Answer with the decision id. */
import { act, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectId, PayOptions } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { cost, poolOf, source } from '@/test-utils/mana'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'

const fixture = buildFixture({ seats: 3, viewer: 1 })
const lands = fixture.view.objects
  .filter(
    (o) =>
      o.controller === 1 &&
      o.zone === 'Battlefield' &&
      o.permanent?.tapped === false &&
      o.types.includes('Land'),
  )
  .map((o) => o.id)
const [landA, landB] = lands as [ObjectId, ObjectId]
// The caster's spell is on the stack: any visible object stands in for it.
const spell = must(fixture.view.objects.find((o) => o.controller === 0 && o.zone === 'Battlefield')).id

const pay: PayOptions = {
  spell,
  cost: cost('{2}'),
  pool: poolOf(''),
  sources: [source(landA, 'G'), source(landB, 'U')],
  auto_available: true,
  can_cancel: false,
  life: 20,
  max_life: 0,
}

function mount(seat: number) {
  const h = startSession()
  h.join(seat, 3)
  act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
  const table = renderLiveTable(h.session, fixtureLookup(fixture))
  const send = (decision: number, player: number): void =>
    act(() => h.deliver({ type: 'Decision', id: decision, decision: { player, kind: { PayMana: pay } } }))
  return { h, send, ...table }
}

describe('a payment decision for the helper seat', () => {
  it('draws the prompt for the helper, without Cancel, and Pay automatically answers it', async () => {
    const { h, send, user, container } = mount(1)
    send(40, 1)
    expect(container.querySelector('[data-decision="PayMana"]')).not.toBeNull()
    expect(container.querySelector('[data-slot="pay-cancel"]')).toBeNull()
    await user.click(screen.getByRole('button', { name: 'Pay automatically' }))
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 40, answer: { PayMana: 'Auto' } }])
  })

  it('a click on the helper land builds a manual payment', async () => {
    const { h, send, user, container } = mount(1)
    send(41, 1)
    for (const id of [landA, landB])
      await user.click(must(container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`)))
    await user.click(screen.getByRole('button', { name: 'Confirm payment' }))
    expect(h.sentOf('Answer')).toEqual([
      {
        type: 'Answer',
        decision: 41,
        answer: {
          PayMana: {
            Manual: {
              pool: [],
              taps: [
                { object: landA, ability: 0 },
                { object: landB, ability: 0 },
              ],
              life: 0,
            },
          },
        },
      },
    ])
  })

  it('the caster seat shows nothing for the helper decision', () => {
    const { send, container } = mount(0)
    send(42, 1)
    expect(container.querySelector('[data-decision]')).toBeNull()
  })
})
