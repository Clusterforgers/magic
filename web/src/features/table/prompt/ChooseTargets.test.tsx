/** Goal: for every kind of target decision (one target, up to some, a range, players, objects the table draws and objects it does not, 2 to 12 seats) the legal cards are marked and the rest of the viewer dimmed; a click on a card or a button of the list chooses a target and again takes it back, one target switches while a full choice of several closes the rest; Confirm waits for the minimum and sends the targets in the order chosen, once; Undo and Remove take targets back; Cancel takes a spell back and a triggered ability has none; and the sentence names the source and the words of the specification. */
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { Fixture } from '@/features/table/dev/fixture'
import type { ObjectId, ObjectView, Target, TargetOptions } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, ownOf, renderDecision } from '@/test-utils/combatTable'

const objects = (ids: readonly ObjectId[]): Target[] => ids.map((id) => ({ Object: { id, counter: 0 } }))
const players = (seats: number): Target[] => Array.from({ length: seats }, (_, seat) => ({ Player: seat }))

interface Spec {
  name: string
  seats: number
  legal(fixture: Fixture): Target[]
  min: number
  max: number
}

const SPECS: Spec[] = [
  {
    name: 'one creature of the next seat',
    seats: 2,
    legal: (f) => objects(creaturesOf(f, 1).slice(0, 4)),
    min: 1,
    max: 1,
  },
  {
    name: 'a player or a creature, at 3 seats',
    seats: 3,
    legal: (f) => [...players(3), ...objects(creaturesOf(f, 1).slice(0, 3))],
    min: 1,
    max: 1,
  },
  {
    name: 'up to two creatures',
    seats: 2,
    legal: (f) => objects(creaturesOf(f, 1).slice(0, 5)),
    min: 0,
    max: 2,
  },
  {
    name: 'one to three creatures of the viewer and an opponent',
    seats: 4,
    legal: (f) => objects([...creaturesOf(f, 0).slice(0, 2), ...creaturesOf(f, 2).slice(0, 3)]),
    min: 1,
    max: 3,
  },
  { name: 'exactly two players, at 4 seats', seats: 4, legal: () => players(4), min: 2, max: 2 },
  {
    name: 'creatures at 12 seats, some on a seat that is not drawn',
    seats: 12,
    legal: (f) => objects([...creaturesOf(f, 1).slice(0, 2), ...creaturesOf(f, 7).slice(0, 2)]),
    min: 1,
    max: 2,
  },
]

/** The viewer's own spell on the stack of the fixture. */
const spellOf = (fixture: Fixture): ObjectView =>
  must(fixture.view.objects.find((object) => object.zone === 'Stack' && object.controller === 0))

function mount(
  spec: Pick<Spec, 'seats' | 'min' | 'max'> & { legal: Target[] },
  over: Partial<TargetOptions> = {},
) {
  const fixture = fixtureOf(spec.seats)
  const options: TargetOptions = {
    source: spellOf(fixture).id,
    spec: 0,
    legal: spec.legal,
    min: spec.min,
    max: spec.max,
    can_cancel: true,
    wording: 'target creature',
    ...over,
  }
  const table = renderDecision({ ChooseTargets: options }, { seats: spec.seats })
  const group = (): HTMLElement => screen.getByRole('group', { name: 'Legal targets' })
  const listed = (): HTMLElement[] => within(group()).getAllByRole('button')
  const chosenList = (): HTMLElement | null => screen.queryByRole('list', { name: 'Your targets' })
  const chosen = (): HTMLElement[] => {
    const list = chosenList()
    return list === null ? [] : within(list).getAllByRole('listitem')
  }
  const confirm = (): HTMLElement => screen.getByRole('button', { name: 'Confirm targets' })
  /** Chooses the target at `index` of the legal list as a person would: its card when the table draws one. */
  const choose = async (index: number): Promise<void> => {
    const target = must(options.legal[index])
    if ('Object' in target && table.hasCard(target.Object.id))
      await table.user.click(table.card(target.Object.id))
    else await table.user.click(must(listed()[index]))
  }
  return { ...table, fixture, options, group, listed, chosen, chosenList, confirm, choose }
}

