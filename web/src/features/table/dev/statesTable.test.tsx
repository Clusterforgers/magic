// Goal: `?states=1` is read only as 1, true or "1", and the real table over the states fixture draws each state on the card that has it: attachments behind their host, a player's Curse, the copy, the other face, the merge, the face-down cards and the stack of equal tokens.
import { within } from '@testing-library/react'
import { beforeEach, describe, expect, it } from 'vitest'
import { fixtureGame } from './devGame'
import { setDetailFetcher } from '../printed'
import { parseDevSearch } from '../devSearch'
import { renderTable } from '@/test-utils/tableHarness'
import { must } from '@/test-utils/batches'
import { printedDetail, STATES_BASE_PERMANENTS, withStates } from './states'
import { buildFixture } from './fixture'

describe('?states=', () => {
  it.each([true, 1, '1'])('reads %j', (states) => {
    expect(parseDevSearch({ seats: 4, states }).states).toBe(true)
  })

  it.each([undefined, 0, '0', 2, '', 'true', 'yes', null, false])('leaves out %j', (states) => {
    expect(parseDevSearch({ seats: 4, states }).states).toBeUndefined()
  })

  it('makes a game of four seats with the state permanents, and a game without the switch has none', () => {
    const on = must(fixtureGame({ seats: 4, states: true }).session.store.getState().view)
    expect(on.players).toHaveLength(4)
    expect(on.objects.some((object) => object.name === 'Bard Class')).toBe(true)
    const off = must(fixtureGame({ seats: 4 }).session.store.getState().view)
    expect(off.objects.some((object) => object.name === 'Bard Class')).toBe(false)
  })
})

describe('the table over the states fixture', () => {
  beforeEach(() => setDetailFetcher(printedDetail))

  const fixture = withStates(buildFixture({ seats: 4, permanents: STATES_BASE_PERMANENTS }))
  const draw = () => renderTable({ fixture, options: { mode: 'grid', stacks: true } })
  const card = (container: HTMLElement, name: RegExp) => within(container).getAllByRole('button', { name })

  it('marks a host with what is attached to it and each attachment with its host', () => {
    const { container } = draw()
    const lion = must(container.querySelector('[data-hosting="2"]'))
    expect(lion.getAttribute('aria-label')).toMatch(/Pacifism and Bonesplitter attached/)
    const id = Number(lion.getAttribute('data-object-id'))
    const attached = [...container.querySelectorAll('[data-attached]')].filter((button) =>
      (button.getAttribute('aria-label') ?? '').includes('attached to Silvercoat Lion'),
    )
    expect(attached.map((button) => button.getAttribute('data-object-id'))).toEqual([
      String(id + 1),
      String(id + 2),
    ])
  })

  it('names the player a Curse enchants and the creature an Aura of the viewer sits on', () => {
    const { container } = draw()
    expect(card(container, /Curse of Chains.*attached to Bo/).length).toBeGreaterThan(0)
    expect(card(container, /Arrest.*attached to Hill Giant/).length).toBeGreaterThan(0)
  })

  it('says copy, back face, merge, face down, prepared and level aloud', () => {
    const { container } = draw()
    expect(card(container, /Serra Angel.*copy of Serra Angel/).length).toBeGreaterThan(0)
    expect(card(container, /Insectile Aberration.*back face/).length).toBeGreaterThan(0)
    expect(card(container, /Gemrazer.*merged from Gemrazer, Hill Giant/).length).toBeGreaterThan(0)
    expect(card(container, /Hostile Hostel.*prepared/).length).toBeGreaterThan(0)
    expect(card(container, /Bard Class.*level 2/).length).toBeGreaterThan(0)
    expect(container.querySelectorAll('[data-slot="card-status"]').length).toBeGreaterThan(0)
  })

  it('marks the cards with clauses left to the table, says so in their names and lists them on h', async () => {
    const { container, user } = draw()
    const marked = [...container.querySelectorAll('button[data-object-id]')]
      .filter((button) => button.querySelector('[data-slot="card-support"]') !== null)
      .map((button) => button.getAttribute('aria-label') ?? '')
    expect(marked.map((name) => name.split(',')[0]).sort()).toEqual([
      'Observed Stasis',
      'Verbose Oracle',
      'Wild Mage Apprentice',
    ])
    expect(marked.every((name) => name.includes('some rules are played by hand, press H to read them'))).toBe(
      true,
    )
    const stasis = card(container, /^Observed Stasis/)[0] as HTMLElement
    stasis.focus()
    await user.keyboard('h')
    const list = within(document.body).getByRole('dialog', { name: 'Observed Stasis: rules played by hand' })
    expect(within(list).getAllByRole('listitem')).toHaveLength(2)
    expect(list).toHaveTextContent('loses all abilities and can’t attack or block')
  })

  it('draws the face-down card of a rival as a back and the equal tokens as one stack of three', () => {
    const { container } = draw()
    expect(within(container).getAllByRole('img', { name: 'Face-down card' }).length).toBeGreaterThan(0)
    const stacks = [...container.querySelectorAll('[data-stack]')].map((stack) =>
      stack.getAttribute('data-stack'),
    )
    expect(stacks).toContain('3')
  })
})
