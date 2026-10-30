// Goal: a batch becomes exactly the transient effects it deserves, whatever mix of attacks, blocks, damage, deaths, entries and life changes it holds: derived from the game before and after it plus its events (so a whole combat inside one batch still shows), with nothing for a batch that changed nothing worth showing, for damage that is merely cleared, or for permanents that left the game instead of dying, and without touching its inputs; and it finds the attachments that changed host between two moments (equipped, moved, unattached, put on a player) and none for one that stays, enters attached or whose host changes hands.
import { describe, expect, it } from 'vitest'
import { clone, deepFreeze, must } from '@/test-utils/batches'
import {
  afterChange,
  FOE,
  FOE_BEAR,
  FOE_FOREST,
  FOE_GOLEM,
  FOE_IDOL,
  FOE_IMP,
  FOE_SPELL,
  ME,
  miniView,
  momentOf,
  MY_BEAR,
  MY_FOREST,
  MY_GIANT,
  MY_HAND_CARD,
  MY_WALL,
  pub,
} from '@/test-utils/miniTable'
import type { Draft } from '@/test-utils/miniTable'
import type { AttackerView, Event, EventView } from '@/protocol/generated'
import { deriveAttaches, deriveEffects, MAX_GHOSTS } from './derive'
import type { Moment } from './derive'
import type { Effect } from './model'

const base = momentOf(miniView())

const attacker = (object: number, target: number, blockers: number[] = []): AttackerView => ({
  object,
  target: { Player: target },
  blocked: blockers.length > 0,
  blockers,
})

const tap =
  (...ids: number[]) =>
  (draft: Draft): void => {
    for (const id of ids) {
      draft.edit(id, (object) => {
        must(object.permanent).tapped = true
      })
    }
  }

const mark =
  (id: number, damage: number) =>
  (draft: Draft): void => {
    draft.edit(id, (object) => {
      must(object.permanent).damage = damage
    })
  }

/** Every effect of the batch that `change` and `events` make out of `from`. */
function effectsOf(change: (draft: Draft) => void, events: EventView[] = [], from: Moment = base): Effect[] {
  return deriveEffects(from, afterChange(from, change, events), events)
}

const declare = (player: number, ids: number[], target: number): Event => ({
  AttackersDeclared: { player, attackers: ids.map((id) => [id, { Player: target }]) },
})

const hit = (source: number, target: number, amount: number): Event => ({
  DamageDealt: { source, target: { Object: target }, amount },
})

const hitPlayer = (source: number, target: number, amount: number): Event => ({
  DamageDealt: { source, target: { Player: target }, amount },
})

const died = (object: number): Event => ({ CreatureDied: { object, cause: 'LethalDamage' } })

/** The table after the opponent attacked with their Bear and Golem. */
const attacked = afterChange(base, (draft) => {
  draft.view.combat = {
    attackers: [attacker(FOE_BEAR, ME), attacker(FOE_GOLEM, ME)],
    attackers_declared: true,
    blockers_declared: [],
  }
  tap(FOE_BEAR, FOE_GOLEM)(draft)
})

