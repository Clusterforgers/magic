// Goal: the dev fixture is a realistic, valid table for every seat count 2 to 12 (about 40 permanents per seat in every state, an attack with blockers, three spells on the stack, graveyards, a commander each and about 50 log events), is the same every time, and passes the wire parser, so the screens built on it are looking at something the server could send; it can also hand the viewer a named decision (`prompt=priority`) made of real cards, and ignores a name it does not know.
import { describe, expect, it } from 'vitest'
import { isStateView, parseMessage } from '@/net/parse'
import { applyBatch } from '@/play/view'
import type { ObjectView, StateView } from '@/protocol/generated'
import { batchOf, must, tableOf } from '@/test-utils/batches'
import { fixtureGame, fixtureLookup, fixtureSession, fixtureState } from './dev/devGame'
import { buildFixture, FIXTURE_MARKER, NAMES } from './dev/fixture'

const COUNTS = Array.from({ length: 11 }, (_, index) => index + 2)

const byController = (view: StateView, seat: number): ObjectView[] =>
  view.objects.filter((object) => object.zone === 'Battlefield' && object.controller === seat)

describe('buildFixture', () => {
  it.each(COUNTS)('builds a table of %i seats with 40 permanents each, in every state', (seats) => {
    const { view } = buildFixture({ seats })
    expect(view.players).toHaveLength(seats)
    expect(view.battlefield.count).toBe(40 * seats)
    for (let seat = 0; seat < seats; seat += 1) {
      const mine = byController(view, seat)
      expect(mine, `seat ${seat}`).toHaveLength(40)
      expect(mine.filter((object) => object.types.includes('Land'))).toHaveLength(16)
      expect(mine.filter((object) => object.types.includes('Creature'))).toHaveLength(14)
      expect(mine.filter((object) => object.permanent?.tapped === true).length).toBeGreaterThan(5)
      expect(mine.filter((object) => object.permanent?.summoning_sick === true).length).toBeGreaterThan(0)
      expect(mine.filter((object) => (object.permanent?.damage ?? 0) > 0).length).toBeGreaterThan(0)
      const types = new Set(mine.flatMap((object) => object.types))
      expect([...types].toSorted()).toEqual(['Artifact', 'Creature', 'Enchantment', 'Land'])
    }
  })

  it('gives every seat a graveyard of 5 and a commander, and the viewer a hand of 7', () => {
    const { view } = buildFixture({ seats: 5 })
    for (const player of view.players) {
      expect(player.graveyard.count).toBe(5)
      expect(player.command.count).toBe(player.seat === 0 ? 2 : 1)
      expect(player.commanders?.map((entry) => entry.object)).toEqual(player.command.cards)
      const commander = must(view.objects.find((object) => object.id === player.command.cards?.[0]))
      expect(commander.supertypes).toContain('Legendary')
      expect(commander.zone).toEqual({ Command: player.seat })
    }
    expect(view.players[0]?.hand).toMatchObject({ count: 7 })
    expect(view.players[0]?.hand.cards).toHaveLength(7)
    for (const player of view.players.slice(1)) expect(player.hand.cards).toBeNull()
    expect(view.format).toBe('Commander')
  })

  it('has an attack under way: three attackers, one double-blocked, and tapped attackers', () => {
    const { view } = buildFixture({ seats: 4 })
    expect(view.turn).toMatchObject({ number: 5, active: 1, step: 'DeclareBlockers' })
    const { attackers } = view.combat
    expect(attackers).toHaveLength(3)
    expect(attackers.map((attacker) => attacker.blocked)).toEqual([true, false, false])
    expect(attackers[0]?.blockers).toHaveLength(2)
    for (const attacker of attackers) {
      expect(attacker.target).toEqual({ Player: 0 })
      expect(view.objects.find((object) => object.id === attacker.object)?.permanent?.tapped).toBe(true)
    }
  })

  it('has three spells on the stack, cast by different seats, top last', () => {
    const { view, names } = buildFixture({ seats: 4 })
    const stack = must(view.stack.cards).map((id) => must(view.objects.find((object) => object.id === id)))
    expect(stack.map((object) => object.name)).toEqual(['Counterspell', 'Giant Growth', 'Lightning Bolt'])
    expect(stack.map((object) => names[object.controller])).toEqual(['Cy', 'Bo', 'Alice'])
    for (const spell of stack) expect(spell.zone).toBe('Stack')
  })

  it('has about 50 log events with one-to-one sequence numbers, and a few lines of chat', () => {
    const { log, chat } = buildFixture({ seats: 4 })
    expect(log.length).toBeGreaterThanOrEqual(50)
    expect(log.map((entry) => entry.seq)).toEqual(log.map((_, index) => index))
    expect(chat).toHaveLength(3)
  })

  it('is the same table every time, and a different one for different options', () => {
    expect(buildFixture({ seats: 6 })).toEqual(buildFixture({ seats: 6 }))
    expect(buildFixture({ seats: 6 }).view).not.toEqual(buildFixture({ seats: 7 }).view)
    expect(buildFixture({ seats: 6, viewer: 3 }).view.viewer).toBe(3)
  })

  it('lets the active seat be the viewer, who then attacks the next seat', () => {
    const { view } = buildFixture({ seats: 3, active: 0 })
    expect(view.turn.active).toBe(0)
    expect(
      view.combat.attackers.every((attacker) => 'Player' in attacker.target && attacker.target.Player === 1),
    ).toBe(true)
  })

  it('names the seats and keeps at least two, with no most', () => {
    expect(buildFixture({ seats: 3 }).names).toEqual(NAMES.slice(0, 3))
    expect(buildFixture({ seats: 1 }).view.players).toHaveLength(2)
    expect(buildFixture({ seats: 40 }).view.players).toHaveLength(40)
  })

  it.each([13, 24, 40, 64])('gives each of %i seats a name of its own', (seats) => {
    const { names, view } = buildFixture({ seats })
    expect(view.players).toHaveLength(seats)
    expect(new Set(names).size).toBe(seats)
  })

  it('can be made smaller for quick tests', () => {
    const { view } = buildFixture({ seats: 3, permanents: 6 })
    expect(byController(view, 2)).toHaveLength(6)
  })
})