const withSpec = (spec: Spec) => mount({ ...spec, legal: spec.legal(fixtureOf(spec.seats)) })

describe('what is marked', () => {
  it.each(SPECS)('with $name the legal cards are legal and the rest of the viewer is dimmed', (spec) => {
    const { marks, options, fixture, hasCard } = withSpec(spec)
    const legal = new Set(options.legal.flatMap((target) => ('Object' in target ? [target.Object.id] : [])))
    const expected = new Map<ObjectId, string>(
      ownOf(fixture).map((id) => [id, legal.has(id) ? 'legal' : 'dimmed']),
    )
    for (const id of legal) if (hasCard(id)) expected.set(id, 'legal')
    expect(marks()).toEqual(expected)
  })

  it.each(SPECS)(
    'with $name the list holds each legal target once, in the order given, none pressed',
    (spec) => {
      const { listed, options } = withSpec(spec)
      expect(listed()).toHaveLength(options.legal.length)
      for (const button of listed()) expect(button).toHaveAttribute('aria-pressed', 'false')
    },
  )
})

describe('choosing', () => {
  it.each(SPECS)(
    'with $name each choice marks, lists and counts, and Confirm sends them once in order',
    async (spec) => {
      const table = withSpec(spec)
      const { options, marks, hasCard, listed, chosen, confirm, choose, text, answer, user } = table
      const picks = Math.min(spec.max, options.legal.length)
      const waiting = (): boolean => confirm().matches(':disabled')
      expect(waiting()).toBe(spec.min > 0)
      for (let i = 0; i < picks; i += 1) {
        await choose(i)
        const target = must(options.legal[i])
        if ('Object' in target && hasCard(target.Object.id))
          expect(marks().get(target.Object.id)).toBe('selected')
        expect(listed()[i]).toHaveAttribute('aria-pressed', 'true')
        expect(chosen()).toHaveLength(i + 1)
        expect(waiting()).toBe(i + 1 < spec.min)
      }
      if (picks > 0) expect(text()).toContain(`${picks} ${picks === 1 ? 'target' : 'targets'} chosen`)
      await user.click(confirm())
      expect(answer).toHaveBeenCalledTimes(1)
      expect(answer).toHaveBeenCalledWith({ ChooseTargets: { Targets: options.legal.slice(0, picks) } })
    },
  )

  it.each(SPECS.filter((spec) => spec.max > 1))(
    'with $name a full choice disables the other buttons and unmarks the other cards until a target is taken back',
    async (spec) => {
      const { options, marks, hasCard, listed, choose, user } = withSpec(spec)
      for (let i = 0; i < spec.max; i += 1) await choose(i)
      const rest = options.legal.slice(spec.max)
      for (const [offset, target] of rest.entries()) {
        expect(listed()[spec.max + offset]).toBeDisabled()
        if ('Object' in target && hasCard(target.Object.id)) {
          expect(marks().get(target.Object.id)).not.toBe('legal')
        }
      }
      // A click that would be the third target changes nothing.
      if (rest[0] !== undefined) {
        await choose(spec.max)
        expect(listed().filter((b) => b.getAttribute('aria-pressed') === 'true')).toHaveLength(spec.max)
      }
      await user.click(must(listed()[0]))
      expect(listed()[0]).toHaveAttribute('aria-pressed', 'false')
      for (let i = spec.max; i < listed().length; i += 1) expect(listed()[i]).toBeEnabled()
    },
  )

  it.each(SPECS.filter((spec) => spec.max === 1))(
    'with $name a click on another target changes the choice, and the same target again takes it back',
    async (spec) => {
      const { listed, chosen, choose, confirm, answer, user } = withSpec(spec)
      await choose(0)
      await choose(1)
      expect(listed()[0]).toHaveAttribute('aria-pressed', 'false')
      expect(listed()[1]).toHaveAttribute('aria-pressed', 'true')
      expect(chosen()).toHaveLength(1)
      await choose(1)
      expect(chosen()).toHaveLength(0)
      expect(confirm()).toBeDisabled()
      await choose(0)
      await user.click(confirm())
      expect(answer).toHaveBeenCalledTimes(1)
    },
  )

  it('takes a target back with Undo (the last one) and with Remove (the one on its row)', async () => {
    const { listed, chosen, chosenList, choose, user } = mount({
      seats: 2,
      legal: players(2).concat(objects(creaturesOf(fixtureOf(2), 1).slice(0, 2))),
      min: 0,
      max: 3,
    })
    const undo = (): HTMLElement => screen.getByRole('button', { name: 'Undo' })
    expect(undo()).toBeDisabled()
    for (const index of [0, 1, 2]) await choose(index)
    expect(chosen()).toHaveLength(3)
    await user.click(undo())
    expect(listed().map((b) => b.getAttribute('aria-pressed'))).toEqual(['true', 'true', 'false', 'false'])
    await user.click(within(must(chosenList())).getByRole('button', { name: /^Remove Alice/ }))
    expect(listed().map((b) => b.getAttribute('aria-pressed'))).toEqual(['false', 'true', 'false', 'false'])
    expect(chosen()).toHaveLength(1)
    await user.click(undo())
    expect(chosenList()).toBeNull()
    expect(undo()).toBeDisabled()
  })

  it('sends an empty choice when up to some are asked for and none is chosen', async () => {
    const { confirm, answer, user } = withSpec(must(SPECS[2]))
    expect(confirm()).toBeEnabled()
    await user.click(confirm())
    expect(answer).toHaveBeenCalledWith({ ChooseTargets: { Targets: [] } })
  })

  it('sends a double click on Confirm as one answer', async () => {
    const { confirm, choose, answer, user } = withSpec(must(SPECS[0]))
    await choose(0)
    await user.dblClick(confirm())
    expect(answer).toHaveBeenCalledTimes(1)
  })
})

