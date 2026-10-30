import { Fragment } from 'react'
import { cn } from '@/lib/utils'
import type { ManaCost, ManaType } from '@/protocol/generated'
import { costPips } from './cardModel'
import { madeCost } from './manaMade'
import type { Gives } from './manaMade'
import { AnyPip, Pip } from './ManaPips'
import { andList, costWords, manaName } from './prompt/payText'

/** The runs of equal mana in order: `[C, C, G]` is C twice then G once. */
function runsOf(made: readonly ManaType[]): Array<{ mana: ManaType; count: number }> {
  const runs: Array<{ mana: ManaType; count: number }> = []
  for (const mana of made) {
    const last = runs.at(-1)
    if (last?.mana === mana) last.count += 1
    else runs.push({ mana, count: 1 })
  }
  return runs
}

/** What one way makes in words: "white", "2 colorless", "white and blue". */
function wayWords(made: readonly ManaType[]): string {
  return andList(
    runsOf(made).map(({ mana, count }) => (count > 1 ? `${count} ${manaName(mana)}` : manaName(mana))),
  )
}

/** What a source gives in words: "2 colorless", "white or blue", "one mana of any color", "pay 1 generic for white and black". */
export function madeWords(gives: Gives): string {
  const body = gives.any ? 'one mana of any color' : gives.ways.map((way) => wayWords(way.made)).join(' or ')
  const pay = gives.cost.length > 0 ? `pay ${costWords(gives.cost)} for ` : ''
  return `${pay}${body}${gives.sacrifice ? ', sacrificing it' : ''}`
}

/** The mana as symbols: each mana a symbol up to three of a kind, then one symbol with its count. */
function MadeRun({ made, sizeClass }: { made: readonly ManaType[]; sizeClass: string }) {
  return (
    <span className="inline-flex items-center gap-0.5">
      {runsOf(made).map(({ mana, count }, at) => {
        const [pip] = costPips(madeCost([mana]))
        if (pip === undefined) return null
        return count > 3 ? (
          <span key={`${mana}-${at}`} className="inline-flex items-center gap-px">
            <Pip pip={pip} sizeClass={sizeClass} />
            <span aria-hidden="true" data-count={count} className="text-[0.85em] font-semibold tabular-nums">
              ×{count}
            </span>
          </span>
        ) : (
          Array.from({ length: count }, (_, n) => (
            <Pip key={`${mana}-${at}-${n}`} pip={pip} sizeClass={sizeClass} />
          ))
        )
      })}
    </span>
  )
}

/** The symbols a use costs. */
function CostRun({ cost, sizeClass }: { cost: ManaCost; sizeClass: string }) {
  return (
    <span className="inline-flex items-center gap-0.5">
      {costPips(cost).map((pip) => (
        <Pip key={pip.key} pip={pip} sizeClass={sizeClass} />
      ))}
    </span>
  )
}

/** Props of `ManaMade`. */
export interface ManaMadeProps {
  gives: Gives
  /** Size classes for one symbol, such as `size-4 text-xs`. */
  sizeClass: string
  /** Spell out "sacrifice" beside a source that is sacrificed; the corner strip keeps it out. */
  words?: boolean
  className?: string
}

/**
 * What a source gives, as symbols: `{1}:` first when a use costs mana, then the mana of each way (`{C}{C}`, `{W}/{U}`
 * with a slash between the ways) or a five-colour disc for any colour. One spoken name, the symbols are decoration.
 */
export function ManaMade({ gives, sizeClass, words = false, className }: ManaMadeProps) {
  return (
    <span
      role="img"
      aria-label={madeWords(gives)}
      data-slot="mana-made"
      className={cn('inline-flex flex-wrap items-center gap-x-1 gap-y-0.5', className)}
    >
      {gives.cost.length === 0 ? null : (
        <>
          <CostRun cost={gives.cost} sizeClass={sizeClass} />
          <span aria-hidden="true">:</span>
        </>
      )}
      {gives.any ? (
        <AnyPip sizeClass={sizeClass} />
      ) : (
        gives.ways.map((way, at) => (
          <Fragment key={`${way.made.join('')}-${at}`}>
            {at === 0 ? null : (
              <span aria-hidden="true" className="text-muted-foreground">
                /
              </span>
            )}
            <MadeRun made={way.made} sizeClass={sizeClass} />
          </Fragment>
        ))
      )}
      {words && gives.sacrifice ? (
        <span aria-hidden="true" className="text-[0.85em] text-muted-foreground">
          sacrifice
        </span>
      ) : null}
    </span>
  )
}
