// Goal: combat damage, and only combat damage, becomes strikes: each attacker or blocker that deals damage in a damage step strikes the player, planeswalker or creature it damages (summed per target, waves for first strike, mutual when two trade), damage from spells, abilities or creatures outside the combat never strikes, the struck targets' flashes wait for the impact, and the path stops at the target's edge.
import { describe, expect, it } from 'vitest'
import {
  FOE,
  FOE_BEAR,
  FOE_IDOL,
  ME,
  miniView,
  momentOf,
  MY_BEAR,
  MY_GIANT,
  pub,
} from '@/test-utils/miniTable'
import type { AttackerView, CombatView, DamageTarget, Event, EventView, Step } from '@/protocol/generated'
import type { Moment } from './derive'
import type { Effect } from './model'
import {
  impactDelay,
  impactsOf,
  planStrikes,
  splitImpacts,
  STRIKE_GAP_PX,
  STRIKE_HIT,
  STRIKE_MS,
  strikeKeyframes,
} from './strikes'
import type { StrikePlan } from './strikes'
import { uprightBox } from './strikeLayer'

const NO_COMBAT: CombatView = { attackers: [], attackers_declared: false, blockers_declared: [] }

const attacking = (object: number, blockers: number[] = []): AttackerView => ({
  object,
  target: { Player: FOE },
  blocked: blockers.length > 0,
  blockers,
})

const combat = (...attackers: AttackerView[]): CombatView => ({
  attackers,
  attackers_declared: true,
  blockers_declared: [FOE],
})

/** The table in `step` with `combat`, someone holding priority unless `holder` is null. */
function at(step: Step, fight: CombatView, holder: number | null = ME): Moment {
  const view = miniView()
  return momentOf({
    ...view,
    turn: { ...view.turn, active: ME, step },
    priority: holder === null ? null : { holder, passed: [] },
    combat: fight,
  })
}

const damage = (source: number, target: DamageTarget, amount: number): Event => ({
  DamageDealt: { source, target, amount },
})
const toPlayer = (source: number, amount: number): Event => damage(source, { Player: FOE }, amount)
const toObject = (source: number, object: number, amount: number): Event =>
  damage(source, { Object: object }, amount)
const began = (step: Step): Event => ({ StepBegan: { step } })
const priority: Event = { PriorityGranted: { player: ME } }

const plan = (source: number, hits: StrikePlan['hits'], wave = 0, mutual = false): StrikePlan => ({
  source,
  hits,
  wave,
  mutual,
})

const STRIKE_CASES: [string, Moment, Moment, EventView[], StrikePlan[]][] = [
  [
    'an unblocked attacker strikes the player',
    at('DeclareBlockers', combat(attacking(MY_BEAR))),
    at('CombatDamage', combat(attacking(MY_BEAR))),
    pub(began('CombatDamage'), toPlayer(MY_BEAR, 2), priority),
    [plan(MY_BEAR, [{ target: { player: FOE }, amount: 2 }])],
  ],
  [
    'an attacker and its blocker strike each other',
    at('DeclareBlockers', combat(attacking(MY_GIANT, [FOE_BEAR]))),
    at('CombatDamage', combat(attacking(MY_GIANT, [FOE_BEAR]))),
    pub(began('CombatDamage'), toObject(MY_GIANT, FOE_BEAR, 4), toObject(FOE_BEAR, MY_GIANT, 2)),
    [
      plan(MY_GIANT, [{ target: { object: FOE_BEAR }, amount: 4 }], 0, true),
      plan(FOE_BEAR, [{ target: { object: MY_GIANT }, amount: 2 }], 0, true),
    ],
  ],
  [
    'a trampler strikes its blocker first and the rest lands on the player',
    at('DeclareBlockers', combat(attacking(MY_GIANT, [FOE_BEAR]))),
    at('CombatDamage', combat(attacking(MY_GIANT, [FOE_BEAR]))),
    pub(began('CombatDamage'), toObject(MY_GIANT, FOE_BEAR, 2), toPlayer(MY_GIANT, 3)),
    [
      plan(MY_GIANT, [
        { target: { object: FOE_BEAR }, amount: 2 },
        { target: { player: FOE }, amount: 3 },
      ]),
    ],
  ],
  [
    'an attacker at a planeswalker strikes the planeswalker, its damage summed',
    at('DeclareBlockers', combat(attacking(MY_BEAR))),
    at('CombatDamage', combat(attacking(MY_BEAR))),
    pub(began('CombatDamage'), toObject(MY_BEAR, FOE_IDOL, 1), toObject(MY_BEAR, FOE_IDOL, 2)),
    [plan(MY_BEAR, [{ target: { object: FOE_IDOL }, amount: 3 }])],
  ],
  [
    'first strike and regular damage in one batch strike in two waves',
    at('DeclareBlockers', combat(attacking(MY_BEAR), attacking(MY_GIANT))),
    at('CombatDamage', combat(attacking(MY_BEAR), attacking(MY_GIANT))),
    pub(
      began('FirstStrikeDamage'),
      toPlayer(MY_BEAR, 2),
      priority,
      began('CombatDamage'),
      toPlayer(MY_GIANT, 4),
    ),
    [
      plan(MY_BEAR, [{ target: { player: FOE }, amount: 2 }], 0),
      plan(MY_GIANT, [{ target: { player: FOE }, amount: 4 }], 1),
    ],
  ],
  [
    'a whole combat inside one batch still strikes',
    at('Main1', NO_COMBAT),
    at('Main2', NO_COMBAT),
    pub(
      { AttackersDeclared: { player: ME, attackers: [[MY_BEAR, { Player: FOE }]] } },
      began('CombatDamage'),
      toPlayer(MY_BEAR, 2),
      began('EndCombat'),
    ),
    [plan(MY_BEAR, [{ target: { player: FOE }, amount: 2 }])],
  ],
  [
    'a batch that resumes the damage step after an assignment strikes',
    at('CombatDamage', combat(attacking(MY_GIANT, [FOE_BEAR])), null),
    at('CombatDamage', combat(attacking(MY_GIANT, [FOE_BEAR]))),
    pub(toObject(MY_GIANT, FOE_BEAR, 4)),
    [plan(MY_GIANT, [{ target: { object: FOE_BEAR }, amount: 4 }])],
  ],
  [
    'an attacker that pings after priority does not strike',
    at('CombatDamage', combat(attacking(MY_BEAR))),
    at('CombatDamage', combat(attacking(MY_BEAR))),
    pub(toPlayer(MY_BEAR, 1)),
    [],
  ],
  [
    'damage after priority in the damage step does not strike',
    at('DeclareBlockers', combat(attacking(MY_BEAR))),
    at('CombatDamage', combat(attacking(MY_BEAR))),
    pub(began('CombatDamage'), priority, toPlayer(MY_BEAR, 1)),
    [],
  ],
  [
    'a creature outside the combat never strikes',
    at('DeclareBlockers', combat(attacking(MY_BEAR))),
    at('CombatDamage', combat(attacking(MY_BEAR))),
    pub(began('CombatDamage'), toPlayer(MY_GIANT, 3)),
    [],
  ],
  [
    'damage outside a damage step never strikes',
    at('Main1', combat(attacking(MY_BEAR)), null),
    at('Main1', combat(attacking(MY_BEAR))),
    pub(toPlayer(MY_BEAR, 2)),
    [],
  ],
]

