// Goal: the `?states=1` fixture is a valid four-seat Commander table that holds a permanent for every state a card shows, its attachments agree on both ends, its equal tokens share a box, its printed cards make the changed numbers read as buffed or debuffed, and nothing of it leaks into a table without the switch.
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { fixtureState } from './devGame'
import { buildFixture } from './fixture'
import { STATES_BASE_PERMANENTS, printedDetail, withStates } from './states'
import { loyaltyOf, statusLabels } from '../cardModel'
import { battlefieldStacks } from '../rowStacks'
import { statBadge } from '../overlay'
import type { ObjectView } from '@/protocol/generated'

const plain = buildFixture({ seats: 4, permanents: STATES_BASE_PERMANENTS })
const fixture = withStates(plain)
const objects = fixture.view.objects
const byId = new Map(objects.map((object) => [object.id, object]))
const onBattlefield = objects.filter((object) => object.zone === 'Battlefield')

describe('the states fixture', () => {
  it('adds permanents and one foretold card to a four-seat table and changes nothing else', () => {
    expect(fixture.view.players).toHaveLength(4)
    expect(fixture.view.format).toBe('Commander')
    expect(objects.length).toBeGreaterThan(plain.view.objects.length)
    expect(new Set(objects.map((object) => object.id)).size).toBe(objects.length)
    expect(fixture.view.battlefield.count).toBe(fixture.view.battlefield.cards?.length)
    expect(fixture.view.exile.cards?.some((id) => byId.get(id)?.face_down === true)).toBe(true)
    expect(withStates(plain)).toEqual(fixture)
    expect(plain.view.objects.every((object) => object.face_down === undefined)).toBe(true)
  })

  it('lists every battlefield id once, each with an object', () => {
    const ids = fixture.view.battlefield.cards ?? []
    expect(new Set(ids).size).toBe(ids.length)
    for (const id of ids) expect(byId.has(id)).toBe(true)
  })

  it('agrees on both ends of every attachment, a rival host included', () => {
    for (const object of onBattlefield) {
      if (object.attached_to !== undefined) {
        const host = must(byId.get(object.attached_to))
        expect(host.attachments).toContain(object.id)
      }
      for (const id of object.attachments ?? []) expect(byId.get(id)?.attached_to).toBe(object.id)
    }
  })

  const has = (
    name: string,
    test: (object: ObjectView) => boolean,
  ): [string, (object: ObjectView) => boolean] => [name, test]
  it.each([
    has('a +1/+1 counter', (o) => o.counters?.some(([kind]) => kind === '+1/+1') === true),
    has('a -1/-1 counter', (o) => o.counters?.some(([kind]) => kind === '-1/-1') === true),
    has('a charge counter', (o) => o.counters?.some(([kind]) => kind === 'charge') === true),
    has('a named counter', (o) => o.counters?.some(([kind]) => typeof kind === 'object') === true),
    has('a planeswalker', (o) => loyaltyOf(o) !== null),
    has(
      'damage short of lethal',
      (o) => (o.permanent?.damage ?? 0) === 1 || (o.permanent?.damage ?? 0) === 2,
    ),
    has('lethal damage', (o) => statusLabels(o, null).includes('4 dmg')),
    has('a tapped permanent', (o) => o.permanent?.tapped === true),
    has('a summoning-sick permanent', (o) => o.permanent?.summoning_sick === true),
    has('an Aura on a permanent', (o) => o.attached_to !== undefined && o.subtypes.includes('Aura')),
    has('an Aura on a player', (o) => o.attached_player !== undefined),
    has('an Equipment', (o) => o.attached_to !== undefined && o.subtypes.includes('Equipment')),
    has('a transformed or back face', (o) => (o.face ?? 0) > 0),
    has('a face-down card the viewer may see', (o) => o.face_down === true && o.identity !== undefined),
    has('a face-down card the viewer may not see', (o) => o.face_down === true && o.identity === undefined),
    has('a copy', (o) => o.copy !== undefined),
    has('a merge', (o) => (o.merged?.length ?? 0) > 1),
    has('a prepared permanent', (o) => o.permanent?.prepared === true),
    has('a Class level', (o) => o.permanent?.level !== undefined),
  ])('holds %s', (_, test) => {
    expect(objects.some(test)).toBe(true)
  })

  it('has an Aura of the viewer on a rival’s creature and a rival’s own Aura on its own creature', () => {
    const viewer = fixture.view.viewer
    const arrest = must(onBattlefield.find((o) => o.name === 'Arrest'))
    expect(arrest.controller).toBe(viewer)
    expect(must(byId.get(must(arrest.attached_to))).controller).not.toBe(viewer)
    const rival = must(onBattlefield.find((o) => o.name === 'Pacifism' && o.controller !== viewer))
    expect(must(byId.get(must(rival.attached_to))).controller).toBe(rival.controller)
  })

  it('shares one box among the equal tokens and keeps the changed ones apart', () => {
    const state = fixtureState(fixture)
    const stacks = battlefieldStacks(state, fixture.view.viewer).plain.creatures
    const soldiers = stacks.filter((stack) => byId.get(stack.ids[0] ?? -1)?.name === 'Soldier')
    expect(soldiers.map((stack) => stack.ids.length).sort()).toEqual([1, 1, 3])
  })

  it('says whether each changed number rose or fell from the printed card', async () => {
    const expected: [string, string | null][] = [
      ['Grizzly Bears', 'up'],
      ['Hill Giant', 'down'],
      ['Jace Beleren', 'up'],
      ['Liliana of the Veil', 'down'],
      ['Teferi, Hero of Dominaria', null],
    ]
    for (const [name, modified] of expected) {
      const object = must(
        onBattlefield.find(
          (o) => o.name === name && o.controller === fixture.view.viewer && o.identity !== undefined,
        ),
      )
      const faces = (await printedDetail(must(object.identity).oracle_id)).faces
      const printed = must(faces[0])
      const badge = statBadge(object, printed, loyaltyOf(object))
      expect(badge?.modified ?? null).toBe(modified)
    }
  })

  it('knows no printed card it was not given', async () => {
    await expect(printedDetail('unknown')).rejects.toThrow('no printed card')
  })
})