describe('attacks', () => {
  it.each<[string, number, number[], number, boolean]>([
    ['the viewer attacks with one creature', ME, [MY_BEAR], FOE, true],
    ['the viewer attacks with two creatures', ME, [MY_BEAR, MY_GIANT], FOE, true],
    ['the opponent attacks the viewer', FOE, [FOE_BEAR], ME, false],
    ['the opponent attacks with everything', FOE, [FOE_BEAR, FOE_GOLEM, FOE_IMP], ME, false],
  ])('%s', (_name, player, ids, target, own) => {
    const declared = ids.map((id) => attacker(id, target))
    const effects = effectsOf(
      (draft) => {
        draft.view.combat = { attackers: declared, attackers_declared: true, blockers_declared: [] }
        tap(...ids)(draft)
      },
      pub(declare(player, ids, target)),
    )
    expect(effects).toEqual(ids.map((object) => ({ kind: 'attacked', object, own })))
  })

  it('finds the attackers in the combat alone when the batch carries no event for them', () => {
    const effects = effectsOf((draft) => {
      draft.view.combat = {
        attackers: [attacker(MY_BEAR, FOE)],
        attackers_declared: true,
        blockers_declared: [],
      }
    })
    expect(effects).toEqual([{ kind: 'attacked', object: MY_BEAR, own: true }])
  })

  it('finds an attack in the events alone when the whole combat ran inside the batch and left none behind', () => {
    const events = pub(declare(FOE, [FOE_BEAR], ME), hitPlayer(FOE_BEAR, ME, 2), 'CombatEnded')
    const effects = effectsOf((draft) => draft.life(ME, 18), events)
    expect(effects).toEqual([
      { kind: 'damaged', seat: ME, amount: 2 },
      { kind: 'attacked', object: FOE_BEAR, own: false },
    ])
  })

  it('does not repeat an attacker that was already attacking before the batch', () => {
    expect(effectsOf(mark(FOE_BEAR, 0), [], attacked)).toEqual([])
    expect(effectsOf(tap(MY_BEAR), pub('CombatEnded'), attacked)).toEqual([])
  })

  it('shows only the newcomer when another creature joins an attack already declared', () => {
    const effects = effectsOf(
      (draft) => {
        draft.view.combat = {
          attackers: [attacker(FOE_BEAR, ME), attacker(FOE_GOLEM, ME), attacker(FOE_IMP, ME)],
          attackers_declared: true,
          blockers_declared: [],
        }
      },
      [],
      attacked,
    )
    expect(effects).toEqual([{ kind: 'attacked', object: FOE_IMP, own: false }])
  })

  it('gives an attacker that died in the same batch a ghost and no lunge', () => {
    const effects = effectsOf(
      (draft) => {
        draft.die(FOE_IMP)
      },
      pub(declare(FOE, [FOE_IMP], ME), hit(MY_WALL, FOE_IMP, 1), died(FOE_IMP)),
    )
    expect(effects.map((effect) => effect.kind)).toEqual(['died'])
  })
})

describe('blocks', () => {
  const blocked = (blockers: [number, number][]) => (draft: Draft) => {
    draft.view.combat = {
      attackers: [
        attacker(
          FOE_BEAR,
          ME,
          blockers.filter(([, victim]) => victim === FOE_BEAR).map(([blocker]) => blocker),
        ),
        attacker(
          FOE_GOLEM,
          ME,
          blockers.filter(([, victim]) => victim === FOE_GOLEM).map(([blocker]) => blocker),
        ),
      ],
      attackers_declared: true,
      blockers_declared: [ME],
    }
  }
  const declareBlocks = (blocks: [number, number][]): Event => ({ BlockersDeclared: { player: ME, blocks } })

  it.each<[string, [number, number][]]>([
    ['one blocker', [[MY_GIANT, FOE_BEAR]]],
    [
      'two blockers on different attackers',
      [
        [MY_GIANT, FOE_BEAR],
        [MY_WALL, FOE_GOLEM],
      ],
    ],
    [
      'two blockers on one attacker',
      [
        [MY_GIANT, FOE_GOLEM],
        [MY_WALL, FOE_GOLEM],
      ],
    ],
  ])('shows %s leaning, once each, by object id', (_name, blocks) => {
    const effects = effectsOf(blocked(blocks), pub(declareBlocks(blocks)), attacked)
    const ids = blocks.map(([blocker]) => blocker).sort((a, b) => a - b)
    expect(effects).toEqual(ids.map((object) => ({ kind: 'blocked', object, own: true })))
  })

  it('finds the blockers in the combat alone', () => {
    const effects = effectsOf(blocked([[MY_GIANT, FOE_BEAR]]), [], attacked)
    expect(effects).toEqual([{ kind: 'blocked', object: MY_GIANT, own: true }])
  })

  it('marks an opponent’s blocker as not the viewer’s own', () => {
    const mine = afterChange(base, (draft) => {
      draft.view.combat = {
        attackers: [attacker(MY_BEAR, FOE)],
        attackers_declared: true,
        blockers_declared: [],
      }
    })
    const events = pub({ BlockersDeclared: { player: FOE, blocks: [[FOE_GOLEM, MY_BEAR]] } })
    const effects = effectsOf(
      (draft) => {
        draft.view.combat = {
          attackers: [attacker(MY_BEAR, FOE, [FOE_GOLEM])],
          attackers_declared: true,
          blockers_declared: [FOE],
        }
      },
      events,
      mine,
    )
    expect(effects).toEqual([{ kind: 'blocked', object: FOE_GOLEM, own: false }])
  })

  it('does not repeat a blocker that was already blocking', () => {
    const blockedAlready = afterChange(attacked, blocked([[MY_GIANT, FOE_BEAR]]))
    expect(effectsOf(blocked([[MY_GIANT, FOE_BEAR]]), [], blockedAlready)).toEqual([])
  })
})

