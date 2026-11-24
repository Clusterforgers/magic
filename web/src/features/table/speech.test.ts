// Goal: the viewer's own seat reads in second person wherever the log, the turn ticker and the preview name it: "You play Plains", "Your turn", "Controlled by you", for any seat number, while every other seat and a spectator's log read as before; a seat whose player is named "You" never reads "You plays" or "You's turn".
import { describe, expect, it } from 'vitest'
import type { LogEntry } from '@/play/store'
import type { EventView, ObjectView, PlayerId, Zone } from '@/protocol/generated'
import { pass, pub } from '@/test-utils/turnEvents'
import { foldLog } from './logFold'
import { controlText, zoneText } from './previewText'
import { VIEWER, secondPerson, speak, voiced } from './speech'

const NAMES = ['You', 'Bo', 'Cy', 'Dee']
const seat = (at: PlayerId): string => NAMES[at] ?? `Seat ${at + 1}`
const names = { seat, card: (id: number) => (id === 7 ? 'Plains' : `Card ${id}`) }
const logOf = (events: readonly EventView[]): LogEntry[] => events.map((event, seq) => ({ seq, event }))
const fold = (events: readonly EventView[], viewer: PlayerId | null): string[] =>
  foldLog(logOf(events), names, { seats: [0, 1, 2, 3], viewer }).map((line) => line.text)

describe('a verb in second person', () => {
  it.each([
    ['plays', 'play'],
    ['casts', 'cast'],
    ['passes', 'pass'],
    ['scries', 'scry'],
    ['dies', 'die'],
    ['copies', 'copy'],
    ['searches', 'search'],
    ['amasses', 'amass'],
    ['forages', 'forage'],
    ['is', 'are'],
    ['has', 'have'],
    ["doesn't", "don't"],
    ['goes', 'go'],
  ])('%s becomes %s', (third, second) => {
    expect(secondPerson(third)).toBe(second)
  })

  it.each(['can', "can't", 'may', 'and', 'must'])('%s is no verb to change', (word) => {
    expect(secondPerson(word)).toBeNull()
  })
})

describe('a sentence spoken to the viewer', () => {
  it.each([
    [`${VIEWER} plays Plains`, 'You play Plains'],
    [`${VIEWER}'s turn`, 'Your turn'],
    [`Turn 3 begins: ${VIEWER}'s turn`, 'Turn 3 begins: your turn'],
    [
      `${VIEWER} puts a card from their library into their hand`,
      'You put a card from your library into your hand',
    ],
    [`${VIEWER} is dealt 7 cards`, 'You are dealt 7 cards'],
    [`${VIEWER} no longer has the initiative`, 'You no longer have the initiative'],
    [`${VIEWER} can't attack`, "You can't attack"],
    [`The Ring tempts ${VIEWER} (level 1)`, 'The Ring tempts you (level 1)'],
    [`Bo attacks ${VIEWER}`, 'Bo attacks you'],
    [`Bo looks at ${VIEWER}'s hand: Plains`, 'Bo looks at your hand: Plains'],
    [`Bo, ${VIEWER} and Cy pass`, 'Bo, you and Cy pass'],
    [`Bo puts their card into ${VIEWER}'s graveyard`, 'Bo puts their card into your graveyard'],
    ['Bo plays Plains', 'Bo plays Plains'],
  ])('%s', (marked, spoken) => {
    expect(speak(marked)).toBe(spoken)
  })

  it('names the viewer only, and nobody while there is no viewer', () => {
    expect(voiced(seat, 1)(1)).toBe(VIEWER)
    expect(voiced(seat, 1)(2)).toBe('Cy')
    expect(voiced(seat, null)(0)).toBe('You')
  })
})

describe.each([0, 2])('the log read by seat %i', (viewer) => {
  const played: EventView = pub({ LandPlayed: { player: viewer, object: 7 } })
  const turn: EventView = pub({ TurnBegan: { number: 3, active: viewer, extra: false } } as never)
  const other: EventView = pub({ LandPlayed: { player: 1, object: 7 } })

  it('tells what the viewer does in second person and every other seat in third', () => {
    expect(fold([turn, played, other], viewer)).toEqual([
      'Turn 3 begins: your turn',
      'You play Plains',
      'Bo plays Plains',
    ])
  })

  it('folds the viewer alone passing into "You pass"', () => {
    const lines = fold([pass(viewer), pub({ SpellCast: { player: 1, object: 8 } })], viewer)
    expect(lines[0]).toBe('You pass')
  })

  it('tells a spectator every seat by name', () => {
    expect(fold([played], null)).toEqual([`${seat(viewer)} plays Plains`])
  })
})

describe('the preview of a card of the viewer', () => {
  const object = { controller: 0, owner: 1 } as ObjectView
  const named = voiced(seat, 0)

  it.each<[Zone, string]>([
    [{ Hand: 0 }, 'In your hand'],
    [{ Graveyard: 1 }, "In Bo's graveyard"],
    ['Battlefield', 'On the battlefield'],
  ])('says where it is: %j', (zone, words) => {
    expect(speak(zoneText(zone, named))).toBe(words)
  })

  it('says who controls it', () => {
    expect(speak(controlText(object, named))).toBe('Controlled by you, owned by Bo')
  })
})
