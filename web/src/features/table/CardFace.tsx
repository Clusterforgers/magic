import { CARD_CORNER } from '@/features/cards/CardImage'
import { useImageChoice } from '@/features/cards/imageSetting'
import { imageSizeFor, imageSourceOf, showsBack } from '@/features/cards/images'
import type { ImageSize } from '@/features/cards/images'
import { Link as LinkIcon, Unlink } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { ObjectView } from '@/protocol/generated'
import {
  costPips,
  counterLabels,
  frameOf,
  keywordLine,
  loyaltyOf,
  powerToughness,
  statusChips,
  typeLine,
} from './cardModel'
import type { CombatRole, Frame, Links } from './cardModel'
import { FRAME, SIZE } from './cardSize'
import type { CardSize } from './cardSize'
import { isUnattached } from '@/play/hosts'
import { BackFace, ImageFace, useDrawnWidth } from './ImageFace'
import { GrantMarks } from './GrantMarks'
import { NoteMark, RevealedMark } from './HandMarks'
import { ByHandMark } from './manual/ByHand'
import { ManaPips } from './ManaPips'
import { statBadge } from './overlay'
import { usePrinted } from './printed'
import type { CardNote } from './prompt/marks'

interface CardFaceProps {
  object: ObjectView
  role: CombatRole | null
  size: CardSize
  /** The box comes from an enclosing row (`Card` `fit`); `size` then only picks which parts of the face are drawn. */
  fit?: boolean
  /** A compact face also draws the mana cost. */
  cost?: boolean
  /** Keep a tapped card upright, as the preview does: it says "tapped" in words instead. */
  upright?: boolean
  /** What the active prompt writes on the card, such as "At Ana"; drawn after the status labels. */
  note?: CardNote | undefined
  /** The face tops a pile of equal permanents: two edges peek out behind it. */
  stacked?: boolean
  /** What it is attached to and what is attached to it, by name: a cue beside the counters. */
  links?: Links | undefined
  /** The card is face up to every seat (`revealedPlace`): the "Revealed" mark. */
  revealed?: boolean
  /** The note a seat put on the card (`ObjectView.note`): the mark with its first words. */
  handNote?: string | undefined
  /** The picture size to ask for; the one the drawn width needs when left out. */
  imageSize?: ImageSize
}

/** The lines of name a face can spare: a face on its side is low, and a compact one has no type line (nor a cost, unless asked). */
function nameLinesOf(turned: boolean, full: boolean, size: CardSize): string {
  if (turned) return size === 'compact' ? 'line-clamp-1' : 'line-clamp-2'
  return full ? 'line-clamp-2' : 'line-clamp-3'
}

// A token's frame keeps its colour on the border and takes the back's fill.
const TOKEN_BORDER: Record<Frame, string> = {
  white: 'border-border-strong',
  blue: 'border-mana-blue',
  black: 'border-mana-black',
  red: 'border-mana-red',
  green: 'border-mana-green',
  gold: 'border-mana-gold',
  colorless: 'border-mana-colorless',
}

// The edges stay inside the 6 px between boxes of a line and the 4 px between lines.
const PILE = 'shadow-[2px_2px_0_0_var(--color-border-strong),4px_4px_0_0_var(--color-border)]'

// What the art slot leaves of the face's width for a mark.
const ART_SHARE = 0.88
// The height the name, cost, type line and stat box take from a face, and a row of chips with the gap under it.
const FACE_FRAME_PX = 70
const ART_ROW_PX = 20

/** The rows of the art slot of a face `height` px high that the marks of attachments may fill; the slot clips the rest. */
function artRoom(height: number, chips: number, links: Links | undefined): number {
  const rows = Math.floor((height - FACE_FRAME_PX) / ART_ROW_PX)
  return (
    rows - (chips > 0 ? 1 : 0) - (links?.host === undefined ? 0 : 1) - (links?.partner === undefined ? 0 : 1)
  )
}

/**
 * A card of the table: its picture with the state over it (`ImageFace`), the neutral back for a card the viewer may
 * not see, or the text face when pictures are off ("Card images": Text only) or the card has no picture source. The
 * text face stays under a picture while it loads and when it is missing.
 */
