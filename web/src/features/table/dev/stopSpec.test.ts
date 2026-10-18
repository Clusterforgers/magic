// Goal: the spec of /dev/table?stops= reads side:step and side:step:keep items into the stops the server would hold (own side first, turn order, one per side and step), accepts the short and the long step names in any case, and skips whatever is not an item, so a mistyped address still shows the table.
import { describe, expect, it } from 'vitest'
import type { Step, Stop } from '@/protocol/generated'
import { parseDevSearch } from '../devSearch'
import { parseStopSpec } from './stopSpec'

const own = (step: Step, keep = false): Stop => ({ side: 'Own', step, keep })
const opponent = (step: Step, keep = false): Stop => ({ side: 'Opponent', step, keep })

describe('the items', () => {
  it.each([
    ['own:main1', [own('Main1')]],
    ['opp:end:keep', [opponent('End', true)]],
    ['own:main1,opp:end:keep', [own('Main1'), opponent('End', true)]],
    ['  OWN : Main1 , Opp:END:Keep ', [own('Main1'), opponent('End', true)]],
    ['opponent:upkeep', [opponent('Upkeep')]],
    ['own:upkeep:keep,own:draw,own:main1:keep', [own('Upkeep', true), own('Draw'), own('Main1', true)]],
  ] satisfies [string, Stop[]][])('reads %j', (spec, stops) => {
    expect(parseStopSpec(spec)).toEqual(stops)
  })

  it.each([
    ['upkeep', 'Upkeep'],
    ['draw', 'Draw'],
    ['main1', 'Main1'],
    ['begin', 'BeginCombat'],
    ['begincombat', 'BeginCombat'],
    ['attackers', 'DeclareAttackers'],
    ['declareattackers', 'DeclareAttackers'],
    ['blockers', 'DeclareBlockers'],
    ['declareblockers', 'DeclareBlockers'],
    ['firststrike', 'FirstStrikeDamage'],
    ['firststrikedamage', 'FirstStrikeDamage'],
    ['damage', 'CombatDamage'],
    ['combatdamage', 'CombatDamage'],
    ['endcombat', 'EndCombat'],
    ['main2', 'Main2'],
    ['end', 'End'],
    ['endstep', 'End'],
  ] satisfies [string, Step][])('calls the step %s %s, on both sides', (name, step) => {
    expect(parseStopSpec(`own:${name},opp:${name}:keep`)).toEqual([own(step), opponent(step, true)])
  })

  it('lists the stops as the server does, own side first and each side in turn order, whatever order they were named in', () => {
    expect(parseStopSpec('opp:end,own:main2,opp:upkeep,own:upkeep')).toEqual([
      own('Upkeep'),
      own('Main2'),
      opponent('Upkeep'),
      opponent('End'),
    ])
  })

  it('holds a stop named twice once, the last one winning', () => {
    expect(parseStopSpec('own:main1,own:main1:keep')).toEqual([own('Main1', true)])
    expect(parseStopSpec('own:main1:keep,own:main1')).toEqual([own('Main1')])
  })
})

describe('what is skipped', () => {
  it.each([
    '',
    ',',
    'own',
    'own:',
    ':main1',
    'both:main1',
    'own:combat',
    'own:untap',
    'own:cleanup',
    'own:main1:lasting',
    'own:main1:keep:extra',
    'own:main1:',
    'constructor:main1',
    'own:__proto__',
    'main1',
  ])('reads %j as no stop', (spec) => {
    expect(parseStopSpec(spec)).toEqual([])
  })

  it('keeps the good items next to the bad ones', () => {
    expect(parseStopSpec('own:untap,own:main1,nonsense,opp:end:keep,own:cleanup,opp:weird:keep')).toEqual([
      own('Main1'),
      opponent('End', true),
    ])
  })
})

describe('?stops= and ?fc= in the address', () => {
  it('keeps a spec that names a stop as it was written', () => {
    expect(parseDevSearch({ seats: 2, stops: 'own:main1,opp:end:keep' }).stops).toBe('own:main1,opp:end:keep')
    expect(parseDevSearch({ seats: 2, stops: 'zzz,own:draw' }).stops).toBe('zzz,own:draw')
  })

  it.each([undefined, '', 'nonsense', 'own:untap', 7, true, null, ['own:main1'], { own: 'main1' }])(
    'leaves out %j',
    (stops) => {
      expect(parseDevSearch({ seats: 2, stops }).stops).toBeUndefined()
    },
  )

  it.each(['next', 'held'])('keeps fc=%s', (fc) => {
    expect(parseDevSearch({ seats: 2, fc }).fc).toBe(fc)
  })

  it.each([undefined, '', 'off', 'Held', 'NextWindow', 'on', 1, true, null])('leaves out fc=%j', (fc) => {
    expect(parseDevSearch({ seats: 2, fc }).fc).toBeUndefined()
  })

  it('gives the same answer when the router validates its own answer again', () => {
    const once = parseDevSearch({
      seats: 4,
      turn: 'you',
      stops: 'own:draw,opp:end:keep',
      fc: 'held',
      slow: 4,
    })
    expect(parseDevSearch({ ...once })).toEqual(once)
  })

  it('goes along with the rest of the address', () => {
    expect(parseDevSearch({ seats: 4, turn: 'you', stops: 'own:draw', fc: 'held' })).toMatchObject({
      seats: 4,
      turn: 'you',
      stops: 'own:draw',
      fc: 'held',
    })
  })
})
