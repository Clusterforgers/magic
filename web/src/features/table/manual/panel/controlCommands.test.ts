// Goal: the control rows of a picked card (remove summoning sickness, sacrifice, give control, onto the battlefield for another seat) say their change in words and send the exact action, the seat row asks for a seat unless one is left and is inert without another seat or for a seat out of the game.
import { describe, expect, it } from 'vitest'
import type { ManualAction, ObjectView } from '@/protocol/generated'
import { card, permanent } from '@/test-utils/cards'
import { OUT_OF_GAME } from '../offers'
import type { MenuInput } from '../offers'
import { controlCommands, previewOfCard, startCardDraft } from './cardCommands'
import type { CardCommandId, CardContext } from './cardCommands'

const names = ['Ana', 'Bo', 'Cy']

function input(object: ObjectView, over: Partial<MenuInput> = {}): MenuInput {
  return { seat: 0, object, players: [0, 1], hasCommandZone: false, ownerName: 'Ana', ...over }
}

function context(menu: MenuInput): CardContext {
  return { input: menu, objects: { [menu.object.id]: menu.object }, nameOf: (seat) => names[seat] ?? 'Seat' }
}

function preview(object: ObjectView, id: CardCommandId, over: Partial<MenuInput> = {}, giveTo?: string) {
  const menu = input(object, over)
  const command = controlCommands(menu).find((each) => each.id === id)
  if (command === undefined) throw new Error(`no ${id}`)
  const draft = startCardDraft(command, menu)
  return previewOfCard(command, giveTo === undefined ? draft : { ...draft, giveTo }, context(menu))
}

const SICK = permanent({ summoning_sick: true }, { id: 7, name: 'Fresh Elf' })
const DEAD = card({ id: 8, name: 'Old Bear', zone: { Graveyard: 0 }, permanent: null })
const EXILED = card({ id: 9, name: 'It That Betrays', zone: 'Exile', permanent: null })

describe('what a control row sends', () => {
  it.each<[string, ObjectView, CardCommandId, string, string, ManualAction]>([
    [
      'a sickness removed',
      SICK,
      'unsick',
      'Fresh Elf: no longer summoning sick this turn',
      'Remove summoning sickness',
      { RemoveSickness: { object: 7 } },
    ],
    [
      'a sacrifice',
      SICK,
      'sacrifice',
      'Fresh Elf: the battlefield → graveyard, sacrificed',
      'Sacrifice Fresh Elf',
      { Sacrifice: { object: 7 } },
    ],
    [
      'a permanent turned over',
      SICK,
      'transform',
      'Fresh Elf: turned over to its other face',
      'Turn over Fresh Elf',
      { Transform: { object: 7 } },
    ],
    [
      'a card put back face up',
      DEAD,
      'back',
      'Old Bear: onto the battlefield, back face up',
      'Put Old Bear back face up',
      { MoveBack: { object: 8 } },
    ],
    [
      'control given',
      SICK,
      'give',
      'Fresh Elf: control → Bo',
      'Give Fresh Elf to Bo',
      { GiveControl: { object: 7, to: 1 } },
    ],
    [
      'a card from the graveyard',
      DEAD,
      'give',
      'Old Bear: graveyard → the battlefield under Bo’s control',
      'Give Old Bear to Bo',
      { GiveControl: { object: 8, to: 1 } },
    ],
    [
      'a card from exile',
      EXILED,
      'give',
      'It That Betrays: exile → the battlefield under Bo’s control',
      'Give It That Betrays to Bo',
      { GiveControl: { object: 9, to: 1 } },
    ],
  ])('%s', (_name, object, id, words, apply, action) => {
    expect(preview(object, id)).toEqual({ preview: words, apply, action })
  })
})

describe('the seat that gains control', () => {
  it('is asked for among several seats and offers every other seat', () => {
    const over = { players: [0, 1, 2] }
    expect(preview(SICK, 'give', over)).toEqual({ problem: 'Choose the seat that gains control.' })
    expect(controlCommands(input(SICK, over)).find((c) => c.id === 'give')?.seats).toEqual([1, 2])
    expect(preview(SICK, 'give', over, '2')).toMatchObject({ action: { GiveControl: { object: 7, to: 2 } } })
  })

  it.each<[string, Partial<MenuInput>, string]>([
    ['no other seat in the game', { players: [0] }, 'No other seat is in the game.'],
    ['a seat out of the game', { off: OUT_OF_GAME }, OUT_OF_GAME],
  ])('is inert with %s', (_name, over, reason) => {
    const rows = controlCommands(input(SICK, over))
    expect(rows.find((c) => c.id === 'give')?.unavailable).toBe(reason)
    expect(preview(SICK, 'give', over)).toEqual({ problem: reason })
  })
})
