/** The game's payment that "Pay for me" proposes on the dev table, which has no engine to find it. */
import type { PayOptions } from '@/protocol/generated'
import { completion, toPayment } from '../prompt/payment'

/** `options` with the payment the engine would propose, found as the prompt's own search finds one. */
export function withProposal(options: PayOptions): PayOptions {
  const rest: PayOptions = { ...options }
  delete rest.proposal
  if (!options.auto_available) return rest
  const whole = completion(options.cost, options.pool, [], [], {
    sources: options.sources,
    maxLife: options.max_life,
    ...(options.spend === undefined ? {} : { spend: options.spend }),
  })
  return whole === undefined ? rest : { ...rest, proposal: toPayment(whole.spends, whole.life) }
}
