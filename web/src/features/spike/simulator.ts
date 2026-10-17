import type { GameObject, ObjectId, ObjectPatch } from '@/game/types'

export interface SimConfig {
  players: number
  permanentsPerPlayer: number
  handSize: number
  seed: number
}

/** Small deterministic PRNG so runs are reproducible. */
export function createRng(seed: number): () => number {
  let state = seed >>> 0
  return () => {
    state = (state + 0x6d2b79f5) >>> 0
    let t = state
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

const NAMES = ['Forest', 'Island', 'Grizzly Bears', 'Serra Angel', 'Llanowar Elves', 'Goblin Guide']

/** Builds an initial board: battlefield permanents and a hand per player. */
export function createBoard(config: SimConfig): GameObject[] {
  const rng = createRng(config.seed)
  const objects: GameObject[] = []
  let id = 1
  for (let player = 0; player < config.players; player += 1) {
    for (let n = 0; n < config.permanentsPerPlayer + config.handSize; n += 1) {
      const onBattlefield = n < config.permanentsPerPlayer
      objects.push({
        id: id++,
        name: NAMES[Math.floor(rng() * NAMES.length)] ?? 'Forest',
        controller: player,
        zone: onBattlefield ? 'battlefield' : 'hand',
        tapped: false,
        attacking: false,
        power: 1 + Math.floor(rng() * 4),
        toughness: 1 + Math.floor(rng() * 4),
        damage: 0,
        playable: !onBattlefield && rng() < 0.4,
      })
    }
  }
  return objects
}

/** Produces one server-like tick: a few state changes plus an occasional zone move. */
export function nextPatches(objects: readonly GameObject[], rng: () => number, tick: number): ObjectPatch[] {
  const patches: ObjectPatch[] = []
  const pick = (): GameObject | undefined => objects[Math.floor(rng() * objects.length)]
  for (let n = 0; n < 6; n += 1) {
    const object = pick()
    if (!object || object.zone !== 'battlefield') continue
    const kind = Math.floor(rng() * 3)
    if (kind === 0) patches.push({ id: object.id, changes: { tapped: !object.tapped } })
    else if (kind === 1) patches.push({ id: object.id, changes: { attacking: !object.attacking } })
    else patches.push({ id: object.id, changes: { damage: Math.floor(rng() * object.toughness) } })
  }
  if (tick % 20 === 0) {
    const inHand = objects.find((object) => object.zone === 'hand')
    if (inHand) patches.push({ id: inHand.id, changes: { zone: 'battlefield', playable: false } })
  }
  return patches
}

/** Ids of every battlefield object, used as a "legal target" set in the spike. */
export function battlefieldIds(objects: readonly GameObject[]): Set<ObjectId> {
  return new Set(objects.filter((object) => object.zone === 'battlefield').map((object) => object.id))
}
