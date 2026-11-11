/**
 * Rules text as the preview draws it: mana symbols as the card's coloured discs, the tap and untap marks as icons, each
 * "target …" phrase with a crosshair and set apart, and keywords as `KeywordText` does. Every symbol keeps its printed
 * form for a screen reader and for copying, so the text reads the same without the pictures.
 */
import { Crosshair, RotateCcw, RotateCw } from 'lucide-react'
import { cn } from '@/lib/utils'
import { KeywordText } from './Keyword'
import { Pip } from './ManaPips'
import { PIP } from './cardSize'
import { rulesPieces, splitCost } from './rulesText'

const SYMBOL = 'mx-px inline-flex size-4 align-[-3px] text-xs'

/** One rules text in pieces. */
export function RulesText({ text, className }: { text: string; className?: string }) {
  return (
    <span className={className}>
      {rulesPieces(text).map((piece, index) => {
        switch (piece.kind) {
          case 'text':
            return <KeywordText key={index} text={piece.text} />
          case 'mana':
            return (
              <span key={index} data-symbol={piece.pip.symbol}>
                <span className="sr-only">{piece.text}</span>
                {piece.pip.split === undefined ? (
                  // The letter is drawn by CSS, so the line's text stays the card's words.
                  <span
                    aria-hidden="true"
                    data-letter={piece.pip.text}
                    className={cn(
                      SYMBOL,
                      'items-center justify-center rounded-full leading-none font-bold ring-1 ring-foreground/25 before:content-[attr(data-letter)]',
                      PIP[piece.pip.tone],
                    )}
                  />
                ) : (
                  <Pip pip={piece.pip} sizeClass={SYMBOL} />
                )}
              </span>
            )
          case 'tap':
          case 'untap': {
            const Icon = piece.kind === 'tap' ? RotateCw : RotateCcw
            return (
              <span key={index} data-symbol={piece.kind}>
                <span className="sr-only">{piece.text}</span>
                <span
                  aria-hidden="true"
                  className={cn(
                    SYMBOL,
                    'items-center justify-center rounded-full bg-muted text-foreground ring-1 ring-foreground/25',
                  )}
                >
                  <Icon className="size-2.5" strokeWidth={3} />
                </span>
              </span>
            )
          }
          case 'target':
            return (
              <span key={index} data-slot="rules-target" className="font-medium text-foreground">
                <Crosshair aria-hidden="true" className="mr-0.5 inline size-3 align-[-1px] text-primary" />
                <KeywordText text={piece.text} />
              </span>
            )
        }
      })}
    </span>
  )
}

/**
 * An activated ability: its keyword name in bold, the cost in a tinted box, then the effect. The words are those of the
 * card (`text`), so an ability without a cost is its effect alone.
 */
export function AbilityLine({ text, keyword }: { text: string; keyword?: string | undefined }) {
  const inner = keyword !== undefined && text.startsWith(`${keyword} (`) && text.endsWith(')')
  const body = inner && keyword !== undefined ? text.slice(keyword.length + 2, -1) : text
  const { cost, effect } = splitCost(body)
  return (
    <span data-text={text}>
      {inner && keyword !== undefined ? (
        <b className="font-semibold">
          <RulesText text={keyword} />{' '}
        </b>
      ) : null}
      {cost === '' ? null : (
        <>
          <span data-slot="ability-cost" className="rounded bg-muted px-1 py-px font-medium">
            <RulesText text={cost} />
          </span>
          {': '}
        </>
      )}
      <RulesText text={effect} />
    </span>
  )
}