describe('the sentence and the way out', () => {
  const two = { seats: 2, legal: objects(creaturesOf(fixtureOf(2), 1).slice(0, 2)), min: 1, max: 1 }

  it('names the spell and asks in the words of the specification', () => {
    const { text, fixture } = mount(two, { wording: 'target creature you do not control' })
    expect(text()).toBe(`${spellOf(fixture).name}: choose target creature you do not control.`)
  })

  it('says how many targets are asked for when the engine gave no words', () => {
    const { text, fixture } = mount({ ...two, min: 0, max: 2 }, { wording: '' })
    expect(text()).toBe(`${spellOf(fixture).name}: choose up to 2 targets.`)
  })

  it('takes a spell back with Cancel, which nothing is paid for yet', async () => {
    const { answer, user } = mount(two)
    await user.click(screen.getByRole('button', { name: 'Cancel the spell' }))
    expect(answer).toHaveBeenCalledWith({ ChooseTargets: 'Cancel' })
  })

  it('calls the way out of an activated ability the ability', () => {
    const fixture = fixtureOf(2)
    const permanent = must(fixture.view.objects.find((o) => o.zone === 'Battlefield' && o.controller === 0))
    mount(two, { source: permanent.id })
    expect(screen.getByRole('button', { name: 'Cancel the ability' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Cancel the spell' })).toBeNull()
  })

  it('has no way out of a triggered ability, which names its source', () => {
    const fixture = fixtureOf(2)
    const permanent = must(fixture.view.objects.find((o) => o.zone === 'Battlefield' && o.controller === 0))
    const { text } = mount(two, { source: permanent.id, can_cancel: false, triggered: true })
    expect(text()).toBe(`${permanent.name}'s ability: choose target creature.`)
    expect(screen.queryByRole('button', { name: /^Cancel/ })).toBeNull()
  })

  it('names an ability whose source the view does not hold', () => {
    const { text } = mount(two, { source: 999_999, can_cancel: false, triggered: true })
    expect(text()).toBe('A triggered ability: choose target creature.')
  })

  it('offers Confirm only once enough are chosen, and says how many more it takes', async () => {
    const { text, confirm, choose } = mount({ ...two, min: 2, max: 2 }, { wording: '' })
    expect(confirm()).toBeDisabled()
    await choose(0)
    expect(text()).toContain('1 of 2 targets chosen. Choose 1 more.')
    expect(confirm()).toBeDisabled()
    await choose(1)
    expect(text()).toContain('2 targets chosen. Confirm the targets, or take one back.')
    expect(confirm()).toBeEnabled()
  })
})

describe('what a button says of its target', () => {
  it('names a player by name, the viewer as "you", and an object with its power and toughness and whose it is', () => {
    const fixture = fixtureOf(3)
    const [creature] = creaturesOf(fixture, 1)
    const { listed } = mount({
      seats: 3,
      legal: [{ Player: 0 }, { Player: 2 }, ...objects([must(creature)])],
      min: 1,
      max: 1,
    })
    const object = must(fixture.view.objects.find((o) => o.id === creature))
    expect(listed().map((b) => b.textContent)).toEqual([
      'Alice (you)',
      'Cy',
      `${object.name} ${object.power}/${object.toughness} (Bo)`,
    ])
  })

  it('says where a card is that the table does not draw it in: on the stack, in a graveyard, in exile', async () => {
    const fixture = fixtureOf(2)
    const spell = must(fixture.view.objects.find((o) => o.zone === 'Stack' && o.controller === 1))
    const card = must(
      fixture.view.objects.find((o) => typeof o.zone === 'object' && 'Graveyard' in o.zone && o.owner === 1),
    )
    const exiled = must(fixture.view.objects.find((o) => o.zone === 'Exile'))
    const { listed, hasCard, answer, user, confirm } = mount({
      seats: 2,
      legal: objects([spell.id, card.id, exiled.id]),
      min: 1,
      max: 1,
    })
    const stats = (o: ObjectView): string => (o.power === null ? '' : ` ${o.power}/${o.toughness}`)
    expect(listed().map((b) => b.textContent)).toEqual([
      `${spell.name}${stats(spell)} (on the stack, Bo)`,
      `${card.name}${stats(card)} (in Bo's graveyard)`,
      `${exiled.name}${stats(exiled)} (in exile)`,
    ])
    expect(hasCard(card.id)).toBe(false)
    await user.click(must(listed()[1]))
    await user.click(confirm())
    expect(answer).toHaveBeenCalledWith({ ChooseTargets: { Targets: objects([card.id]) } })
  })

  it('numbers objects that share a name, so two buttons never read alike', () => {
    const fixture = fixtureOf(2)
    const byName = new Map<string, ObjectId[]>()
    for (const object of fixture.view.objects) {
      if (object.zone !== 'Battlefield' || object.controller !== 1) continue
      byName.set(object.name, [...(byName.get(object.name) ?? []), object.id])
    }
    const [name, ids] = must([...byName].find(([, list]) => list.length >= 3))
    const { listed } = mount({ seats: 2, legal: objects(ids.slice(0, 3)), min: 1, max: 1 })
    const [first, second, third] = listed().map((b) => b.textContent ?? '')
    expect(first).toMatch(new RegExp(`^${name} 1`))
    expect(second).toMatch(new RegExp(`^${name} 2`))
    expect(third).toMatch(new RegExp(`^${name} 3`))
  })

  it('lists a target once when the engine lists it twice', () => {
    const [id] = creaturesOf(fixtureOf(2), 1)
    const { listed } = mount({ seats: 2, legal: objects([must(id), must(id)]), min: 1, max: 1 })
    expect(listed()).toHaveLength(1)
  })
})
