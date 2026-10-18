/** A table of seat 0 facing a combat decision, with an answer spy and readers of what the cards show. */
import { screen } from '@testing-library/react'
import { vi } from 'vitest'
import { buildFixture } from '@/features/table/dev/fixture'
import type { Fixture } from '@/features/table/dev/fixture'
import type { Answer, AttackOptions, BlockOptions, DecisionKind, ObjectId } from '@/protocol/generated'
import { must } from './batches'
import { renderTable } from './tableHarness'

const fixtures = new Map<number, Fixture>()

/** The fixture of `seats` seats, built once per run. */
export function fixtureOf(seats: number): Fixture {
  let fixture = fixtures.get(seats)
  if (fixture === undefined) {
    fixture = buildFixture({ seats })
    fixtures.set(seats, fixture)
  }
  return fixture
}

/** The creatures `seat` controls on the battlefield, in the order the fixture lists them, untapped ones first. */
export function creaturesOf(fixture: Fixture, seat: number): ObjectId[] {
  const mine = fixture.view.objects.filter(
    (object) => object.zone === 'Battlefield' && object.controller === seat && object.types.includes('Creature'),
  )
  const untapped = mine.filter((object) => object.permanent?.tapped === false)
  return [...untapped, ...mine.filter((object) => !untapped.includes(object))].map((object) => object.id)
}

/** Everything seat 0 holds or controls on the battlefield: what a prompt marks as its own. */
export function ownOf(fixture: Fixture): ObjectId[] {
  const hand = must(fixture.view.players[0]?.hand.cards)
  const battlefield = fixture.view.objects.filter(
    (object) => object.zone === 'Battlefield' && object.controller === 0,
  )
  return [...hand, ...battlefield.map((object) => object.id)]
}

/** Options of the combat decision to face. */
export type CombatOptions = { DeclareAttackers: AttackOptions } | { DeclareBlockers: BlockOptions }

/** Renders the table of `seats` seats facing `kind` as decision `id`; every answer goes to a spy that accepts it. */
export function renderCombat(kind: CombatOptions, { seats = 2, id = 7 } = {}) {
  const fixture = fixtureOf(seats)
  const answer = vi.fn((_answer: Answer) => true)
  const table = renderTable({
    fixture,
    session: { answer },
    state: (state) => ({
      ...state,
      pending: { id, decision: { player: 0, kind: kind satisfies DecisionKind } },
    }),
  })
  const root = table.container
  const card = (object: ObjectId): HTMLElement =>
    must(root.querySelector<HTMLElement>(`button[data-object-id="${object}"]`), `card ${object}`)
  const hasCard = (object: ObjectId): boolean => root.querySelector(`button[data-object-id="${object}"]`) !== null
  const marks = (): Map<ObjectId, string> =>
    new Map(
      [...root.querySelectorAll<HTMLElement>('button[data-highlight]')].map((element) => [
        Number(element.getAttribute('data-object-id')),
        must(element.getAttribute('data-highlight')),
      ]),
    )
  const notes = (): Map<ObjectId, string> =>
    new Map(
      [...root.querySelectorAll<HTMLElement>('[data-slot="card-note"]')].map((element) => [
        Number(element.closest('button')?.getAttribute('data-object-id')),
        element.textContent ?? '',
      ]),
    )
  const text = (): string => must(root.querySelector('[data-slot="prompt-text"]')).textContent ?? ''
  const button = (name: string | RegExp): HTMLElement => screen.getByRole('button', { name })
  return { ...table, fixture, answer, card, hasCard, marks, notes, text, button }
}