describe('planStrikes', () => {
  it.each(STRIKE_CASES)('%s', (_name, before, after, events, expected) => {
    expect(planStrikes(before, after, events)).toEqual(expected)
  })
})

describe('impacts', () => {
  const plans = [
    plan(MY_GIANT, [
      { target: { object: FOE_BEAR }, amount: 2 },
      { target: { player: FOE }, amount: 3 },
    ]),
    plan(MY_BEAR, [{ target: { player: FOE }, amount: 1 }], 1),
  ]

  it('lands each wave a strike later, at the hit', () => {
    expect(impactDelay(0)).toBe(STRIKE_HIT * STRIKE_MS)
    expect(impactDelay(1, 2)).toBe((1 + STRIKE_HIT) * STRIKE_MS * 2)
    const { objects, seats } = impactsOf(plans)
    expect(objects.get(FOE_BEAR)).toBe(impactDelay(0))
    expect(seats.get(FOE)).toBe(impactDelay(1))
  })

  it.each<[string, Effect, 'now' | number]>([
    [
      'a struck creature flashes at the hit',
      { kind: 'damaged', object: FOE_BEAR, amount: 2 },
      impactDelay(0),
    ],
    ['a struck seat flashes at its last hit', { kind: 'damaged', seat: FOE, amount: 4 }, impactDelay(1)],
    ['a seat that gains life shows it now', { kind: 'healed', seat: FOE, amount: 1 }, 'now'],
    ['an unstruck creature flashes now', { kind: 'damaged', object: MY_BEAR, amount: 1 }, 'now'],
    ['an entry shows now', { kind: 'entered', object: FOE_BEAR }, 'now'],
  ])('%s', (_name, effect, when) => {
    const { now, later } = splitImpacts([effect], impactsOf(plans))
    if (when === 'now') {
      expect(now).toEqual([effect])
      expect(later.size).toBe(0)
    } else {
      expect(now).toEqual([])
      expect(later.get(when)).toEqual([effect])
    }
  })
})

/** The x and y a keyframe's transform moves to. */
const moveOf = (frame: Keyframe): [number, number] => {
  const match = /translate\((-?\d+)px, (-?\d+)px\)/.exec(String(frame.transform))
  return [Number(match?.[1]), Number(match?.[2])]
}

describe('strikeKeyframes', () => {
  const from = { x: 0, y: 300, width: 100, height: 140 }
  const to = { x: 0, y: 0, width: 100, height: 140 }

  it.each<[string, boolean, number]>([
    ['stops short of the target edge', false, 300 - 140 - STRIKE_GAP_PX],
    ['meets a striker that strikes back halfway', true, (300 - 140 - STRIKE_GAP_PX) / 2],
  ])('%s', (_name, mutual, travel) => {
    const frames = strikeKeyframes(from, to, mutual)
    expect(frames.map((frame) => frame.offset)).toEqual([0, 0.2, STRIKE_HIT, 1])
    expect(moveOf(frames[0] ?? {})).toEqual([0, 300])
    expect(moveOf(frames[1] ?? {})).toEqual([0, 310])
    expect(moveOf(frames[2] ?? {})).toEqual([0, Math.round(300 - travel)])
    expect(moveOf(frames.at(-1) ?? {})).toEqual([0, 300])
  })

  it('never passes through a target it already touches', () => {
    const frames = strikeKeyframes(from, { ...from, y: 250 })
    expect(moveOf(frames[2] ?? {})).toEqual([0, 300])
  })

  it('turns a tapped box back upright about its centre', () => {
    expect(uprightBox({ x: 0, y: 0, width: 140, height: 100 })).toEqual({
      x: 20,
      y: -20,
      width: 100,
      height: 140,
    })
    expect(uprightBox(from)).toBe(from)
  })
})