export function CardFace(props: CardFaceProps) {
  const { object, size, fit = false, upright = false, stacked = false } = props
  const choice = useImageChoice()
  const width = useDrawnWidth(size, fit)
  if (choice === 'text') return <TextFace {...props} />
  const turned = object.permanent?.tapped === true && !upright
  const box = fit ? null : SIZE[size].vars
  if (showsBack(object))
    return (
      <BackFace
        box={box}
        turned={turned}
        label={object.face_down === true ? 'Face-down card' : 'Hidden card'}
        object={object}
        role={props.role}
        size={size}
        note={props.note}
        links={props.links}
        revealed={props.revealed}
        handNote={props.handNote}
      />
    )
  const source = imageSourceOf(object)
  if (source === null) return <TextFace {...props} />
  return (
    <ImageFace
      source={source}
      box={box}
      turned={turned}
      stacked={stacked}
      imageSize={props.imageSize ?? imageSizeFor(width)}
      object={object}
      role={props.role}
      size={size}
      note={props.note}
      links={props.links}
      revealed={props.revealed}
      handNote={props.handNote}
    >
      {(status) => (
        <TextFace {...props} stacked={false} card={status === 'failed' && source.kind === 'token'} />
      )}
    </ImageFace>
  )
}

const PLATE = 'rounded-sm border border-foreground/30 bg-background/80'
// A text face is plain, or drawn as a card where a token's picture is missing: a rounded frame in the card's colour,
// plates for the name and the type line, a rules box and a boxed power/toughness.
const PARTS = {
  plain: {
    body: 'rounded-md border-2',
    name: 'px-1 pt-0.5',
    type: 'px-1 text-muted-foreground',
    rules: 'px-1',
    stat: 'self-end rounded-tl-md bg-foreground px-1.5 text-background',
  },
  card: {
    body: `${CARD_CORNER} border-[3px]`,
    name: `mx-0.5 mt-0.5 px-1 ${PLATE}`,
    type: `mx-0.5 px-1 ${PLATE}`,
    rules: `mx-0.5 mb-0.5 px-1 py-px ${PLATE} bg-background/90`,
    stat: 'mr-0.5 mb-0.5 self-end rounded-md border-2 border-foreground/70 bg-background px-1.5 text-foreground',
  },
}

interface TextFaceProps extends CardFaceProps {
  /** Draw it as a card: the face of a token whose picture the server has none of. */
  card?: boolean
}

/**
 * What a card looks like as text: name, cost, a place for art, type line and power/toughness, in its colour. Plain
 * `span`s so it can sit inside a button. A tapped card lies on its side inside the box of an upright one: a landscape
 * face as wide as the box and five sevenths as high, its text upright and as large as ever, plus the word "Tapped"
 * (never colour alone). A row therefore counts one box for it, tapped or not.
 */
