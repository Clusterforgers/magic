/**
 * The first choice waiting for the player, each as a scene of its own: the loot's card and relic picks as cards, the
 * goblin trader's wares, the campfire's two paths and an event as a story card with its choices.
 */
import {
  CoinsIcon,
  FlameIcon,
  FlaskConicalIcon,
  GemIcon,
  HammerIcon,
  HeartPulseIcon,
  ScrollTextIcon,
  SparklesIcon,
  StoreIcon,
} from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { CustomCardView, DelveContent, Offer, Pick } from '@/protocol/generated'
import { CustomCardFrame } from './CustomCardFrame'
import { isRisky, MAX_LIFE } from './model'
import { NamedCard } from './NamedCard'
import type { FindCard } from './findCard'

/** Prices the server charges (`mtg-campaign`, run.rs). */
export const CARD_PRICE = 35
export const DRAUGHT_PRICE = 30
/** Life a campfire rest and a draught heal (`docs/delves.md`, Shape). */
export const REST_HEAL = 20
export const DRAUGHT_HEAL = 15

function card(content: DelveContent, id: string): CustomCardView {
  return (
    content.cards.find((c) => c.id === id) ?? {
      id,
      name: id,
      kind: 'Relic',
      type_line: 'Relic',
      text: '',
      art: '',
      role: '',
      price: 0,
    }
  )
}

/** A scene of the run: an emblem, a title, a line of what it is and the player's gold, over the dungeon's stone. */
function Scene({
  label,
  icon: Icon,
  title,
  blurb,
  gold,
  children,
}: {
  label: string
  icon: LucideIcon
  title: string
  blurb?: ReactNode
  gold?: number
  children: ReactNode
}) {
  return (
    <section
      aria-label={label}
      data-scene={label}
      className="grid gap-4 rounded-xl border bg-delve-stone p-4 shadow-e1 motion-safe:animate-in motion-safe:fade-in motion-safe:slide-in-from-bottom-2 motion-safe:duration-slow sm:p-6"
    >
      <div className="flex flex-wrap items-center gap-3">
        <span className="grid size-12 shrink-0 place-items-center rounded-full border-2 border-delve bg-surface text-delve">
          <Icon aria-hidden="true" className="size-6" />
        </span>
        <div className="grid min-w-0 flex-1 gap-1">
          <h2 className="text-lg font-semibold">{title}</h2>
          {blurb ? <p className="text-sm text-muted-foreground">{blurb}</p> : null}
        </div>
        {gold === undefined ? null : (
          <span className="inline-flex items-center gap-1 rounded-full border border-relic bg-surface px-3 py-1 text-sm font-semibold">
            <CoinsIcon aria-hidden="true" className="size-4 text-relic" />
            {gold} gold
          </span>
        )}
      </div>
      {children}
    </section>
  )
}

/** A big choice of the campfire: an icon, a name, what it does and its button. */
function Path({
  icon: Icon,
  title,
  text,
  action,
  primary,
  busy,
  onPick,
}: {
  icon: LucideIcon
  title: string
  text: string
  action: string
  primary?: boolean
  busy: boolean
  onPick: () => void
}) {
  return (
    <div className="grid content-between gap-3 rounded-xl border bg-surface p-4 shadow-e1">
      <div className="flex items-start gap-3">
        <Icon aria-hidden="true" className="size-8 shrink-0 text-delve" />
        <div className="grid gap-1">
          <h3 className="font-semibold">{title}</h3>
          <p className="text-sm text-muted-foreground">{text}</p>
        </div>
      </div>
      <Button variant={primary ? 'default' : 'outline'} disabled={busy} onClick={onPick}>
        {action}
      </Button>
    </div>
  )
}

