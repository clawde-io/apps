/// Embedded provider knowledge bundles — V02.T33.
///
/// Static knowledge about cloud providers: API patterns, gotchas, rate limits,
/// deprecations. Injected as system prompt context on session.create (V02.T35).
use super::Provider;

/// Return the knowledge bundle text for a provider.
pub fn bundle_for_provider(provider: &Provider) -> &'static str {
    match provider {
        Provider::Hetzner => HETZNER_KNOWLEDGE,
        Provider::Vercel => VERCEL_KNOWLEDGE,
        Provider::Stripe => STRIPE_KNOWLEDGE,
        Provider::Cloudflare => CLOUDFLARE_KNOWLEDGE,
        Provider::ElasticEmail => ELASTIC_EMAIL_KNOWLEDGE,
        Provider::Supabase => SUPABASE_KNOWLEDGE,
        Provider::Neon => NEON_KNOWLEDGE,
    }
}

// ─── Hetzner ──────────────────────────────────────────────────────────────────

const HETZNER_KNOWLEDGE: &str = r#"
## Provider Knowledge: Hetzner Cloud

**API**: REST at `https://api.hetzner.cloud/v1/`. Auth: `Authorization: Bearer <token>`.
**Key gotchas**:
- Server names must be unique within a project (max 63 chars, lowercase alphanumeric + hyphens).
- Creating a server is async — poll `actions/{id}` until `status == "success"` before using the IP.
- Floating IPs must be in the same datacenter as the server. Region transfers are not supported.
- `cx22` = 2 vCPU / 4GB RAM / 40GB NVMe (~€3.79/mo). Minimum for production workloads.
- SSH keys must be added to project before server creation; use `ssh_keys: [key_id]` not `user_data` for key injection.
- Rate limit: 3600 requests/hour per token. Bulk operations use server action polling.
- Server deletion is permanent — no recycle bin. Always snapshot before destructive operations.
- **Regions**: `fsn1` (Falkenstein), `nbg1` (Nuremberg), `hel1` (Helsinki), `ash` (Ashburn), `hil` (Hillsboro).
"#;

// ─── Vercel ───────────────────────────────────────────────────────────────────

const VERCEL_KNOWLEDGE: &str = r#"
## Provider Knowledge: Vercel

**CLI**: `vercel deploy --prod`. Always use `--scope <team>` (not `--team`, deprecated in v50+).
**Key gotchas**:
- Environment variables set via `vercel env add` are NOT available during `vercel build` unless also in `.env.production`.
- `VERCEL_URL` is the deployment URL without https://. Use `NEXT_PUBLIC_VERCEL_URL` for client-side access.
- Build outputs must be in the `public/` directory (static) or `api/` directory (serverless functions).
- Edge functions: max 1MB compressed. No Node.js built-ins (fs, path). Use `next/headers` not `node:http`.
- Hobby plan: 100 GB-hours / month. Pro: 1000 GB-hours. Function timeout: 10s (Hobby), 60s (Pro), 800s (Enterprise).
- Deployments are immutable — you cannot overwrite a deployment. Promote an existing deployment to production with `vercel promote <url>`.
- `vercel.json` `rewrites` run before `redirects`. Middleware runs before both.
- `--scope` flag is required when working with team projects; without it commands target personal account.
"#;

// ─── Stripe ───────────────────────────────────────────────────────────────────

const STRIPE_KNOWLEDGE: &str = r#"
## Provider Knowledge: Stripe

**API**: `https://api.stripe.com/v1/`. Auth: `Authorization: Bearer sk_live_xxx` or test `sk_test_xxx`.
**Key gotchas**:
- Never log or store raw webhook payloads — they may contain PII. Verify signature with `stripe.webhooks.constructEvent()`.
- Webhook signatures use the raw request body. JSON.parse + re-stringify will break the signature.
- Idempotency keys: pass `Idempotency-Key` header for POST requests to safely retry on network failure.
- `price_id` vs `plan_id`: Plans are legacy. Always use Prices (created in dashboard or API). Price IDs start with `price_`.
- Test mode and live mode use separate API keys, separate objects, separate webhooks. Never mix them.
- Subscription `cancel_at_period_end: true` does NOT immediately cancel — it cancels at next billing date.
- Customer portal requires Stripe-hosted configuration. Enable in Dashboard → Billing → Customer portal.
- Rate limits: 100 reads/second, 100 writes/second per key. Use exponential backoff on 429 responses.
- Stripe CLI: `stripe listen --forward-to localhost:3000/webhooks` for local webhook testing.
"#;

// ─── Cloudflare ───────────────────────────────────────────────────────────────

