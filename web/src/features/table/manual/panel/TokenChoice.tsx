import type { ReactNode } from 'react'
import { Input } from '@/components/ui/input'
import { NativeSelect, NativeSelectOptGroup, NativeSelectOption } from '@/components/ui/native-select'
import type { TokenOfferView } from '@/protocol/generated'
import { COMMON_TOKENS, GRANT_KEYWORDS } from '../offers'
import {
  CUSTOM,
  cardChoice,
  cardTokenOf,
  offerLabel,
  TOKEN_COLORS,
  TOKEN_NAME_MAX,
  TOKEN_STAT_MAX,
  TOKEN_SUBTYPES_MAX,
} from './customToken'
import type { TokenChoice as TokenChoiceValue, TokenDraft } from './customToken'

interface Props {
  draft: TokenDraft
  set: (next: Partial<TokenDraft>) => void
  /** The tokens the seat's cards make. */
  offers: readonly TokenOfferView[]
}

function Labelled({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="grid gap-0.5 text-xs text-muted-foreground">
      {label}
      {children}
    </label>
  )
}

const STAT_INPUT = 'h-8 tabular-nums pointer-coarse:h-11'

/** One of the creature's numbers; an emptied field is not a number, so the preview says what is wrong. */
function Stat({
  label,
  value,
  onChange,
}: {
  label: string
  value: number
  onChange: (value: number) => void
}) {
  return (
    <Labelled label={label}>
      <Input
        type="number"
        inputMode="numeric"
        min={0}
        max={TOKEN_STAT_MAX}
        value={Number.isNaN(value) ? '' : value}
        onChange={(event) => onChange(event.currentTarget.valueAsNumber)}
        className={STAT_INPUT}
      />
    </Labelled>
  )
}

/** A checkbox of the custom token's form, with its words beside it. */
function Check({
  label,
  checked,
  onChange,
}: {
  label: string
  checked: boolean
  onChange: (on: boolean) => void
}) {
  return (
    <label className="flex items-center gap-1.5 text-sm text-foreground pointer-coarse:min-h-11">
      <input type="checkbox" checked={checked} onChange={(event) => onChange(event.currentTarget.checked)} />
      {label}
    </label>
  )
}

/** What a token of the seat's cards does, in its own words, under the list once it is chosen. */
function OfferRules({ offer }: { offer: TokenOfferView }) {
  return (
    <div data-slot="card-token" className="grid gap-0.5 rounded-md border border-border px-2 py-1.5 text-xs">
      <p className="text-muted-foreground">{`Made by ${offer.card}`}</p>
      {(offer.rules ?? []).length === 0 ? (
        <p className="text-muted-foreground">No abilities.</p>
      ) : (
        <ul className="grid gap-0.5 text-foreground">
          {(offer.rules ?? []).map((rule, at) => (
            <li key={at}>{rule}</li>
          ))}
        </ul>
      )}
    </div>
  )
}

/**
 * The list of tokens: the common ones, those the seat's cards make (with every ability; the card is named under the
 * list, and in the option too when two cards make tokens of the same name and size) and, last, "Custom creature…"
 * which asks for the creature's name, power, toughness, colors, types, keywords and subtypes.
 */
export function TokenChoice({ draft, set, offers }: Props) {
  const offer = cardTokenOf(draft.token, offers)
  return (
    <>
      <NativeSelect
        aria-label="Token"
        value={draft.token}
        onChange={(event) => set({ token: event.currentTarget.value as TokenChoiceValue })}
      >
        <NativeSelectOptGroup label="Common tokens">
          {COMMON_TOKENS.map((token) => (
            <NativeSelectOption key={token} value={token}>
              {token}
            </NativeSelectOption>
          ))}
        </NativeSelectOptGroup>
        {offers.length === 0 ? null : (
          <NativeSelectOptGroup label="Your cards make">
            {offers.map((each, at) => (
              <NativeSelectOption key={`${each.card}:${each.index}`} value={cardChoice(at)}>
                {offers.filter((other) => offerLabel(other) === offerLabel(each)).length > 1
                  ? `${offerLabel(each)} (${each.card})`
                  : offerLabel(each)}
              </NativeSelectOption>
            ))}
          </NativeSelectOptGroup>
        )}
        <NativeSelectOption value={CUSTOM}>Custom creature…</NativeSelectOption>
      </NativeSelect>
      {offer === undefined ? null : <OfferRules offer={offer} />}
      {draft.token === CUSTOM ? (
        <div data-slot="custom-token" className="grid gap-2">
          <Labelled label="Name of the token">
            <Input
              // The field appears when "Custom creature…" is chosen, and the name is what comes next.
              autoFocus
              value={draft.name}
              maxLength={TOKEN_NAME_MAX}
              onChange={(event) => set({ name: event.currentTarget.value })}
              className={STAT_INPUT}
            />
          </Labelled>
          <div className="grid grid-cols-2 gap-2">
            <Stat label="Power" value={draft.power} onChange={(power) => set({ power })} />
            <Stat label="Toughness" value={draft.toughness} onChange={(toughness) => set({ toughness })} />
          </div>
          <fieldset className="flex flex-wrap gap-x-3 gap-y-0.5">
            <legend className="text-xs text-muted-foreground">Colors</legend>
            {TOKEN_COLORS.map((color) => (
              <Check
                key={color}
                label={color}
                checked={draft.colors.includes(color)}
                onChange={(on) =>
                  set({
                    colors: on ? [...draft.colors, color] : draft.colors.filter((each) => each !== color),
                  })
                }
              />
            ))}
          </fieldset>
          <fieldset className="flex flex-wrap gap-x-3 gap-y-0.5">
            <legend className="text-xs text-muted-foreground">Also</legend>
            <Check label="Artifact" checked={draft.artifact} onChange={(artifact) => set({ artifact })} />
            <Check
              label="Enchantment"
              checked={draft.enchantment}
              onChange={(enchantment) => set({ enchantment })}
            />
          </fieldset>
          <fieldset className="flex flex-wrap gap-x-3 gap-y-0.5">
            <legend className="text-xs text-muted-foreground">Abilities</legend>
            {GRANT_KEYWORDS.map(([keyword, words]) => (
              <Check
                key={keyword}
                label={words}
                checked={draft.keywords.includes(keyword)}
                onChange={(on) =>
                  set({
                    keywords: on
                      ? [...draft.keywords, keyword]
                      : draft.keywords.filter((each) => each !== keyword),
                  })
                }
              />
            ))}
          </fieldset>
          <Labelled label={`Subtypes (up to ${TOKEN_SUBTYPES_MAX}, empty: the name)`}>
            <Input
              value={draft.subtypes}
              onChange={(event) => set({ subtypes: event.currentTarget.value })}
              className={STAT_INPUT}
            />
          </Labelled>
        </div>
      ) : null}
    </>
  )
}
