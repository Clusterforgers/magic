/** The number that floats up from a card or a life total: "-3" for damage, "+4" for life gained. Decoration only. */
export function FxFloat({ kind, amount }: { kind: 'damaged' | 'healed'; amount: number }) {
  if (amount <= 0) return null
  return (
    <span aria-hidden="true" data-slot="fx-float" data-fx-float={kind} className="fx-float">
      {kind === 'damaged' ? '-' : '+'}
      {amount}
    </span>
  )
}