const CLOUDFLARE_KNOWLEDGE: &str = r#"
## Provider Knowledge: Cloudflare

**API**: `https://api.cloudflare.com/client/v4/`. Auth: `Authorization: Bearer <token>` or `X-Auth-Key` + `X-Auth-Email`.
**Key gotchas**:
- Zone ID and Account ID are different. Most DNS/Cache operations use Zone ID; Workers deployments use Account ID.
- DNS propagation via API is near-instant (< 30s) but CDN cache purge is eventually consistent (up to 30s).
- Workers: max 10ms CPU per request on Free, 50ms on Paid. `fetch()` awaits don't count against CPU limit.
- KV: eventually consistent with ~60s propagation. Not suitable for counters or locking. Use Durable Objects for that.
- Cache purge by tag requires Enterprise; purge by URL/prefix is available on all plans.
- `wrangler.toml` `[env.production]` sections override the root config for named environments.
- Redirect rules use 301 (permanent) or 302 (temporary). Use 302 during testing to avoid client-side caching.
- Rate limit API: 1200 requests/5 minutes per API token. Batch DNS operations when possible.
"#;

// ─── Elastic Email ────────────────────────────────────────────────────────────

const ELASTIC_EMAIL_KNOWLEDGE: &str = r#"
## Provider Knowledge: Elastic Email

**API v4**: `https://api.elasticemail.com/v4/`. Auth: `X-ElasticEmail-ApiKey: <key>`.
**Key gotchas**:
- Two API key types: Admin (full access) vs per-project (limited scopes). Never use Admin keys in production app code.
- Sending emails requires a verified sender domain. Add SPF (`include:_spf.elasticemail.com`) and DKIM records.
- DMARC policy must be `p=none` minimum for deliverability. Add `_dmarc` TXT record before sending.
- Bounces: Elastic Email auto-suppresses hard-bounced addresses. Check suppression list before re-sending campaigns.
- `status: "complete"` on a campaign means queued, not delivered. Use `transactional` endpoint for single-send and track `MessageID`.
- Rate limits: 10,000 emails/day on Free; higher on paid plans. Bulk sends use campaign API, not transactional.
- Templates must be created via API or dashboard before referencing by name in `send` calls.
"#;

// ─── Supabase ─────────────────────────────────────────────────────────────────

const SUPABASE_KNOWLEDGE: &str = r#"
## Provider Knowledge: Supabase

**Client**: `@supabase/supabase-js`. Init: `createClient(url, anon_key)`. Never expose `service_role` key client-side.
**Key gotchas**:
- Row Level Security (RLS) is DISABLED by default. Always enable RLS on tables with user data. Without RLS, `anon` key can read all rows.
- `supabase.auth.getSession()` is async — never call synchronously. Use `onAuthStateChange` for reactive auth state.
- Realtime subscriptions persist across page navigations. Always `channel.unsubscribe()` on component unmount.
- `supabase.from('table').select('*')` returns max 1000 rows by default. Use `.range(0, 999)` or set `PGRST_DB_MAX_ROWS`.
- Storage bucket policies are separate from RLS. A bucket set to "private" still needs RLS policies for authenticated access.
- Edge Functions: Deno runtime, not Node.js. Use `Deno.env.get()` not `process.env`. Import from `https://deno.land/x/`.
- Database migrations: use `supabase db push` for local changes → `supabase db push --linked` for remote. Never edit the `supabase/migrations/` files manually after they're applied.
"#;

// ─── Neon ─────────────────────────────────────────────────────────────────────

const NEON_KNOWLEDGE: &str = r#"
## Provider Knowledge: Neon (Serverless Postgres)

**Driver**: `@neondatabase/serverless` for edge/serverless environments. Standard `pg` driver works but adds cold-start latency.
**Key gotchas**:
- Branches are copy-on-write snapshots — cheap to create, ideal for preview environments and feature testing.
- Auto-suspend: compute suspends after 5 minutes of inactivity (free tier), 300 minutes (launch+). First query after suspension takes ~500ms cold start.
- Connection pooling: Neon uses pgBouncer in transaction mode. `SET LOCAL`, advisory locks, and prepared statements don't work reliably with pooling.
- `DATABASE_URL` from Neon dashboard is a pooled connection string. Use `DIRECT_DATABASE_URL` (no `-pooler` suffix) for migrations.
- Drizzle ORM: use `neon-http` driver for edge, `neon-serverless` for Node.js serverless. Don't mix the two.
- Prisma: `directUrl` must point to unpooled connection; `url` can use pooled. Required for `prisma migrate`.
- Free tier: 0.5 GB storage, 1 branch, 1 project. Paid (Launch): 10 GB, unlimited branches.
"#;
