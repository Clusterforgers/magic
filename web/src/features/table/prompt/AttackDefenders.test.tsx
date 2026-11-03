/** Goal: at 2 to 4 opponents a creature clicked, then a defender of any kind (an opponent's pod, board or tile, a planeswalker, a battle) attacks that defender, the next creatures follow it, a defender the creature cannot attack leaves it where it is, a battle is named as one, and a creature a rule requires to attack (goad) starts in the attack, stays after Clear and leaves no empty attack to choose. */
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { Fixture } from '@/features/table/dev/fixture'
import type { AttackerOption, AttackTarget, ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderCombat } from '@/test-utils/combatTable'

type Kind = 'pod' | 'board' | 'tile' | 'planeswalker' | 'battle'

/** A permanent of `seat` that is no creature, standing in for a planeswalker or battle the fixture does not have. */
const permanentOf = (fixture: Fixture, seat: number): ObjectId =>
  must(
    fixture.view.objects.find(
      (object) =>
        object.zone === 'Battlefield' && object.controller === seat && !object.types.includes('Creature'),
    ),
  ).id

/** The fixture with `id` turned into a battle. */
const withBattle = (fixture: Fixture, id: ObjectId): Fixture => ({
  ...fixture,
  view: {
    ...fixture.view,
    objects: fixture.view.objects.map((object) =>
      object.id === id ? { ...object, types: ['Battle'], subtypes: ['Siege'] } : object,
    ),
  },
})

const players = (seats: number): AttackTarget[] =>
  Array.from({ length: seats - 1 }, (_, index): AttackTarget => ({ Player: index + 1 }))

interface Setup {
  fixture: Fixture
  defenders: AttackTarget[]
  /** What the defender clicked is, as the answer names it. */
  target: AttackTarget
  /** Clicks the defender on the rendered table. */
  click: (t: ReturnType<typeof renderCombat>) => Promise<void>
  focus: boolean
}

function setup(kind: Kind, seats: number): Setup {
  const last = seats - 1
  const base = fixtureOf(seats)
  const seatClick =
    (selector: string) =>
    async (t: ReturnType<typeof renderCombat>): Promise<void> =>
      t.user.click(must(t.container.querySelector<HTMLElement>(selector)))
  switch (kind) {
    case 'pod':
      return {
        fixture: base,
        defenders: players(seats),
        target: { Player: last },
        click: seatClick(`section[data-seat="${last}"]`),
        focus: false,
      }
    case 'board':
      return {
        fixture: base,
        defenders: players(seats),
        target: { Player: last },
        click: seatClick(`[data-seat-area="${last}"]`),
        focus: false,
      }
    case 'tile':
      return {
        fixture: base,
        defenders: players(seats),
        target: { Player: last },
        click: seatClick(`[data-tile-seat="${last}"]`),
        focus: true,
      }
    case 'planeswalker': {
      const walker = permanentOf(base, last)
      return {
        fixture: base,
        defenders: [...players(seats), { Planeswalker: walker }],
        target: { Planeswalker: walker },
        click: (t) => t.user.click(t.card(walker)),
        focus: false,
      }
    }
    case 'battle': {
      // A Siege its caster controls and an opponent protects: the viewer's own permanent is the defender.
      const battle = permanentOf(base, 0)
      return {
        fixture: withBattle(base, battle),
        defenders: [...players(seats), { Planeswalker: battle }],
        target: { Planeswalker: battle },
        click: (t) => t.user.click(t.card(battle)),
        focus: false,
      }
    }
  }
}

const render = (s: Setup, attackers: AttackerOption[], seats: number) =>
  renderCombat(
    { DeclareAttackers: { attackers } },
    // Five seats open in focus mode: the last opponent is the one drawn large unless its tile is the one clicked.
    { seats, fixture: s.fixture, options: s.focus ? { mode: 'focus', focus: 1 } : { focus: seats - 1 } },
  )

const KINDS: Kind[] = ['pod', 'board', 'tile', 'planeswalker', 'battle']
const TABLE = KINDS.flatMap((kind) => [3, 4, 5].map((seats) => ({ kind, seats, opponents: seats - 1 })))

