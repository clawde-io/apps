pub mod bundles;
/// Provider knowledge library — V02.T33-T35.
///
/// Detects which cloud providers a project uses by scanning `.env`, config files,
/// and source code for provider footprints. Returns embedded knowledge bundles
/// for each detected provider, appended to the system prompt on session.create.
pub mod detection;

pub use bundles::bundle_for_provider;
pub use detection::detect_providers;

/// A detected cloud/service provider.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Provider {
    Hetzner,
    Vercel,
    Stripe,
    Cloudflare,
    ElasticEmail,
    Supabase,
    Neon,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::Hetzner => "hetzner",
            Provider::Vercel => "vercel",
            Provider::Stripe => "stripe",
            Provider::Cloudflare => "cloudflare",
            Provider::ElasticEmail => "elastic_email",
            Provider::Supabase => "supabase",
            Provider::Neon => "neon",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Provider::Hetzner => "Hetzner",
            Provider::Vercel => "Vercel",
            Provider::Stripe => "Stripe",
            Provider::Cloudflare => "Cloudflare",
            Provider::ElasticEmail => "Elastic Email",
            Provider::Supabase => "Supabase",
            Provider::Neon => "Neon",
        }
    }
}
