/** The marks of a mana source chip: what its mana may pay for and what a use takes besides a tap. */
import type { ManaOption, Take } from '@/protocol/generated'
import { usesWords } from './spendRules'

/** One mark: the short word the chip shows and the sentence a screen reader and the tooltip get. */
export interface SourceMark {
  key: 'only' | 'life' | 'energy' | 'tap' | 'sacrifice' | 'exile'
  short: string
  words: string
}

type TakeKind = 'tap' | 'sacrifice' | 'exile'

const takeKind = (take: Take): TakeKind =>
  take.sacrifice === true ? 'sacrifice' : take.exile === true ? 'exile' : 'tap'

const TAKE_WORDS: Record<TakeKind, (count: number) => string> = {
  tap: (count) => `taps ${count} more ${count === 1 ? 'permanent' : 'permanents'}`,
  sacrifice: (count) => `sacrifices ${count} more ${count === 1 ? 'permanent' : 'permanents'}`,
  exile: (count) => `exiles ${count} ${count === 1 ? 'card' : 'cards'} from your hand`,
}

/** The most `amount` asks of any option, and whether every option asks it. */
function most(options: readonly ManaOption[], amount: (option: ManaOption) => number) {
  const amounts = options.map(amount)
  return { count: Math.max(0, ...amounts), all: amounts.every((one) => one > 0) }
}

/**
 * The marks of the options of one source, in a fixed order: `only` for restricted mana, then life, energy and the
 * permanents or cards a use takes; a mark that only some ways carry says so in its words.
 */
export function sourceMarks(options: readonly ManaOption[]): SourceMark[] {
  const some = (all: boolean): string => (all ? '' : ' for some mana')
  const marks: SourceMark[] = []
  const restricted = options.find((option) => usesWords(option.only ?? []) !== 'anything')
  if (restricted !== undefined) {
    const all = options.every((option) => usesWords(option.only ?? []) !== 'anything')
    marks.push({
      key: 'only',
      short: 'only',
      words: `${all ? 'mana' : 'some mana'} only for ${usesWords(restricted.only ?? [])}`,
    })
  }
  const life = most(options, (option) => option.life ?? 0)
  if (life.count > 0) {
    marks.push({
      key: 'life',
      short: `${life.count} life`,
      words: `pays ${life.count} life${some(life.all)}`,
    })
  }
  const energy = most(options, (option) => option.energy ?? 0)
  if (energy.count > 0) {
    marks.push({
      key: 'energy',
      short: `${energy.count} energy`,
      words: `pays ${energy.count} energy${some(energy.all)}`,
    })
  }
  for (const kind of ['tap', 'sacrifice', 'exile'] as const) {
    const taken = most(options, (option) =>
      (option.takes ?? [])
        .filter((take) => takeKind(take) === kind)
        .reduce((sum, take) => sum + take.count, 0),
    )
    if (taken.count > 0) {
      marks.push({
        key: kind,
        short: `${kind} ${taken.count}`,
        words: `${TAKE_WORDS[kind](taken.count)}${some(taken.all)}`,
      })
    }
  }
  return marks
}
