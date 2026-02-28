# Pack Marketplace

The ClawDE Pack Marketplace lets community developers publish and sell packs.

## For Users

Browse packs at `base.clawde.io/marketplace`. Install with:

```bash
clawd pack install {slug}
```

Free packs install immediately. Paid packs open a purchase page in your browser.

### Paid Pack License

After purchase, a signed install token is stored locally:
- Token is verified on every daemon start
- Automatically renewed when <24h remaining
- Revoked when subscription cancels

## For Authors

### Revenue Model

- **70%** goes to you, **30%** platform fee
- Monthly payouts via Stripe Connect
- Pricing: free / one-time / monthly subscription

### Publishing a Paid Pack

Add to `clawde-pack.json`:

```json
{
  "price_usd": 4.99,
  "price_type": "monthly"
}
```

Then: `clawd pack publish`

Connect your Stripe account at `base.clawde.io/marketplace/author` to receive payouts.

### Author Dashboard

- View earnings by period
- Payout history with status (pending / processing / paid)
- Stripe Connect setup and account status

## Architecture

```
User installs pack
    → clawd detects paid pack
    → opens registry.clawde.io/packs/{slug}/purchase
    → Stripe Checkout
    → webhook: checkout.session.completed
    → api.clawde.io issues signed install token
    → clawd polls for token, stores in ~/.clawd/pack_tokens/
    → pack loads
```

Monthly payout job (1st of month, 02:00 UTC):
```
api.clawde.io aggregates prior-month revenue
    → calculates 70/30 split per author
    → Stripe Connect transfer to author account
```