describe('damage to permanents', () => {
  it('shows damage marked, with the amount of the event', () => {
    const effects = effectsOf(mark(MY_GIANT, 2), pub(hit(FOE_GOLEM, MY_GIANT, 2)))
    expect(effects).toEqual([{ kind: 'damaged', object: MY_GIANT, amount: 2 }])
  })

  it('adds up several hits in one batch', () => {
    const effects = effectsOf(mark(MY_WALL, 3), pub(hit(FOE_BEAR, MY_WALL, 2), hit(FOE_IMP, MY_WALL, 1)))
    expect(effects).toEqual([{ kind: 'damaged', object: MY_WALL, amount: 3 }])
  })

  it('takes the amount from the damage newly marked when no event names it', () => {
    expect(effectsOf(mark(MY_GIANT, 2))).toEqual([{ kind: 'damaged', object: MY_GIANT, amount: 2 }])
    const hurt = afterChange(base, mark(MY_GIANT, 1))
    expect(effectsOf(mark(MY_GIANT, 3), [], hurt)).toEqual([{ kind: 'damaged', object: MY_GIANT, amount: 2 }])
  })

  it('shows nothing for damage that the cleanup step wipes', () => {
    const hurt = afterChange(base, mark(MY_GIANT, 2))
    expect(effectsOf(mark(MY_GIANT, 0), pub('DamageCleared'), hurt)).toEqual([])
  })

  it('shows nothing for damage taken off but not all of it', () => {
    const hurt = afterChange(base, mark(MY_WALL, 3))
    expect(effectsOf(mark(MY_WALL, 1), [], hurt)).toEqual([])
  })

  it('shows damage on several permanents at once, by object id', () => {
    const effects = effectsOf(
      (draft) => {
        mark(FOE_GOLEM, 3)(draft)
        mark(MY_BEAR, 1)(draft)
      },
      pub(hit(FOE_GOLEM, MY_BEAR, 1), hit(MY_GIANT, FOE_GOLEM, 3)),
    )
    expect(effects).toEqual([
      { kind: 'damaged', object: MY_BEAR, amount: 1 },
      { kind: 'damaged', object: FOE_GOLEM, amount: 3 },
    ])
  })

  it('leaves the amount of a creature that died to its ghost, which carries it', () => {
    const effects = effectsOf(
      (draft) => {
        draft.die(MY_BEAR)
      },
      pub(hit(FOE_GOLEM, MY_BEAR, 4), died(MY_BEAR)),
    )
    expect(effects).toHaveLength(1)
    expect(effects[0]).toMatchObject({ kind: 'died', object: MY_BEAR, damage: 4 })
  })

  it('ignores damage dealt to something that is not on the battlefield', () => {
    expect(effectsOf(() => undefined, pub(hit(FOE_GOLEM, FOE_SPELL, 2)))).toEqual([])
  })
})

