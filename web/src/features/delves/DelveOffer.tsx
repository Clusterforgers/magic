/** The first choice waiting for the player: a card pick, a relic pick, the shop, the campfire or an event. */
import { Button } from '@/components/ui/button'
import type { CustomCardView, DelveContent, Offer, Pick } from '@/protocol/generated'
import { CustomCardFrame } from './CustomCardFrame'

/** Prices the server charges (`mtg-campaign`, run.rs). */
export const CARD_PRICE = 35
export const DRAUGHT_PRICE = 30

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

export function DelveOffer({
  offer,
  gold,
  content,
  busy,
  onPick,
}: {
  offer: Offer
  gold: number
  content: DelveContent
  busy: boolean
  onPick: (pick: Pick) => void
}) {
  const take = (i: number) => onPick({ Take: i })
  if (offer === 'Campfire') {
    return (
      <section aria-label="Campfire" className="surface grid gap-3 p-4">
        <h2 className="text-lg font-semibold">Campfire</h2>
        <p className="text-sm text-muted-foreground">
          Rest to heal 20 life, or reforge: pick a new card for your deck.
        </p>
        <div className="flex gap-2">
          <Button disabled={busy} onClick={() => onPick('Rest')}>
            Rest
          </Button>
          <Button variant="outline" disabled={busy} onClick={() => onPick('Reforge')}>
            Reforge
          </Button>
        </div>
      </section>
    )
  }
  if ('Cards' in offer) {
    return (
      <section aria-label="Card pick" className="surface grid gap-3 p-4">
        <h2 className="text-lg font-semibold">Pick a card</h2>
        <p className="text-sm text-muted-foreground">
          It replaces a basic land of your deck for the rest of the run.
        </p>
        <ul className="flex flex-wrap gap-2">
          {offer.Cards.cards.map((name, i) => (
            <li key={name}>
              <Button variant="outline" disabled={busy} onClick={() => take(i)}>
                Take {name}
              </Button>
            </li>
          ))}
        </ul>
        <Button variant="ghost" className="justify-self-start" disabled={busy} onClick={() => onPick('Skip')}>
          Skip for 10 gold
        </Button>
      </section>
    )
  }
  if ('Relics' in offer) {
    return (
      <section aria-label="Relic pick" className="surface grid gap-3 p-4">
        <h2 className="text-lg font-semibold">Pick a relic</h2>
        <div className="flex flex-wrap gap-3">
          {offer.Relics.relics.map((id, i) => (
            <CustomCardFrame key={id} card={card(content, id)}>
              <Button size="sm" disabled={busy} onClick={() => take(i)}>
                Take
              </Button>
            </CustomCardFrame>
          ))}
        </div>
        <Button variant="ghost" className="justify-self-start" disabled={busy} onClick={() => onPick('Skip')}>
          Take none
        </Button>
      </section>
    )
  }
  if ('Shop' in offer) {
    return (
      <section aria-label="Shop" className="surface grid gap-3 p-4">
        <h2 className="text-lg font-semibold">Goblin trader</h2>
        <p className="text-sm text-muted-foreground">You have {gold} gold.</p>
        <div className="flex flex-wrap gap-3">
          {offer.Shop.items.map((item, i) => {
            if (item === 'Draught') {
              return (
                <Button
                  key="draught"
                  variant="outline"
                  disabled={busy || gold < DRAUGHT_PRICE}
                  onClick={() => take(i)}
                >
                  Healing draught: 15 life ({DRAUGHT_PRICE} gold)
                </Button>
              )
            }
            if ('Card' in item) {
              return (
                <Button
                  key={item.Card.name}
                  variant="outline"
                  disabled={busy || item.Card.sold || gold < CARD_PRICE}
                  onClick={() => take(i)}
                >
                  {item.Card.sold ? `${item.Card.name} (sold)` : `${item.Card.name} (${CARD_PRICE} gold)`}
                </Button>
              )
            }
            const relic = card(content, item.Relic.id)
            return (
              <CustomCardFrame key={item.Relic.id} card={relic}>
                <Button
                  size="sm"
                  disabled={busy || item.Relic.sold || gold < item.Relic.price}
                  onClick={() => take(i)}
                >
                  {item.Relic.sold ? 'Sold' : `Buy (${item.Relic.price} gold)`}
                </Button>
              </CustomCardFrame>
            )
          })}
        </div>
        <Button variant="ghost" className="justify-self-start" disabled={busy} onClick={() => onPick('Skip')}>
          Leave the shop
        </Button>
      </section>
    )
  }
  const event = content.events.find((e) => e.id === offer.Event.event)
  return (
    <section aria-label="Event" className="surface grid gap-3 p-4">
      <h2 className="text-lg font-semibold">{event?.title ?? 'An event'}</h2>
      <p className="text-sm">{event?.text}</p>
      <div className="flex flex-wrap gap-2">
        {(event?.options ?? []).map((label, i) => (
          <Button key={label} variant="outline" disabled={busy} onClick={() => take(i)}>
            {label}
          </Button>
        ))}
      </div>
    </section>
  )
}
