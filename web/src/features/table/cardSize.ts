import type { Frame, Highlight, PipTone } from './cardModel'

/** How big a card face is drawn: compact for crowded tables, large for the preview. */
export type CardSize = 'compact' | 'normal' | 'medium' | 'large'

interface SizeClasses {
  /** Sets the box (`--card-w` by `--card-h`); every size keeps the 5:7 shape of a card. */
  vars: string
  name: string
  text: string
  pip: string
}

/** The smallest text on a card face, in rem (11 px): the box shrinks with the zone, the type never does. */
export const MIN_TEXT_REM = 0.6875

/** The box and text classes of each card size. Whole class names, so Tailwind can see them. */
export const SIZE: Record<CardSize, SizeClasses> = {
  compact: {
    vars: '[--card-w:3.75rem] [--card-h:5.25rem]',
    name: 'text-[0.6875rem]',
    text: 'text-[0.6875rem]',
    pip: 'size-3 text-[0.6875rem]',
  },
  normal: {
    vars: '[--card-w:6rem] [--card-h:8.4rem]',
    name: 'text-[0.72rem]',
    text: 'text-[0.6875rem]',
    pip: 'size-4 text-[0.6875rem]',
  },
  medium: {
    vars: '[--card-w:10rem] [--card-h:14rem]',
    name: 'text-sm',
    text: 'text-[0.7rem]',
    pip: 'size-4 text-[0.6875rem]',
  },
  large: {
    vars: '[--card-w:15rem] [--card-h:21rem]',
    name: 'text-base',
    text: 'text-xs',
    pip: 'size-5 text-xs',
  },
}

/** The border and tint of each frame colour. */
export const FRAME: Record<Frame, string> = {
  white: 'border-border-strong bg-mana-white/25',
  blue: 'border-mana-blue bg-mana-blue/12',
  black: 'border-mana-black bg-mana-black/12',
  red: 'border-mana-red bg-mana-red/12',
  green: 'border-mana-green bg-mana-green/12',
  gold: 'border-mana-gold bg-mana-gold/15',
  colorless: 'border-mana-colorless bg-mana-colorless/12',
}

/** The fill of a mana symbol. */
export const PIP: Record<PipTone, string> = {
  white: 'bg-mana-white text-on-mana-white',
  blue: 'bg-mana-blue text-on-mana-blue',
  black: 'bg-mana-black text-on-mana-black',
  red: 'bg-mana-red text-on-mana-red',
  green: 'bg-mana-green text-on-mana-green',
  gold: 'bg-mana-gold text-on-mana-gold',
  colorless: 'bg-mana-colorless text-on-mana-colorless',
  generic: 'bg-muted text-foreground',
}

/** The ink that reads on a fill of this tone. */
export const PIP_INK: Record<PipTone, string> = {
  white: 'text-on-mana-white',
  blue: 'text-on-mana-blue',
  black: 'text-on-mana-black',
  red: 'text-on-mana-red',
  green: 'text-on-mana-green',
  gold: 'text-on-mana-gold',
  colorless: 'text-on-mana-colorless',
  generic: 'text-foreground',
}

// A split symbol (hybrid) is a diagonal gradient with a hard edge in the middle: `from` is the upper left half and
// `to` the lower right one. Whole class names, so Tailwind can see them.
/** The upper left half of a split mana symbol. */
export const PIP_FROM: Record<PipTone, string> = {
  white: 'from-mana-white',
  blue: 'from-mana-blue',
  black: 'from-mana-black',
  red: 'from-mana-red',
  green: 'from-mana-green',
  gold: 'from-mana-gold',
  colorless: 'from-mana-colorless',
  generic: 'from-muted',
}

/** The lower right half of a split mana symbol. */
export const PIP_TO: Record<PipTone, string> = {
  white: 'to-mana-white',
  blue: 'to-mana-blue',
  black: 'to-mana-black',
  red: 'to-mana-red',
  green: 'to-mana-green',
  gold: 'to-mana-gold',
  colorless: 'to-mana-colorless',
  generic: 'to-muted',
}

/** How the decision layer's marks look on a card. */
export const HIGHLIGHT: Record<Highlight, string> = {
  legal: 'ring-2 ring-primary',
  selected: 'ring-4 ring-primary',
  dimmed: 'opacity-40 saturate-50',
}