describe('permanents that die', () => {
  it('leaves a ghost of the creature as it was drawn, with where it stood', () => {
    const effects = effectsOf(
      (draft) => {
        draft.die(MY_BEAR)
      },
      pub(died(MY_BEAR)),
    )
    expect(effects).toEqual([
      {
        kind: 'died',
        object: MY_BEAR,
        seat: ME,
        row: 'creatures',
        snapshot: must(base.objectsById[MY_BEAR]),
        before: MY_GIANT,
        damage: 0,
      },
    ])
  })

  it.each<[string, number[], Record<number, number | null>]>([
    ['the first of a row', [MY_BEAR], { [MY_BEAR]: MY_GIANT }],
    ['the middle of a row', [MY_GIANT], { [MY_GIANT]: MY_WALL }],
    ['the end of a row', [MY_WALL], { [MY_WALL]: null }],
    ['the first two of a row', [MY_BEAR, MY_GIANT], { [MY_BEAR]: MY_WALL, [MY_GIANT]: MY_WALL }],
    ['a whole row', [MY_BEAR, MY_GIANT, MY_WALL], { [MY_BEAR]: null, [MY_GIANT]: null, [MY_WALL]: null }],
    ['the opponent’s first creature', [FOE_BEAR], { [FOE_BEAR]: FOE_GOLEM }],
  ])('puts the ghost of %s back in front of the card that stays after it', (_name, ids, anchors) => {
    const effects = effectsOf(
      (draft) => {
        for (const id of ids) draft.die(id)
      },
      pub(...ids.map(died)),
    )
    expect(effects.map((effect) => (effect.kind === 'died' ? [effect.object, effect.before] : null))).toEqual(
      ids.map((id) => [id, anchors[id]]),
    )
  })

  it('sorts a creature, a land and another permanent into their own rows', () => {
    const effects = effectsOf(
      (draft) => {
        draft.die(FOE_GOLEM)
        draft.die(FOE_FOREST)
        draft.die(FOE_IDOL)
      },
      pub(died(FOE_GOLEM)),
    )
    const rows = effects.map((effect) =>
      effect.kind === 'died' ? [effect.object, effect.seat, effect.row] : null,
    )
    expect(rows).toEqual([
      [FOE_GOLEM, FOE, 'creatures'],
      [FOE_FOREST, FOE, 'lands'],
      [FOE_IDOL, FOE, 'other'],
    ])
  })

  it('counts a permanent that is not a creature as dead from where it lies, with no creature event', () => {
    const effects = effectsOf((draft) => {
      draft.die(FOE_IDOL)
    })
    expect(effects).toHaveLength(1)
    expect(effects[0]).toMatchObject({ kind: 'died', object: FOE_IDOL, row: 'other', before: null })
  })

  it('counts a creature that the viewer can no longer see as dead when the events say it died (a token)', () => {
    const effects = effectsOf(
      (draft) => {
        draft.leave(FOE_IMP)
      },
      pub(died(FOE_IMP)),
    )
    expect(effects).toHaveLength(1)
    expect(effects[0]).toMatchObject({ kind: 'died', object: FOE_IMP, before: null })
  })

  it.each<[string, (draft: Draft) => void]>([
    [
      'goes back to its owner’s hand',
      (draft) => {
        draft.edit(MY_BEAR, (object) => {
          object.zone = { Hand: ME }
          object.permanent = null
        })
        draft.view.battlefield = {
          count: 8,
          cards: must(draft.view.battlefield.cards).filter((id) => id !== MY_BEAR),
        }
      },
    ],
    [
      'is exiled',
      (draft) => {
        draft.edit(MY_BEAR, (object) => {
          object.zone = 'Exile'
          object.permanent = null
        })
        draft.view.battlefield = {
          count: 8,
          cards: must(draft.view.battlefield.cards).filter((id) => id !== MY_BEAR),
        }
      },
    ],
    [
      'is taken by the opponent',
      (draft) => {
        draft.edit(MY_BEAR, (object) => {
          object.controller = FOE
        })
      },
    ],
  ])('shows no ghost for a permanent that %s', (_name, change) => {
    expect(effectsOf(change)).toEqual([])
  })

  it('leaves a ghost for the controller’s side when the controller is not the owner', () => {
    const stolen = afterChange(base, (draft) => {
      draft.edit(MY_BEAR, (object) => {
        object.controller = FOE
      })
    })
    const effects = effectsOf(
      (draft) => {
        draft.die(MY_BEAR)
      },
      pub(died(MY_BEAR)),
      stolen,
    )
    expect(effects[0]).toMatchObject({ kind: 'died', object: MY_BEAR, seat: FOE, row: 'creatures' })
  })

  it(`keeps at most ${MAX_GHOSTS} ghosts of one batch`, () => {
    const crowd = afterChange(base, (draft) => {
      for (let n = 0; n < 40; n += 1) {
        const template = must(draft.view.objects.find((object) => object.id === MY_BEAR))
        draft.add({ ...clone(template), id: 100 + n, name: `Token ${n}` })
        draft.view.battlefield = {
          count: draft.view.battlefield.count + 1,
          cards: [...must(draft.view.battlefield.cards), 100 + n],
        }
      }
    })
    const ids = Array.from({ length: 40 }, (_, n) => 100 + n)
    const effects = effectsOf(
      (draft) => {
        for (const id of ids) draft.die(id)
      },
      pub(...ids.map(died)),
      crowd,
    )
    expect(effects).toHaveLength(MAX_GHOSTS)
    expect(effects.every((effect) => effect.kind === 'died')).toBe(true)
  })
})