export function TextFace({
  object,
  role,
  size,
  fit = false,
  cost = false,
  upright = false,
  note,
  stacked = false,
  links,
  revealed = false,
  handNote,
  card = false,
}: TextFaceProps) {
  const classes = SIZE[size]
  const width = useDrawnWidth(size, fit)
  const parts = PARTS[card ? 'card' : 'plain']
  const turned = object.permanent?.tapped === true && !upright
  const frame = frameOf(object)
  const full = size !== 'compact' && !turned
  const chips = statusChips(object, role)
  const keywords = keywordLine(object)
  const pt = powerToughness(object)
  const counters = counterLabels(object)
  const loyalty = loyaltyOf(object)
  const modified = statBadge(object, usePrinted(object), loyalty)?.modified ?? null
  // A compact face that draws its cost keeps two lines of name, so the name and the cost fit the top a hand row shows.
  const nameLines = nameLinesOf(turned, full || (cost && object.mana_cost !== null), size)
  // A token whose picture is missing still looks like a card: the card back's colours, its name, "Token" and its type.
  const token = object.kind === 'Token'
  // Only a token drawn as text is dark; one drawn as a card (its picture is missing) keeps its frame colour.
  const dark = token && !card
  return (
    <span
      data-slot="card-face"
      data-frame={frame}
      data-turned={turned || undefined}
      data-card-like={card || undefined}
      className={cn(
        'relative flex w-(--card-w) flex-col overflow-hidden text-left text-card-foreground ring-1 ring-foreground/15',
        parts.body,
        stacked ? PILE : 'shadow-sm',
        turned ? 'h-[calc(var(--card-w)*5/7)]' : 'h-(--card-h)',
        fit ? null : classes.vars,
        dark ? cn(TOKEN_BORDER[frame], 'bg-card-back text-white') : FRAME[frame],
      )}
    >
      <span className={cn(classes.name, parts.name, 'leading-tight font-semibold')}>
        <span className={cn('block break-words hyphens-auto', nameLines)}>{object.name}</span>
      </span>
      {(full || (cost && !turned)) && object.mana_cost !== null ? (
        <ManaPips
          pips={costPips(object.mana_cost)}
          sizeClass={classes.pip}
          label="Mana cost"
          className="px-1 pt-0.5"
        />
      ) : null}
      {/* The place for card art: fill this slot when ObjectView carries an image id. */}
      <span
        data-slot="card-art"
        className={cn(
          'mx-1 my-0.5 flex min-h-0 flex-1 flex-wrap content-start gap-0.5 overflow-hidden rounded-sm p-0.5',
          dark ? 'bg-black/25' : 'bg-foreground/5',
        )}
      >
        <ByHandMark object={object} spot="chip" compact={size === 'compact'} text={classes.text} />
        {chips.map(({ label, tone }) => (
          <span
            key={label}
            data-slot="card-status"
            data-tone={tone}
            className={cn(
              'h-fit rounded-sm px-1 leading-snug font-medium text-background',
              tone === 'danger' ? 'bg-destructive' : 'bg-foreground',
              classes.text,
            )}
          >
            {label}
          </span>
        ))}
        {counters.map((label) => (
          <span
            key={`counter-${label}`}
            data-slot="card-counter"
            className={cn(
              'h-fit rounded-sm border border-foreground bg-background px-1 leading-snug font-semibold text-foreground',
              classes.text,
            )}
          >
            {label}
          </span>
        ))}
        {links?.host === undefined ? null : (
          <span
            data-slot="card-attached-to"
            className={cn(
              'h-fit max-w-full rounded-sm border border-dashed border-foreground bg-background px-1 leading-snug break-words text-foreground',
              classes.text,
            )}
          >
            On {links.host}
          </span>
        )}
        <GrantMarks
          attaches={links?.attaches ?? []}
          compact={size === 'compact'}
          room={artRoom(turned ? (width * 5) / 7 : (width * 7) / 5, chips.length + counters.length, links)}
          span={width * ART_SHARE}
          text={classes.text}
        />
        {object.paired_with === undefined ? null : (
          <span
            data-slot="card-paired"
            className={cn(
              'inline-flex h-fit max-w-full items-center gap-0.5 rounded-sm border border-foreground bg-background px-1 leading-snug text-foreground',
              classes.text,
            )}
          >
            <LinkIcon aria-hidden="true" className="size-3 shrink-0" />
            <span className={cn(size === 'compact' && 'sr-only')}>Paired</span>
          </span>
        )}
        {isUnattached(object) ? (
          <span
            data-slot="card-unattached"
            className={cn(
              'inline-flex h-fit max-w-full items-center gap-0.5 rounded-sm border border-dashed border-foreground bg-background px-1 leading-snug text-foreground',
              classes.text,
            )}
          >
            <Unlink aria-hidden="true" className="size-3 shrink-0" />
            <span className={cn(size === 'compact' && 'sr-only')}>Unattached</span>
          </span>
        ) : null}
        {revealed ? <RevealedMark compact={size === 'compact'} text={classes.text} /> : null}
        {handNote === undefined ? null : (
          <NoteMark note={handNote} compact={size === 'compact'} text={classes.text} />
        )}
        {note?.label === undefined ? null : (
          <span
            data-slot="card-note"
            className={cn(
              'h-fit max-w-full rounded-sm bg-primary px-1 leading-snug font-medium break-words text-primary-foreground',
              classes.text,
            )}
          >
            {note.label}
          </span>
        )}
      </span>
      {full ? (
        <span
          data-slot="card-type-line"
          className={cn(classes.text, parts.type, 'line-clamp-2 leading-tight', dark && 'text-white/85')}
        >
          {token ? `Token ${typeLine(object)}` : typeLine(object)}
        </span>
      ) : null}
      {full && keywords !== null ? (
        <span
          data-slot="card-keywords"
          className={cn(classes.text, parts.rules, 'line-clamp-2 leading-tight')}
        >
          {keywords}
        </span>
      ) : null}
      {pt === null && loyalty === null ? (
        <span className="h-1" />
      ) : (
        <span
          data-slot={pt === null ? 'card-loyalty' : undefined}
          data-modified={modified ?? undefined}
          className={cn(
            classes.name,
            parts.stat,
            'leading-snug font-bold tabular-nums',
            modified === 'up' && 'border-background bg-support-verified text-background',
            modified === 'down' && 'border-background bg-destructive text-background',
          )}
        >
          {pt ?? loyalty}
        </span>
      )}
    </span>
  )
}
