// Goal: every keyword action, designation, player status, counter kind and predefined token the engine or the card corpus knows has words in the lexicon, so a new one fails until its term is written.
import { readdirSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { mentions, pieces, searchTerms, termFor, TERMS } from './index'

const ROOT = resolve(process.cwd(), '..')
const CARDS = resolve(ROOT, 'crates/mtg-cards')

const read = (path: string): string => readFileSync(resolve(ROOT, path), 'utf8')

/** The variant names of a Rust enum: the capitalised words at the first indent of its body. */
function variants(path: string, name: string): string[] {
  const body = read(path).split(`pub enum ${name} {\n`)[1]?.split('\n}\n')[0] ?? ''
  return [...body.matchAll(/^ {4}([A-Z][A-Za-z0-9]*)\b/gm)].map((m) => m[1] ?? '')
}

/** The field names of a generated TypeScript object type (one per line at the left margin). */
function fields(path: string): string[] {
  return [...read(path).matchAll(/^([a-z_]+)\??:/gm)].map((m) => m[1] ?? '')
}

const names = (list: string): string[] => list.split(/\s+/).filter((word) => word !== '')

// Each engine enum is split in two: the variants that stand for a keyword action or a designation, with the term that
// explains each, and the ordinary game text (draw, choose, move, conditions and the like). A new variant is in neither
// until someone decides which, and the test names it.

/** The `Effect` variants that are keyword actions, designations or the rules terms behind them, by lexicon term. */
const EFFECTS: Record<string, string> = {
  Discard: 'Discard',
  Mill: 'Mill',
  Scry: 'Scry',
  Reveal: 'Reveal',
  Search: 'Search',
  Shuffle: 'Shuffle',
  Destroy: 'Destroy',
  Regenerate: 'Regenerate',
  R116Assimilate: 'Assimilate',
  RemoveFromCombat: 'Removed from combat',
  Exile: 'Exile',
  Manifest: 'Manifest',
  BecomeForetold: 'Foretell',
  Cloak: 'Cloak',
  Encode: 'Cipher',
  Haunt: 'Haunt',
  Sacrifice: 'Sacrifice',
  SacrificeUpTo: 'Sacrifice',
  Counter: 'Counter',
  DealDamage: 'Damage',
  GainLife: 'Life',
  LoseLife: 'Life',
  SetLife: 'Life',
  PutCounters: 'Counters',
  RemoveCounters: 'Counters',
  RemoveAllCounters: 'Counters',
  MoveCounters: 'Counters',
  MultiplyCounters: 'Double',
  PutCountersOf: 'Counters',
  PutRandomCounter: 'Counters',
  R117AnotherOfKind: 'Counters',
  Tap: 'Tap',
  Untap: 'Untap',
  TapOrUntap: 'Tap',
  PhaseOut: 'Phasing',
  Rephase: 'Phasing',
  Attach: 'Attach',
  Unattach: 'Attach',
  MyriadCopies: 'Myriad',
  Transform: 'Transform',
  Flip: 'Flip card',
  TurnFaceUp: 'Turn face up',
  TurnFaceDown: 'Turn face down',
  Prepare: 'Prepared',
  Unprepare: 'Prepared',
  Suspect: 'Suspect',
  Unsuspect: 'Suspect',
  Solve: 'Solve',
  Harness: 'Harness',
  Plot: 'Plot',
  Surveil: 'Surveil',
  Proliferate: 'Proliferate',
  Goad: 'Goad',
  Fight: 'Fight',
  Venture: 'Venture into the dungeon',
  OpenAttraction: 'Open an Attraction',
  PutSticker: 'Sticker',
  ClaimPrize: 'Claim a prize',
  RollToVisit: 'Roll to Visit Your Attractions',
  UnlockDoor: 'Unlock',
  LockOrUnlockDoor: 'Unlock',
  ControlPlayer: 'Control another player',
  RestartGame: 'Restart the game',
  ReduceSpeed: 'Speed',
  BecomeMonarch: 'Monarch',
  RingTempts: 'The Ring tempts you',
  TakeInitiative: 'Initiative',
  CreateEmblem: 'Emblem',
  R6051PlayerGains: 'Emblem',
  BecomeDayNight: 'Day and night',
  Amass: 'Amass',
  Empower: 'Empower Jace',
  Incubate: 'Incubate',
  Conjure: 'Conjure',
  Seek: 'Seek',
  Monstrosity: 'Monstrosity',
  Renown: 'Renown',
  Exploit: 'Exploit',
  Enlist: 'Enlist',
  Pair: 'Soulbond',
  Crewed: 'Crew',
  BecomeLevel: 'Class',
  Adapt: 'Adapt',
  Bolster: 'Bolster',
  Fabricate: 'Fabricate',
  Explore: 'Explore',
  Connive: 'Connive',
  CastFromZone: 'Cast',
  CopyCard: 'Copy',
  CopyNamedCard: 'Copy',
  CopySpell: 'Copy',
  CopyAbility: 'Copy',
  BecomeCopy: 'Copy',
  Learn: 'Learn',
  TimeTravel: 'Time Travel',
  GainSuspend: 'Suspend',
  ExtraTurn: 'Extra turn',
  AddPhase: 'Additional phase',
  WinGame: 'Win the game',
  LoseGame: 'Lose the game',
  DrawGame: 'Draw the game',
  ExchangeLife: 'Exchange',
  ExchangeLifeWithToughness: 'Exchange',
  ExchangeControl: 'Exchange',
  RemoveDamage: 'Damage',
  RollDice: 'Roll a die',
  FlipCoin: 'Flip a coin',
  Clash: 'Clash',
  Vote: 'Vote',
  VillainousChoice: 'Villainous choice',
  Planechase: 'Planeswalk',
  CreateToken: 'Token',
  Continuous: 'Continuous effect',
  Shield: 'Prevention effect',
  DelayedTrigger: 'Delayed triggered ability',
  Reflexive: 'Reflexive triggered ability',
  Modal: 'Modal',
}

const ORDINARY_EFFECTS =
  names(`Draw LookAtTop LookAtHand LookAtFaceDown Choose ChooseWithin P160CastWithin ChooseValue ChooseAtRandom
  Move LoseUnspentMana PutBeneathTop BecomeBlocked FaceDownPile ExileHidden Remember Forget RememberValue Balance AddMana
  SpellEntersWith JoinAttack Revive Parade ExileUntil RevealUntil ExileTop Performed EndTurn AddStep RepeatWhile
  SeparatePiles RepeatEach ChooseWords ChooseNumbers BidLife Either OneOf ChangeTargets Rule Reattack If Together May Pay
  R6400Block AuraSwap Meld R6401PassControl R6501RedistributeLife R6602CreateAttached
  R6701RemoveAmong R6701TurnFaceDownHorror R6701ControlCombat R6702Distribute R6702CountersPerVote
  PayAnyTimes StillResolves Bind Let R110RemovePutThisWay R110MoveEachKindMissing R110MoveOneOrMore R110ChooseSecretly R111KeepChosen R112NoteManaSpent
  R110AddCombatAfterFirst R111OrderGraveyard R112DoubleX R112NoteChosen R123XBecomes R123ExileContest
  R112GuessNumber R112Piles R112RollStore R112RerollStored R113PhaseIn R114StoreValue R114CopySpellOnto
  R115ExchangeBlocks R115ExchangeBlockers R115JoinBlock R115SkipStep R117EndCombat R119ActivateLandMana R119DrainMana R119BindSpliced R120ChooseEachColor`)

/** The `CostItem` variants that are keyword actions or the words of one, by lexicon term. */
const COSTS: Record<string, string> = {
  SacrificeSelf: 'Sacrifice',
  Sacrifice: 'Sacrifice',
  R6702SacrificeUpTo: 'Sacrifice',
  Discard: 'Discard',
  DiscardSelf: 'Discard',
  DiscardRandom: 'Discard',
  Mill: 'Mill',
  Tap: 'Tap',
  TapPermanents: 'Tap',
  TapPower: 'Tap',
  Untap: 'Untap',
  UntapPermanents: 'Untap',
  ExileSelf: 'Exile',
  ExileFromGraveyard: 'Exile',
  ExilePermanents: 'Exile',
  ExileSpell: 'Exile',
  ExileFromHand: 'Exile',
  ExileTop: 'Exile',
  ExileCardTypes: 'Exile',
  RevealFromHand: 'Reveal',
  PayEnergy: 'Energy counter',
  SpendCounters: 'Counters',
  RemoveCounters: 'Counters',
  RemoveCountersAmong: 'Counters',
  PutCounters: 'Counters',
  GetPoison: 'Poison counter',
  Unattach: 'Attach',
  Exert: 'Exert',
  CollectEvidence: 'Collect evidence',
  R6702CollectEvidenceX: 'Collect evidence',
  Blight: 'Blight',
  Behold: 'Behold',
  Waterbend: 'Waterbend',
  CreaturesHelpPay: 'Tap',
  Forage: 'Forage',
  Craft: 'Craft',
  R114RollDie: 'Roll a die',
}

const ORDINARY_COSTS =
  names(`Mana PayLife Draw ManaCostOf OwnManaCost OwnManaValue OwnManaCostLess ReturnToHand AlikeFromHand
  DamageToYou OthersGainLife OpponentGainsLife GenericOf ExiledToGraveyard ToLibrary XAtLeast XColor R113XHybrid R112XOneEachColor R112ExileFromOneGraveyard R113BottomFromOneGraveyard ChosenColorOnly XAtMost XIs R121XIsPerOpponent OneOf Changed R6702ExileTotalX ExileTotalValue R110ControllerDraws R110RevealChosen R110Choose R110ChooseNumber R6012ChooseOrExiled`)

/** The `TriggerPattern` variants that name a keyword action, designation or rules term, by lexicon term. */
const TRIGGERS: Record<string, string> = {
  SpellCast: 'Cast',
  SpellCopied: 'Copy',
  Sacrificed: 'Sacrifice',
  Milled: 'Mill',
  Discarded: 'Discard',
  Tapped: 'Tap',
  TappedNotAttacking: 'Tap',
  Untapped: 'Untap',
  TappedForMana: 'Mana ability',
  PhasedOut: 'Phasing',
  PhasedIn: 'Phasing',
  Chapter: 'Saga',
  FinalChapter: 'Saga',
  CountersPut: 'Counters',
  PlayerCountersPut: 'Counters',
  CountersRemoved: 'Counters',
  NthCounterPut: 'Counters',
  Attached: 'Attach',
  Unattached: 'Attach',
  Cycled: 'Cycling',
  Ventured: 'Venture into the dungeon',
  CompletedDungeon: 'Dungeon',
  Visit: 'Roll to Visit Your Attractions',
  RolledToVisit: 'Roll to Visit Your Attractions',
  VisitedAttraction: 'Roll to Visit Your Attractions',
  Prize: 'Claim a prize',
  ClaimedPrize: 'Claim a prize',
  OpenedAttraction: 'Open an Attraction',
  DoorUnlocked: 'Unlock',
  TakesInitiative: 'Initiative',
  BecomesMonarch: 'Monarch',
  DayNightChanges: 'Day and night',
  RingTempts: 'The Ring tempts you',
  Proliferated: 'Proliferate',
  DiceRolled: 'Roll a die',
  CoinFlipWon: 'Flip a coin',
  W15cClashWon: 'Clash',
  Amassed: 'Amass',
  Explored: 'Explore',
  Connived: 'Connive',
  BecomesMonstrous: 'Monstrosity',
  BecomesRenowned: 'Renown',
  Exploits: 'Exploit',
  R6401Mentors: 'Mentor',
  BecomesCrewed: 'Crew',
  BecomesSaddled: 'Saddle',
  Exerted: 'Exert',
  TurnedFaceUp: 'Turn face up',
  Mutates: 'Mutate',
  BecomesLevel: 'Class',
  Adapted: 'Adapt',
  Bolstered: 'Bolster',
  Expend: 'Expend',
  W11cCrime: 'Commit a crime',
  R116Dilemma: 'Face a dilemma',
  Surveilled: 'Surveil',
  DealsDamage: 'Damage',
  DealtExcess: 'Damage',
  DamagePrevented: 'Prevention effect',
  LifeGained: 'Life',
  LifeLost: 'Life',
}

const ORDINARY_TRIGGERS =
  names(`ZoneChange ZoneChangeExcept ResolvedToGraveyard Attacks AttacksAlone AttacksPlayer
  AttacksPlayerOrPlaneswalker PlayerAttacked PlayerAttacks BecomesTarget PlayerBecomesTarget PutOnStack FirstDrawRevealed
  Drew Blocks BecomesBlocked Unblocked GroupAttacks GroupAttacksAt StepBegins State Did AnyOf Batch Nth Caused R6701WhileActivating R109WhileCasting DealsToughnessDamage PhaseBegins
  R110ThatCombatBegins R111AbilityTriggered R120RandomAction R120AssistedCast R120DealsExactly`)

/** The fields of the player view that are a designation, a status or a counter the rules define, by lexicon term. */
const PLAYER_FIELDS: Record<string, string> = {
  life: 'Life',
  lost: 'Lose the game',
  mana: 'Mana pool',
  counters: 'Counters',
  citys_blessing: "City's blessing",
  enduring_story: 'Storied',
  speed: 'Speed',
  ring_level: 'The Ring tempts you',
  ring_bearer: 'Ring-bearer',
  enchanted_by: 'Aura',
  mulligans: 'Mulligan',
  mulligan: 'Mulligan',
  hand: 'Hand',
  library: 'Library',
  graveyard: 'Graveyard',
  command: 'Command zone',
  revealed_top: 'Reveal',
  known_top: 'Reveal',
  revealed_hand: 'Reveal',
  attraction_deck: 'Attraction',
  sticker_sheets: 'Sticker',
  commanders: 'Commander',
  commander_damage: 'Commander damage',
  commander_lethal: 'Commander damage',
  companion: 'Companion',
}

const PLAIN_PLAYER_FIELDS = names(
  'seat effects sources spirit eyes boss satchel bank turn_time_ms turn_time_running connected',
)

/** The fields of the turn view that are a designation or a rules term, by lexicon term. */
const TURN_FIELDS: Record<string, string> = {
  step: 'Turn',
  extra: 'Extra turn',
  extra_turns: 'Extra turn',
  day_night: 'Day and night',
  monarch: 'Monarch',
  initiative: 'Initiative',
  controlled_by: 'Control another player',
}

const PLAIN_TURN_FIELDS = names('number round active active_left lands_played')

const SPLITS: Array<[string, string, string, Record<string, string>, string[]]> = [
  ['Effect', 'crates/mtg-cards/src/effect.rs', 'Effect', EFFECTS, ORDINARY_EFFECTS],
  ['CostItem', 'crates/mtg-cards/src/cost.rs', 'CostItem', COSTS, ORDINARY_COSTS],
  ['TriggerPattern', 'crates/mtg-cards/src/trigger.rs', 'TriggerPattern', TRIGGERS, ORDINARY_TRIGGERS],
]

/** What is wrong with a split of names into explained and ordinary ones: new, stale or listed twice. */
function problems(found: string[], explained: Record<string, string>, ordinary: string[]): string[] {
  const known = new Set([...Object.keys(explained), ...ordinary])
  const present = new Set(found)
  return [
    ...found.filter((name) => !known.has(name)).map((name) => `${name} is in neither list`),
    ...[...known].filter((name) => !present.has(name)).map((name) => `${name} is not in the engine`),
    ...ordinary.filter((name) => name in explained).map((name) => `${name} is in both lists`),
  ]
}

describe.each(SPLITS)(
  'the lexicon covers the %s variants of the engine',
  (label, path, name, explained, ordinary) => {
    it('reads the enum and classifies every variant', () => {
      const found = variants(path, name)
      expect(found.length, `${label} variants read`).toBeGreaterThan(40)
      expect(problems(found, explained, ordinary)).toEqual([])
    })

    it.each([...new Set(Object.values(explained))])('has the term %s', (title) => {
      expect(termFor(title)?.text.length ?? 0, title).toBeGreaterThan(10)
    })
  },
)

describe('the lexicon covers the designations of the views', () => {
  it.each([
    ['PlayerView', PLAYER_FIELDS, PLAIN_PLAYER_FIELDS],
    ['TurnView', TURN_FIELDS, PLAIN_TURN_FIELDS],
  ] as const)('classifies every field of %s', (view, explained, plain) => {
    const found = fields(`web/src/protocol/generated/${view}.ts`)
    expect(found.length, `${view} fields read`).toBeGreaterThan(8)
    expect(problems(found, explained, [...plain])).toEqual([])
  })

  it.each([...new Set([...Object.values(PLAYER_FIELDS), ...Object.values(TURN_FIELDS)])])(
    'has the term %s',
    (title) => {
      expect(termFor(title)?.text.length ?? 0, title).toBeGreaterThan(10)
    },
  )
})

/** The counter kinds the engine's own rules name, outside its tests. */
function engineCounters(): string[] {
  const seen = new Set<string>()
  for (const crate of ['mtg-engine', 'mtg-cards']) {
    const dir = resolve(ROOT, `crates/${crate}/src`)
    for (const file of readdirSync(dir, { recursive: true, encoding: 'utf8' })) {
      if (!file.endsWith('.rs') || /(^|\/)tests?(\.rs|\/)|_tests\.rs$/.test(file)) continue
      const source = readFileSync(resolve(dir, file), 'utf8').split('#[cfg(test)]')[0] ?? ''
      for (const m of source.matchAll(/Other\(\s*"([^"]+)"/g)) seen.add(m[1] ?? '')
    }
  }
  return [...seen]
}

/** The counter kinds that at least `least` cards of the corpus put on something. */
function corpusCounters(least: number): string[] {
  const cards = resolve(CARDS, 'cards')
  const cardCount = new Map<string, number>()
  for (const file of readdirSync(cards, { recursive: true, encoding: 'utf8' })) {
    if (!file.endsWith('.json')) continue
    const found = new Set<string>()
    for (const m of readFileSync(resolve(cards, file), 'utf8').matchAll(/"Other":\s*"([^"]+)"/g)) {
      found.add(m[1] ?? '')
    }
    for (const kind of found) cardCount.set(kind, (cardCount.get(kind) ?? 0) + 1)
  }
  return [...cardCount].filter(([, count]) => count >= least).map(([kind]) => kind)
}

/** The keywords a counter named for one gives, from the arms of `CounterKind::keyword`. */
function keywordCounters(): string[] {
  const body =
    read('crates/mtg-cards/src/value.rs').split('pub fn keyword(&self)')[1]?.split('\n    }\n')[0] ?? ''
  return [...body.matchAll(/^\s*"([a-z ]+)" =>/gm)].map((m) => m[1] ?? '')
}

// A counter named for its numbers ("+1/+0") is a power and toughness counter, which the +1/+1 term explains.
const NUMBERS = /^[+-][0-9a-z]+\/[+-][0-9a-z]+$/

// The counters a player holds (CR 122.1), named whether or not a card of the corpus uses them often.
const PLAYER_COUNTERS = ['poison', 'energy', 'experience', 'rad', 'ticket']

describe('the lexicon covers the counters', () => {
  const engine = engineCounters()
  const corpus = corpusCounters(10)
  const keywords = keywordCounters()
  const plain = [...new Set([...engine, ...corpus, ...PLAYER_COUNTERS])]
    .filter((kind) => !NUMBERS.test(kind) && !keywords.includes(kind))
    .sort()

  it('reads the engine, the corpus and the keyword counters', () => {
    expect(engine).toEqual(expect.arrayContaining(['stun', 'finality', 'energy', 'rad']))
    expect(corpus).toEqual(expect.arrayContaining(['energy', 'ticket', 'quest']))
    expect(keywords).toEqual(expect.arrayContaining(['flying', 'lifelink']))
  })

  it.each(plain.map((kind) => [kind]))('has the %s counter', (kind) => {
    expect(termFor(`${kind} counter`)?.text.length ?? 0, `${kind} counter`).toBeGreaterThan(10)
  })

  it.each(keywords.map((kind) => [kind]))('has the keyword %s of a keyword counter', (kind) => {
    expect(termFor(kind), kind).toBeDefined()
  })

  it('explains the keyword counters in one term', () => {
    expect(termFor('Keyword counter')?.kind).toBe('rule')
  })
})

describe('the lexicon covers the predefined tokens', () => {
  const dir = resolve(CARDS, 'cards/tokens')
  const tokens = readdirSync(dir)
    .filter((file) => file.endsWith('.json'))
    .map(
      (file) =>
        JSON.parse(readFileSync(resolve(dir, file), 'utf8')) as { token: { name: string; types: string[] } },
    )
    .filter(({ token }) => !token.types.includes('Creature'))
    .map(({ token }) => token.name)

  it('reads the token files', () => {
    expect(tokens).toEqual(expect.arrayContaining(['Treasure', 'Clue', 'Powerstone']))
  })

  it.each(tokens.map((name) => [name]))('has the %s token', (name) => {
    expect(termFor(name)?.text.length ?? 0, name).toBeGreaterThan(10)
  })
})

describe('the Ring', () => {
  it('has the action, the Ring-bearer and the four abilities in order', () => {
    expect(termFor('the Ring tempts you')?.kind).toBe('action')
    expect(termFor('Ring-bearer')?.kind).toBe('rule')
    const abilities = TERMS.filter((term) => term.title.startsWith('Ring ability '))
    expect(abilities.map((term) => term.title.slice(0, 'Ring ability N'.length))).toEqual([
      'Ring ability 1',
      'Ring ability 2',
      'Ring ability 3',
      'Ring ability 4',
    ])
    expect(abilities.map((term) => term.text)).toEqual([
      expect.stringContaining('legendary and can not be blocked by creatures with greater power'),
      expect.stringContaining('attacks, draw a card, then discard a card'),
      expect.stringContaining('sacrifices it at end of combat'),
      expect.stringContaining('each opponent loses 3 life'),
    ])
  })

  it.each([
    [
      'The Ring tempts Bo (level 2) and chooses Forest as their Ring-bearer',
      ['The Ring tempts', 'Ring-bearer'],
    ],
    ['The Ring tempts you.', ['The Ring tempts you']],
    ['Whenever your Ring-bearer attacks, draw a card, then discard a card.', ['Ring-bearer']],
    ['Bo loses their Ring-bearer', ['Ring-bearer']],
  ])('marks the words of "%s"', (text, marked) => {
    expect(
      pieces(text)
        .filter((piece) => piece.term !== undefined)
        .map((piece) => piece.text),
    ).toEqual(marked)
    expect(mentions(text).length).toBeGreaterThan(0)
  })

  it.each([
    ['ring-bearer', 'Ring-bearer'],
    ['the ring tempts', 'The Ring tempts you'],
    ['city', "City's blessing"],
    ['energy counter', 'Energy counter'],
    ['rad', 'Rad counter'],
  ])('finds "%s" with %s first', (query, title) => {
    expect(searchTerms(query)[0]?.title).toBe(title)
  })

  it('finds the abilities by what they do', () => {
    expect(searchTerms('ring-bearer').map((term) => term.title)).toEqual(
      expect.arrayContaining(['Ring ability 2: loot', 'Ring ability 4: drain']),
    )
  })

  it('leaves a card named Sol Ring alone', () => {
    expect(mentions('Ana casts Sol Ring.')).toEqual([])
  })
})
