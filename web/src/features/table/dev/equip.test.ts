// Goal: `?equip=1` adds an attachments scene whose hosts and attachments agree, on every board, and the table tucks five attachments behind the viewer's first creature in the order Auras then Equipment, draws each attachment at its host whoever controls it and keeps the unattached Equipment and the Curse in the viewer's lanes.
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { parseDevSearch } from '../devSearch'
import { battlefieldStacks, idsOfRows } from '../rowStacks'
import { fixtureState } from './devGame'
import { EQUIP_STACK, withEquip } from './equip'
import { buildFixture } from './fixture'

describe('the attachments scene', () => {
  it.each([
    ['1', true],
    [1, true],
    [true, true],
    ['yes', undefined],
    [undefined, undefined],
  ])('reads equip=%s as %s', (value, parsed) => {
    expect(parseDevSearch({ equip: value }).equip).toBe(parsed)
  })

  it.each([
    ['four seats', buildFixture({ seats: 4 })],
    ['the duel board', buildFixture({ seats: 2, board: 'duel' })],
    ['the heavy board', buildFixture({ seats: 2, board: 'heavy' })],
  ])('on %s every attachment is listed by its host and the host stacks them', (_label, fixture) => {
    const scene = withEquip(fixture)
    const byId = new Map(scene.view.objects.map((object) => [object.id, object]))
    for (const object of scene.view.objects) {
      if (object.attached_to === undefined) continue
      expect(byId.get(object.attached_to)?.attachments).toContain(object.id)
    }
    const state = fixtureState(scene)
    const viewer = scene.view.viewer
    const stacks = battlefieldStacks(state, viewer, false).plain
    const host = must(stacks.creatures.find((stack) => (stack.tucked?.length ?? 0) === EQUIP_STACK.length))
    expect(host.tucked?.map((id) => byId.get(id)?.name)).toEqual([...EQUIP_STACK])
    const drawn = new Set(idsOfRows(stacks))
    const names = [...drawn].map((id) => byId.get(id)?.name)
    // The rival's Pacifism lies at the viewer's creature; the viewer's Eel Umbra at the rival's.
    expect(names).toEqual(expect.arrayContaining(['Sword of Fire and Ice', 'Curse of Opulence', 'Short Sword', 'Pacifism']))
    expect(names).not.toContain('Eel Umbra')
    const rival = must(scene.view.players.find((player) => player.seat !== viewer)).seat
    const theirs = idsOfRows(battlefieldStacks(state, rival, false).plain).map((id) => byId.get(id)?.name)
    expect(theirs).toContain('Eel Umbra')
    const pacifism = Math.max(...scene.view.objects.filter((object) => object.name === 'Pacifism').map((object) => object.id))
    expect(idsOfRows(battlefieldStacks(state, rival, false).plain)).not.toContain(pacifism)
  })
})