describe('life totals', () => {
  it.each<[number, number, 'damaged' | 'healed' | null, number]>([
    [20, 15, 'damaged', 5],
    [20, 19, 'damaged', 1],
    [20, 0, 'damaged', 20],
    [20, -3, 'damaged', 23],
    [20, 25, 'healed', 5],
    [20, 21, 'healed', 1],
    [20, 20, null, 0],
  ])('from %i to %i is %s', (from, to, kind, amount) => {
    const start = afterChange(base, (draft) => draft.life(FOE, from))
    const effects = effectsOf((draft) => draft.life(FOE, to), [], start)
    expect(effects).toEqual(kind === null ? [] : [{ kind, seat: FOE, amount }])
  })

  it('reports each seat that changed, by seat, and leaves the others alone', () => {
    const effects = effectsOf((draft) => {
      draft.life(ME, 23)
      draft.life(FOE, 17)
    })
    expect(effects).toEqual([
      { kind: 'healed', seat: ME, amount: 3 },
      { kind: 'damaged', seat: FOE, amount: 3 },
    ])
  })

  it('takes the net change when damage and gain land in one batch', () => {
    const effects = effectsOf((draft) => draft.life(ME, 19), pub(hitPlayer(FOE_BEAR, ME, 3)))
    expect(effects).toEqual([{ kind: 'damaged', seat: ME, amount: 1 }])
  })

  it('does not read a life total out of the damage events', () => {
    expect(effectsOf(() => undefined, pub(hitPlayer(FOE_BEAR, ME, 3)))).toEqual([])
  })
})

describe('permanents that enter', () => {
  it.each<[string, (draft: Draft) => void]>([
    ['a creature spell resolving', (draft) => draft.enter(FOE_SPELL)],
    ['a land played from the viewer’s hand', (draft) => draft.enter(MY_HAND_CARD)],
    [
      'a permanent the viewer had never seen (played from a hidden hand)',
      (draft) => {
        const land = must(draft.view.objects.find((object) => object.id === FOE_FOREST))
        draft.add({ ...clone(land), id: 200 })
        draft.view.battlefield = {
          count: draft.view.battlefield.count + 1,
          cards: [...must(draft.view.battlefield.cards), 200],
        }
      },
    ],
  ])('shows %s', (_name, change) => {
    const effects = effectsOf(change)
    expect(effects).toHaveLength(1)
    expect(effects[0]).toMatchObject({ kind: 'entered' })
  })

  it('names the object that entered', () => {
    expect(effectsOf((draft) => draft.enter(FOE_SPELL))).toEqual([{ kind: 'entered', object: FOE_SPELL }])
  })

  it('does not count a permanent that only changed, or changed hands', () => {
    expect(effectsOf(tap(MY_FOREST))).toEqual([])
    expect(
      effectsOf((draft) =>
        draft.edit(MY_BEAR, (object) => {
          object.controller = FOE
        }),
      ),
    ).toEqual([])
  })
})