export function DelveOffer({
  offer,
  gold,
  life,
  content,
  busy,
  find,
  onPick,
}: {
  offer: Offer
  gold: number
  /** The hero's life, for what a rest would heal. */
  life?: number
  content: DelveContent
  busy: boolean
  /** Finds offered cards' pictures; the card search by default. */
  find?: FindCard
  onPick: (pick: Pick) => void
}) {
  const take = (i: number) => onPick({ Take: i })
  if (offer === 'Campfire') {
    const healed = life === undefined ? null : Math.min(MAX_LIFE, life + REST_HEAL)
    return (
      <Scene
        label="Campfire"
        icon={FlameIcon}
        title="Campfire"
        blurb="The party catches its breath. Choose one."
      >
        <div className="grid gap-3 sm:grid-cols-2">
          <Path
            icon={HeartPulseIcon}
            title="Rest"
            text={
              healed === null || life === undefined
                ? `Heal ${REST_HEAL} life.`
                : `Heal ${REST_HEAL} life: ${life} → ${healed}.`
            }
            action="Rest"
            primary
            busy={busy}
            onPick={() => onPick('Rest')}
          />
          <Path
            icon={HammerIcon}
            title="Reforge"
            text="Pick a new card for your deck; it replaces a basic land."
            action="Reforge"
            busy={busy}
            onPick={() => onPick('Reforge')}
          />
        </div>
      </Scene>
    )
  }
  if ('Cards' in offer) {
    return (
      <Scene
        label="Card pick"
        icon={SparklesIcon}
        title="Pick a card"
        blurb="It replaces a basic land of your deck for the rest of the run."
      >
        <ul className="grid grid-cols-2 gap-3 sm:flex sm:flex-wrap sm:gap-4">
          {offer.Cards.cards.map((name, i) => (
            <li key={name}>
              <NamedCard name={name} find={find}>
                <Button size="sm" disabled={busy} onClick={() => take(i)} aria-label={`Take ${name}`}>
                  Take
                </Button>
              </NamedCard>
            </li>
          ))}
        </ul>
        <Button variant="ghost" className="justify-self-start" disabled={busy} onClick={() => onPick('Skip')}>
          Skip for 10 gold
        </Button>
      </Scene>
    )
  }
  if ('Relics' in offer) {
    const relics = offer.Relics.relics.map((id) => card(content, id))
    return (
      <Scene
        label="Relic pick"
        icon={GemIcon}
        title="Pick a relic"
        blurb="Take one for free. Relics last the whole run. Dangerous ones help the boss too."
      >
        <ul className="grid grid-cols-2 gap-3 sm:flex sm:flex-wrap sm:gap-4">
          {relics.map((relic, i) => (
            <li key={relic.id}>
              <CustomCardFrame card={relic}>
                <Button
                  size="sm"
                  className="flex-1"
                  variant={isRisky(relic) ? 'destructive' : 'default'}
                  disabled={busy}
                  onClick={() => take(i)}
                  aria-label={`Take ${relic.name}`}
                >
                  Take
                </Button>
              </CustomCardFrame>
            </li>
          ))}
        </ul>
        <Button variant="ghost" className="justify-self-start" disabled={busy} onClick={() => onPick('Skip')}>
          Take none
        </Button>
      </Scene>
    )
  }
  if ('Shop' in offer) {
    return (
      <Scene
        label="Shop"
        icon={StoreIcon}
        title="Goblin trader"
        blurb="“Shiny things for shiny coins.” Buy as much as your purse allows."
        gold={gold}
      >
        <ul className="grid grid-cols-2 gap-3 sm:flex sm:flex-wrap sm:gap-4">
          {offer.Shop.items.map((item, i) => {
            if (item === 'Draught') {
              const short = gold < DRAUGHT_PRICE
              return (
                <li key="draught" className="grid w-full content-start gap-2 sm:w-44">
                  <div className="grid aspect-[5/7] place-items-center rounded-xl border-2 border-success bg-surface shadow-e1">
                    <FlaskConicalIcon aria-hidden="true" className="size-12 text-success" />
                  </div>
                  <p className="text-sm font-medium leading-tight">Healing draught</p>
                  <p className="text-xs text-muted-foreground">Heal {DRAUGHT_HEAL} life.</p>
                  <Button
                    size="sm"
                    variant="outline"
                    disabled={busy || short}
                    onClick={() => take(i)}
                    aria-label={`Healing draught: ${DRAUGHT_HEAL} life (${DRAUGHT_PRICE} gold)`}
                  >
                    {short ? `Needs ${DRAUGHT_PRICE} gold` : `Buy · ${DRAUGHT_PRICE} gold`}
                  </Button>
                </li>
              )
            }
            if ('Card' in item) {
              const short = gold < CARD_PRICE
              return (
                <li key={item.Card.name}>
                  <NamedCard name={item.Card.name} sold={item.Card.sold} find={find}>
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={busy || item.Card.sold || short}
                      onClick={() => take(i)}
                      aria-label={
                        item.Card.sold ? `${item.Card.name} (sold)` : `${item.Card.name} (${CARD_PRICE} gold)`
                      }
                    >
                      {item.Card.sold
                        ? 'Sold'
                        : short
                          ? `Needs ${CARD_PRICE} gold`
                          : `Buy · ${CARD_PRICE} gold`}
                    </Button>
                  </NamedCard>
                </li>
              )
            }
            const relic = card(content, item.Relic.id)
            const short = gold < item.Relic.price
            return (
              <li key={item.Relic.id}>
                <CustomCardFrame card={relic} className={cn(item.Relic.sold && 'opacity-60')}>
                  <Button
                    size="sm"
                    className="flex-1"
                    disabled={busy || item.Relic.sold || short}
                    onClick={() => take(i)}
                    aria-label={item.Relic.sold ? 'Sold' : `Buy (${item.Relic.price} gold)`}
                  >
                    {item.Relic.sold
                      ? 'Sold'
                      : short
                        ? `Needs ${item.Relic.price} gold`
                        : `Buy · ${item.Relic.price} gold`}
                  </Button>
                </CustomCardFrame>
              </li>
            )
          })}
        </ul>
        <Button
          variant="outline"
          className="justify-self-start"
          disabled={busy}
          onClick={() => onPick('Skip')}
        >
          Leave the shop
        </Button>
      </Scene>
    )
  }
  const event = content.events.find((e) => e.id === offer.Event.event)
  return (
    <Scene label="Event" icon={ScrollTextIcon} title={event?.title ?? 'An event'} gold={gold}>
      <article className="mx-auto grid w-full max-w-prose gap-4 rounded-xl border-2 border-relic bg-surface p-4 shadow-e2 sm:p-6">
        <p className="text-base leading-relaxed whitespace-pre-line">{event?.text}</p>
        <ol className="grid gap-2">
          {(event?.options ?? []).map((label, i) => (
            <li key={label}>
              <Button
                variant="outline"
                className="h-auto min-h-11 w-full justify-start py-2 text-left whitespace-normal"
                disabled={busy}
                onClick={() => take(i)}
                aria-label={label}
              >
                <span
                  aria-hidden="true"
                  className="grid size-6 shrink-0 place-items-center rounded-full bg-delve text-xs font-semibold text-on-delve"
                >
                  {i + 1}
                </span>
                {label}
              </Button>
            </li>
          ))}
        </ol>
      </article>
    </Scene>
  )
}