describe('a creature, then a defender', () => {
  it.each(TABLE)('with $opponents opponents a click on a $kind attacks it', async ({ kind, seats }) => {
    const s = setup(kind, seats)
    const [a, b, c] = creaturesOf(s.fixture, 0) as [ObjectId, ObjectId, ObjectId]
    const t = render(
      s,
      [a, b, c].map((object) => ({ object, defenders: s.defenders })),
      seats,
    )
    await t.user.click(t.card(a))
    await s.click(t)
    // The defender clicked is where the next creature goes, and the creature before it stays.
    await t.user.click(t.card(b))
    await t.user.click(t.button('Confirm attack'))
    expect(t.answer.mock.calls).toEqual([
      [
        {
          DeclareAttackers: [
            [a, s.target],
            [b, s.target],
          ],
        },
      ],
    ])
  })

  it.each([3, 4, 5])(
    'with %i seats a defender the held creature cannot attack leaves it at its own',
    async (seats) => {
      const s = setup('pod', seats)
      const [a, b] = creaturesOf(s.fixture, 0) as [ObjectId, ObjectId]
      const t = render(
        s,
        [
          { object: a, defenders: [{ Player: 1 }] },
          { object: b, defenders: s.defenders },
        ],
        seats,
      )
      await t.user.click(t.card(a))
      await s.click(t)
      await t.user.click(t.card(b))
      await t.user.click(t.button('Confirm attack'))
      expect(t.answer.mock.calls).toEqual([
        [
          {
            DeclareAttackers: [
              [a, { Player: 1 }],
              [b, s.target],
            ],
          },
        ],
      ])
    },
  )

  it('glows on the held creature until a defender is clicked', async () => {
    const s = setup('pod', 4)
    const [a] = creaturesOf(s.fixture, 0) as [ObjectId]
    const t = render(s, [{ object: a, defenders: s.defenders }], 4)
    await t.user.click(t.card(a))
    expect(t.card(a)).toHaveAttribute('data-cue', 'active')
    expect(t.text()).toMatch(/Click a player, planeswalker or battle to send .+ there\.$/)
    await s.click(t)
    expect(t.card(a)).not.toHaveAttribute('data-cue')
    expect(t.text()).toBe('1 attacker at Dee.')
  })

  it('names a battle as a battle in the list', () => {
    const s = setup('battle', 4)
    const [a] = creaturesOf(s.fixture, 0) as [ObjectId]
    render(s, [{ object: a, defenders: s.defenders }], 4)
    const labels = [...screen.getByRole('combobox', { name: 'Attack at' }).querySelectorAll('option')].map(
      (option) => option.textContent,
    )
    expect(labels.at(-1)).toMatch(/, battle$/)
  })
})

describe('a creature a rule requires to attack', () => {
  const goaded = (seats: number) => {
    const fixture = fixtureOf(seats)
    const [a, b] = creaturesOf(fixture, 0) as [ObjectId, ObjectId]
    // Goaded by Bo: it may attack every other opponent.
    const t = renderCombat(
      {
        DeclareAttackers: {
          attackers: [
            { object: a, defenders: players(seats).slice(1) },
            { object: b, defenders: players(seats) },
          ],
          required: [a],
        },
      },
      { seats },
    )
    return { a, b, t }
  }

  it.each([3, 4, 5])('with %i seats starts in the attack at a player it may attack', async (seats) => {
    const { a, t } = goaded(seats)
    expect(t.marks().get(a)).toBe('selected')
    expect(screen.queryByRole('button', { name: 'No attack' })).toBeNull()
    await t.user.click(t.button('Confirm attack'))
    expect(t.answer.mock.calls).toEqual([[{ DeclareAttackers: [[a, { Player: 2 }]] }]])
  })

  it('stays after Clear, is marked Must attack and cannot be taken out', async () => {
    const { a, b, t } = goaded(4)
    await t.user.click(t.card(b))
    await t.user.click(t.button('Clear'))
    expect(t.marks().get(a)).toBe('selected')
    expect(t.marks().get(b)).toBe('legal')
    expect(t.button('Clear')).toBeDisabled()
    await t.user.click(t.card(a))
    expect(t.marks().get(a)).toBe('selected')
    expect(t.card(a)).toHaveTextContent('Must attack')
    expect(t.text()).not.toMatch(/must attack if able/)
  })
})
