/** The combat prompts in plain words: who is attacked, how many attack where, who blocks what. */
import type { AttackerOption, AttackTarget, ObjectId } from '@/protocol/generated'

/** How a defender is called: a short name for cards and sums, a longer one where it must be told apart. */
export interface TargetNames {
  /** "Ana", or the planeswalker's name. */
  short(target: AttackTarget): string
  /** "Ana", or "Jace, Bo's planeswalker". */
  long(target: AttackTarget): string
}

/** A key that is equal for equal targets, for maps and `<option value>`. */
export function targetKey(target: AttackTarget): string {
  return 'Player' in target ? `player:${target.Player}` : `walker:${target.Planeswalker}`
}

/** Whether two targets are the same player or the same planeswalker. */
export function sameTarget(a: AttackTarget, b: AttackTarget): boolean {
  return targetKey(a) === targetKey(b)
}

/** Every defender any attacker may attack, in the order they first appear. */
export function distinctTargets(attackers: readonly AttackerOption[]): AttackTarget[] {
  const seen = new Map<string, AttackTarget>()
  for (const attacker of attackers) {
    for (const target of attacker.defenders) if (!seen.has(targetKey(target))) seen.set(targetKey(target), target)
  }
  return [...seen.values()]
}

/** "1 attacker", "3 attackers". */
export function countOf(count: number, one: string, many = `${one}s`): string {
  return `${count} ${count === 1 ? one : many}`
}

/**
 * "2 attackers at Ana" when they all go to one defender, "3 attackers: 2 at Ana, 1 at Bo" when they do not (groups in
 * the order they were first chosen), "No attackers" for none.
 */
export function attackSummary(targets: readonly AttackTarget[], names: TargetNames): string {
  if (targets.length === 0) return 'No attackers'
  const groups = new Map<string, { target: AttackTarget; count: number }>()
  for (const target of targets) {
    const group = groups.get(targetKey(target))
    if (group === undefined) groups.set(targetKey(target), { target, count: 1 })
    else group.count += 1
  }
  const head = countOf(targets.length, 'attacker')
  const [only] = [...groups.values()]
  if (groups.size === 1 && only !== undefined) return `${head} at ${names.short(only.target)}`
  return `${head}: ${[...groups.values()].map((g) => `${g.count} at ${names.short(g.target)}`).join(', ')}`
}

/** One name per creature that stays unique when two share a name: "Elf", "Elf 1", "Elf 2". */
export function uniqueNames(items: readonly { id: ObjectId; name: string }[]): Map<ObjectId, string> {
  const total = new Map<string, number>()
  for (const { name } of items) total.set(name, (total.get(name) ?? 0) + 1)
  const seen = new Map<string, number>()
  const result = new Map<ObjectId, string>()
  for (const { id, name } of items) {
    if ((total.get(name) ?? 0) < 2) {
      result.set(id, name)
      continue
    }
    const nth = (seen.get(name) ?? 0) + 1
    seen.set(name, nth)
    result.set(id, `${name} ${nth}`)
  }
  return result
}

/** "3 blockers on 2 attackers", or "No blocks" for none. */
export function blockSummary(blockers: number, attackers: number): string {
  if (blockers === 0) return 'No blocks'
  return `${countOf(blockers, 'blocker')} on ${countOf(attackers, 'attacker')}`
}

/** The blockers of each attacker, in the order the pairs were made; `pairs` is blocker to attacker. */
export function blockersByAttacker(pairs: ReadonlyMap<ObjectId, ObjectId>): Map<ObjectId, ObjectId[]> {
  const grouped = new Map<ObjectId, ObjectId[]>()
  for (const [blocker, attacker] of pairs) {
    const list = grouped.get(attacker)
    if (list === undefined) grouped.set(attacker, [blocker])
    else list.push(blocker)
  }
  return grouped
}