describe('a batch with several effects', () => {
  // Their Bear and Golem are attacking. We block the Bear with the Giant; the Golem gets through. Combat damage:
  // the Giant kills the Bear and takes 2, the Golem hits us for 4.
  const events = pub(
    { BlockersDeclared: { player: ME, blocks: [[MY_GIANT, FOE_BEAR]] } },
    hit(MY_GIANT, FOE_BEAR, 3),
    hit(FOE_BEAR, MY_GIANT, 2),
    hitPlayer(FOE_GOLEM, ME, 4),
    died(FOE_BEAR),
    'CombatEnded',
  )
  const change = (draft: Draft): void => {
    draft.view.combat = { attackers: [], attackers_declared: false, blockers_declared: [] }
    mark(MY_GIANT, 2)(draft)
    draft.die(FOE_BEAR)
    draft.life(ME, 16)
  }

  it('lists life, blocks, damage and deaths in that order', () => {
    const effects = effectsOf(change, events, attacked)
    expect(effects.map((effect) => effect.kind)).toEqual(['damaged', 'blocked', 'damaged', 'died'])
    expect(effects).toMatchObject([
      { kind: 'damaged', seat: ME, amount: 4 },
      { kind: 'blocked', object: MY_GIANT, own: true },
      { kind: 'damaged', object: MY_GIANT, amount: 2 },
      { kind: 'died', object: FOE_BEAR, seat: FOE, damage: 3, before: FOE_GOLEM },
    ])
  })

  it('gives the same effects again for the same batch', () => {
    expect(effectsOf(change, events, attacked)).toEqual(effectsOf(change, events, attacked))
  })

  it('reads the viewer from the table: the other seat’s creatures are no longer “own”', () => {
    const theirs: Moment = momentOf({ ...clone(attacked.view), viewer: FOE })
    const effects = effectsOf(change, events, theirs)
    expect(effects.filter((effect) => effect.kind === 'blocked')).toEqual([
      { kind: 'blocked', object: MY_GIANT, own: false },
    ])
  })
})

describe('a batch with nothing to show', () => {
  it('has no effects when nothing changed at all', () => {
    expect(deriveEffects(base, base, [])).toEqual([])
  })

  it.each<[string, (draft: Draft) => void, EventView[]]>([
    [
      'the turn moving on a step',
      (draft) => {
        draft.view.turn = { ...draft.view.turn, step: 'Main2' }
      },
      pub({ StepBegan: { step: 'Main2' } }),
    ],
    ['priority passing', () => undefined, pub({ PriorityPassed: { player: ME } })],
    ['a land tapping for mana', tap(MY_FOREST), pub({ PermanentTapped: { object: MY_FOREST } })],
    [
      'mana floating',
      (draft) => {
        must(draft.view.players[ME]).mana = { counts: [0, 0, 0, 0, 1, 0] }
      },
      pub({ ManaAdded: { player: ME, mana: 'Green', source: MY_FOREST } }),
    ],
    [
      'a card drawn that the viewer cannot see',
      () => undefined,
      [{ Hidden: { CardDrawn: { player: FOE } } }, { Hidden: 'ObjectLeftGame' }],
    ],
  ])('has no effects for %s', (_name, change, events) => {
    expect(effectsOf(change, events)).toEqual([])
  })
})

describe('a player eliminated', () => {
  const nearlyDead = afterChange(base, (draft) => draft.life(FOE, 3))
  const lethal = pub(hitPlayer(MY_GIANT, FOE, 3), { PlayerLost: { player: FOE, reason: 'ZeroLife' } })
  const leaves = (draft: Draft): void => {
    draft.life(FOE, 0)
    must(draft.view.players[FOE]).lost = 'ZeroLife'
    for (const id of [FOE_BEAR, FOE_GOLEM, FOE_IMP, FOE_FOREST, FOE_IDOL]) draft.leave(id)
  }

  it('shows the life lost and nothing for the permanents that left the game with them', () => {
    expect(effectsOf(leaves, lethal, nearlyDead)).toEqual([{ kind: 'damaged', seat: FOE, amount: 3 }])
  })

  it('shows no life change for a player who conceded', () => {
    const effects = effectsOf(
      (draft) => {
        must(draft.view.players[FOE]).lost = 'Conceded'
        for (const id of [FOE_BEAR, FOE_GOLEM, FOE_IMP, FOE_FOREST, FOE_IDOL]) draft.leave(id)
      },
      pub({ PlayerLost: { player: FOE, reason: 'Conceded' } }),
    )
    expect(effects).toEqual([])
  })
})