describe('the fixture is a valid table', () => {
  it.each(COUNTS)('passes the wire parser as a Snapshot at %i seats', (seats) => {
    const { view } = buildFixture({ seats })
    expect(isStateView(view)).toBe(true)
    expect(parseMessage({ type: 'Snapshot', seq: 1, state: view }).ok).toBe(true)
  })

  it('passes the wire parser as a batch carrying the whole log', () => {
    const { view, log } = buildFixture({ seats: 4 })
    const message = {
      type: 'Events',
      ...batchOf(view, { from_seq: 0, events: log.map((entry) => entry.event) }),
    }
    expect(parseMessage(message).ok).toBe(true)
  })

  it('has every id once, in order, and every zone list pointing at an object in that zone', () => {
    const { view } = buildFixture({ seats: 4 })
    const ids = view.objects.map((object) => object.id)
    expect(ids).toEqual(ids.toSorted((a, b) => a - b))
    expect(new Set(ids).size).toBe(ids.length)
    const zoneOf = new Map(view.objects.map((object) => [object.id, object.zone]))
    for (const id of must(view.battlefield.cards)) expect(zoneOf.get(id)).toBe('Battlefield')
    for (const id of must(view.stack.cards)) expect(zoneOf.get(id)).toBe('Stack')
    for (const id of must(view.exile.cards)) expect(zoneOf.get(id)).toBe('Exile')
    for (const player of view.players) {
      for (const id of player.graveyard.cards ?? [])
        expect(zoneOf.get(id)).toEqual({ Graveyard: player.seat })
      for (const id of player.hand.cards ?? []) expect(zoneOf.get(id)).toEqual({ Hand: player.seat })
    }
  })

  it('folds an empty batch into itself, as a view from the server would', () => {
    const { view } = buildFixture({ seats: 3 })
    expect(applyBatch(view, batchOf(view))).toBe(view)
    expect(tableOf(view).players).toBe(view.players)
  })
})

