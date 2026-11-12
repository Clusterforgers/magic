// Goal: every detail the engine gives (`Why`) reads as one plain sentence that names the card and what is missing, a target on the stack adds the Full Control hint with the card's own verb, a printed condition is quoted from the rules text, a reason without detail falls back to its plain line, and a refused play becomes the notice.
import { describe, expect, it } from 'vitest'
import type { ObjectView, Unplayable, Why } from '@/protocol/generated'
import { refusalNotice, whyHint, whyLine } from './whyWords'
import type { Subject } from './whyWords'

const cast = (name: string, rulesText: string | null = null): Subject => ({ name, rulesText, doing: 'cast' })
const activate = (name: string, rulesText: string | null = null): Subject => ({
  name,
  rulesText,
  doing: 'activate',
})

describe('whyLine', () => {
  it.each<[string, Subject, Unplayable, Why, string]>([
    [
      'a copy spell with no creature spell',
      cast('Double Major', 'Copy target creature spell you control, except the copy isn’t legendary.'),
      'Targets',
      { Target: { needs: 'a creature spell you control on the stack', zone: 'Stack' } },
      'Double Major needs a target: a creature spell you control on the stack. Cast a creature first and keep priority (Full Control) to copy it.',
    ],
    [
      'a counterspell with an empty stack',
      cast('Counterspell', 'Counter target spell.'),
      'Targets',
      { Target: { needs: 'a spell on the stack', zone: 'Stack' } },
      'Counterspell needs a target: a spell on the stack. Wait until one is cast, then counter it while it is on the stack.',
    ],
    [
      'a kill spell with no creature',
      cast('Doom Blade', 'Destroy target nonblack creature.'),
      'Targets',
      { Target: { needs: 'a nonblack creature on the battlefield', zone: 'Battlefield' } },
      'Doom Blade needs a target: a nonblack creature on the battlefield.',
    ],
    [
      'a graveyard target',
      cast('Raise Dead'),
      'Targets',
      { Target: { needs: 'a creature card in your graveyard', zone: 'Graveyard' } },
      'Raise Dead needs a target: a creature card in your graveyard.',
    ],
    [
      'too little mana',
      cast('Grizzly Bears'),
      'Mana',
      { Mana: { cost: [{ Generic: 1 }, { Colored: 'Green' }], at_hand: 1 } },
      'Grizzly Bears costs {1}{G}; you have 1 mana.',
    ],
    [
      'the wrong colour',
      cast('Grizzly Bears'),
      'Mana',
      { Mana: { cost: [{ Generic: 1 }, { Colored: 'Green' }], at_hand: 2, missing: ['Green'] } },
      'Grizzly Bears costs {1}{G}; you have 2 mana. Nothing you have makes {G}.',
    ],
    [
      'sorcery speed',
      cast('Grizzly Bears'),
      'Timing',
      'SorcerySpeed',
      'Only in your main phase with an empty stack.',
    ],
    [
      'a sorcery-speed ability',
      activate('Slow'),
      'Timing',
      'SorcerySpeed',
      'Activate only as a sorcery: in your main phase with an empty stack.',
    ],
    [
      'a printed condition',
      cast('Combat Trick', 'Cast this spell only during combat.\nDraw a card.'),
      'Restricted',
      'OwnCondition',
      'Cast this spell only during combat.',
    ],
    [
      'an ability condition',
      activate('Hermit', '{T}: Draw a card. Activate only if you have no cards in hand.'),
      'Restricted',
      'OwnCondition',
      'Activate only if you have no cards in hand.',
    ],
    [
      'a permanent that forbids',
      cast('Opt'),
      'Restricted',
      { ForbiddenBy: { source: 7, name: 'Grand Abolisher' } },
      'Grand Abolisher stops you casting it now.',
    ],
    [
      'an effect that forbids',
      activate('Tapper'),
      'Restricted',
      { ForbiddenBy: {} },
      'An effect stops you activating it now.',
    ],
    [
      'split second',
      cast('Opt'),
      'Restricted',
      { SplitSecond: { spell: 3, name: 'Sudden Shock' } },
      'Sudden Shock has split second: nothing can be cast or activated until it resolves.',
    ],
    [
      'summoning sickness',
      activate('Mother of Runes'),
      'Cost',
      'SummoningSick',
      'Mother of Runes came under your control this turn, so its {T} abilities wait until your next turn.',
    ],
    ['a tapped source', activate('Mother of Runes'), 'Cost', 'Tapped', 'Mother of Runes is tapped.'],
    [
      'a spent limit',
      activate('Once'),
      'Restricted',
      'LimitSpent',
      'Already used as often as it allows this turn.',
    ],
    [
      'an additional cost',
      cast(
        "Altar's Reap",
        'As an additional cost to cast this spell, sacrifice a creature.\nDraw two cards.',
      ),
      'Cost',
      'OtherCost',
      "You can't pay its cost. As an additional cost to cast this spell, sacrifice a creature.",
    ],
    [
      'no way to cast',
      cast('Ancestral Vision'),
      'Cost',
      'NoWay',
      "Ancestral Vision can't be cast from here.",
    ],
  ])('%s', (_, subject, reason, why, expected) => {
    expect(whyLine(subject, reason, why)).toBe(expected)
  })

  it.each<Unplayable>(['Timing', 'LandPlayed', 'Mana', 'Targets', 'Cost', 'Restricted'])(
    'falls back to the plain line of %s without detail',
    (reason) => {
      const line = whyLine(cast('Card'), reason, undefined)
      expect(line).toMatch(/\.$/)
      expect(line).not.toContain('Card')
    },
  )
})

describe('whyHint', () => {
  it('speaks only of targets on the stack', () => {
    expect(
      whyHint(cast('Doom Blade'), {
        Target: { needs: 'a creature on the battlefield', zone: 'Battlefield' },
      }),
    ).toBe(null)
    expect(whyHint(cast('Opt'), 'SorcerySpeed')).toBe(null)
    expect(
      whyHint(cast('Redirect'), { Target: { needs: 'a spell you control on the stack', zone: 'Stack' } }),
    ).toBe('Cast a spell first and keep priority (Full Control) to target it.')
  })
})

describe('refusalNotice', () => {
  const objects = {
    5: { name: 'Double Major', rules_text: 'Copy target creature spell you control.' } as ObjectView,
  }

  it('says the detail when the server gives one', () => {
    expect(
      refusalNotice(
        {
          object: 5,
          reason: 'Targets',
          why: { Target: { needs: 'a creature spell you control on the stack', zone: 'Stack' } },
        },
        objects,
      ),
    ).toBe(
      'Double Major needs a target: a creature spell you control on the stack. Cast a creature first and keep priority (Full Control) to copy it.',
    )
  })

  it('names the card and the plain reason otherwise', () => {
    expect(refusalNotice({ object: 5, reason: 'Mana' }, objects)).toBe(
      "Double Major can't be played now. Not enough mana.",
    )
    expect(refusalNotice({ object: 9, ability: { object: 9, ability: 0 }, reason: 'Cost' }, objects)).toBe(
      "That card can't be activated now. A cost cannot be paid.",
    )
  })
})