describe('purity', () => {
  it('reads frozen moments and events without writing to them', () => {
    const before = deepFreeze(momentOf(clone(miniView())))
    const events = deepFreeze(pub(hit(FOE_GOLEM, MY_BEAR, 2), died(MY_BEAR)))
    const after = deepFreeze(
      afterChange(
        before,
        (draft) => {
          draft.die(MY_BEAR)
          draft.life(ME, 18)
        },
        events,
      ),
    )
    expect(deriveEffects(before, after, events).map((effect) => effect.kind)).toEqual(['damaged', 'died'])
  })
})

// An artifact of the opponent stands for an Equipment; the viewer's creatures are its hosts.
const EQUIP = FOE_IDOL
const hangs =
  (host: number | null, player: number | null = null) =>
  (draft: Draft) => {
    draft.edit(EQUIP, (object) => {
      delete object.attached_to
      delete object.attached_player
      if (host !== null) object.attached_to = host
      if (player !== null) object.attached_player = player
    })
  }
const onBear = afterChange(base, hangs(MY_BEAR))

describe('the attachments that changed host between two moments', () => {
  const mine = (card: number) => ({ card, seat: ME })

  it.each([
    ['attached to a creature', base, afterChange(base, hangs(MY_BEAR)), null, mine(MY_BEAR)],
    [
      'moved to another creature',
      onBear,
      afterChange(onBear, hangs(MY_GIANT)),
      mine(MY_BEAR),
      mine(MY_GIANT),
    ],
    [
      'moved to a creature of the other seat',
      onBear,
      afterChange(onBear, hangs(FOE_BEAR)),
      mine(MY_BEAR),
      { card: FOE_BEAR, seat: FOE },
    ],
    ['unattached', onBear, afterChange(onBear, hangs(null)), mine(MY_BEAR), null],
    ['attached to a player', base, afterChange(base, hangs(null, FOE)), null, { player: FOE }],
    [
      'moved from a creature to a player',
      onBear,
      afterChange(onBear, hangs(null, ME)),
      mine(MY_BEAR),
      { player: ME },
    ],
    [
      'moved from a player to a creature',
      afterChange(base, hangs(null, FOE)),
      afterChange(base, hangs(MY_BEAR)),
      { player: FOE },
      mine(MY_BEAR),
    ],
  ])('finds an equipment %s', (_name, before, after, from, to) => {
    expect(deriveAttaches(before, after)).toEqual([{ object: EQUIP, seat: FOE, from, to }])
  })

  it.each([
    ['the table did not change', base, base],
    ['it stays on the same creature', onBear, afterChange(onBear, tap(MY_GIANT))],
    [
      'its host changes controller, taking it along',
      onBear,
      afterChange(onBear, (draft) =>
        draft.edit(MY_BEAR, (object) => {
          object.controller = FOE
        }),
      ),
    ],
    ['it dies', onBear, afterChange(onBear, (draft) => draft.die(EQUIP))],
    [
      'it enters already attached',
      base,
      afterChange(base, (draft) => {
        draft.enter(FOE_SPELL)
        draft.edit(FOE_SPELL, (object) => {
          object.attached_to = MY_BEAR
        })
      }),
    ],
  ])('finds nothing when %s', (_name, before, after) => {
    expect(deriveAttaches(before, after)).toEqual([])
  })

  it('finds each one by object id, with the controller of the attachment', () => {
    const both = afterChange(base, (draft) => {
      draft.edit(MY_WALL, (object) => {
        object.attached_to = MY_BEAR
      })
      draft.edit(EQUIP, (object) => {
        object.attached_to = MY_GIANT
      })
    })
    expect(deriveAttaches(base, both)).toEqual([
      { object: MY_WALL, seat: ME, from: null, to: { card: MY_BEAR, seat: ME } },
      { object: EQUIP, seat: FOE, from: null, to: { card: MY_GIANT, seat: ME } },
    ])
  })

  it('leaves its inputs untouched', () => {
    const after = afterChange(onBear, hangs(MY_GIANT))
    deepFreeze(onBear)
    deepFreeze(after)
    expect(() => deriveAttaches(onBear, after)).not.toThrow()
  })
})