describe('the fixture game', () => {
  it('holds a play state of a seat that sees the table and has a priority decision to make', () => {
    const fixture = buildFixture({ seats: 4 })
    const state = fixtureState(fixture)
    expect(state).toMatchObject({
      phase: 'playing',
      status: 'open',
      seat: 0,
      seatCount: 4,
      seq: fixture.log.length,
    })
    expect(state.pending?.decision.kind).toHaveProperty('Priority')
    expect(Object.keys(state.objectsById)).toHaveLength(fixture.view.objects.length)
  })

  it('answers the lookup with every seat named and connected', () => {
    const lookup = fixtureLookup(buildFixture({ seats: 3 }))
    expect(lookup.game?.seats.map((entry) => entry.name)).toEqual(['Alice', 'Bo', 'Cy'])
    expect(lookup.game?.connected).toEqual([0, 1, 2])
    expect(lookup).toMatchObject({ missing: false, failed: false })
  })

  it('has a session that chats into its own store and refuses everything that needs a server', () => {
    const { session } = fixtureGame({ seats: 2 })
    expect(session.chat('  hello  ')).toBe(true)
    expect(session.store.getState().chat.at(-1)).toEqual({ from: 0, text: 'hello' })
    expect(session.chat('   ')).toBe(false)
    expect(session.answer({ Priority: 'Pass' })).toBe(false)
    expect([session.ready(), session.concede(), session.resync()]).toEqual([false, false, false])
    session.connect()
    session.disconnect()
    session.store.setState({ notice: 'x' })
    session.dismissNotice()
    expect(session.store.getState().notice).toBeNull()
  })

  it('gives every fixture game a store of its own, and a session works on the store it is given', () => {
    const one = fixtureGame({ seats: 2 })
    const two = fixtureGame({ seats: 2 })
    expect(one.session.store).not.toBe(two.session.store)
    expect(fixtureSession(one.session.store).store).toBe(one.session.store)
  })

  it('carries a marker the production build is searched for', () => {
    expect(FIXTURE_MARKER).toMatch(/^dev-table-fixture-/)
  })
})

describe('the fixture game with a prompt', () => {
  it('faces no decision unless one is asked for', () => {
    expect(fixtureGame({ seats: 3 }).session.store.getState().pending).toBeNull()
  })

  it.each([undefined, 'nonsense', 'constructor', 'toString', '__proto__', ''])(
    'faces no decision for the prompt name %j',
    (prompt) => {
      expect(fixtureGame({ seats: 3, prompt }).session.store.getState().pending).toBeNull()
    },
  )

  it.each(COUNTS)('offers a priority decision of real cards at %i seats', (seats) => {
    const game = fixtureGame({ seats, prompt: 'priority' })
    const { pending, view } = game.session.store.getState()
    const decision = must(pending)
    expect(decision.decision.player).toBe(0)
    const options = must('Priority' in decision.decision.kind ? decision.decision.kind.Priority : null)
    const byId = new Map(must(view).objects.map((object) => [object.id, object]))
    const hand = must(must(view).players[0]?.hand.cards)
    expect(options.playable_lands.length).toBeGreaterThan(0)
    expect(options.castable.length).toBeGreaterThan(0)
    expect(options.can_pass).toBe(true)
    for (const id of options.playable_lands) {
      expect(hand).toContain(id)
      expect(byId.get(id)?.types).toContain('Land')
    }
    const command = must(view).players[0]?.command.cards ?? []
    for (const id of options.castable) {
      expect([...command, ...hand]).toContain(id)
      expect(byId.get(id)?.types).not.toContain('Land')
    }
    // One spell of the hand is left uncastable, so a dimmed hand card can be seen.
    expect(
      hand.filter((id) => !options.castable.includes(id) && !options.playable_lands.includes(id)),
    ).not.toEqual([])
    for (const { source } of options.mana_abilities) {
      const object = must(byId.get(source.object))
      expect(object).toMatchObject({ controller: 0, zone: 'Battlefield' })
      expect(object.permanent?.tapped).toBe(false)
    }
    const objects = options.mana_abilities.map((option) => option.source.object)
    expect(objects.some((object, index) => objects.indexOf(object) !== index)).toBe(true)
  })

  it('is a decision message the wire parser accepts', () => {
    const { pending } = fixtureGame({ seats: 2, prompt: 'priority' }).session.store.getState()
    const message = { type: 'Decision', id: must(pending).id, decision: must(pending).decision }
    expect(parseMessage(message).ok).toBe(true)
  })

  it('takes an answer as the real session does: the decision is cleared, once', () => {
    const { session } = fixtureGame({ seats: 2, prompt: 'priority' })
    expect(session.answer({ Priority: 'Pass' })).toBe(true)
    expect(session.store.getState().pending).toBeNull()
    expect(session.answer({ Priority: 'Pass' })).toBe(false)
  })
})
