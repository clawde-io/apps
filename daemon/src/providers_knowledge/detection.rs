/// Provider detection — scan `.env` + config files for provider footprints.
///
/// V02.T34: On session.create, scan repo for provider-specific env var prefixes,
/// config file names, and package names.
use super::Provider;
use std::collections::HashSet;
use std::path::Path;

/// Detect which cloud providers are used in a project.
/// Scans: .env, .env.*, vercel.json, wrangler.toml, package.json, Cargo.toml,
///        pubspec.yaml, and source files for recognizable provider keywords.
pub fn detect_providers(repo_path: &Path) -> Vec<Provider> {
    let mut found: HashSet<Provider> = HashSet::new();

    // ── 1. Scan .env files ─────────────────────────────────────────────────
    for env_file in find_env_files(repo_path) {
        if let Ok(content) = std::fs::read_to_string(&env_file) {
            check_env_content(&content, &mut found);
        }
    }

    // ── 2. Scan config files ───────────────────────────────────────────────
    let config_files = [
        "vercel.json",
        ".vercelrc",
        "wrangler.toml",
        "wrangler.jsonc",
        "package.json",
        "Cargo.toml",
        "pubspec.yaml",
        "pyproject.toml",
        ".stripe-cli.json",
    ];
    for name in &config_files {
        let path = repo_path.join(name);
        if let Ok(content) = std::fs::read_to_string(&path) {
            check_config_content(&content, name, &mut found);
        }
    }

    // ── 3. Scan .claude/vault.env if present ──────────────────────────────
    let vault_path = repo_path.join(".claude/vault.env");
    if let Ok(content) = std::fs::read_to_string(&vault_path) {
        check_env_content(&content, &mut found);
    }

    // Convert to sorted Vec for deterministic output
    let mut result: Vec<Provider> = found.into_iter().collect();
    result.sort_by_key(|p| p.as_str());
    result
}

/// Find .env files in the project root and common subdirectories.
fn find_env_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let candidates = [
        ".env",
        ".env.local",
        ".env.development",
        ".env.production",
        ".env.staging",
        ".env.example",
        "backend/.env",
        "app/.env",
        "site/.env",
    ];
    for name in &candidates {
        let p = root.join(name);
        if p.exists() {
            files.push(p);
        }
    }
    files
}

/// Check env file content for provider keywords.
fn check_env_content(content: &str, found: &mut HashSet<Provider>) {
    let upper = content.to_uppercase();

    if upper.contains("HETZNER") {
        found.insert(Provider::Hetzner);
    }
    if upper.contains("VERCEL") {
        found.insert(Provider::Vercel);
    }
    if upper.contains("STRIPE") {
        found.insert(Provider::Stripe);
    }
    if upper.contains("CLOUDFLARE") || upper.contains("CF_") || upper.contains("CF_ZONE") {
        found.insert(Provider::Cloudflare);
    }
    if upper.contains("ELASTIC_EMAIL") || upper.contains("ELASTICEMAIL") {
        found.insert(Provider::ElasticEmail);
    }
    if upper.contains("SUPABASE") {
        found.insert(Provider::Supabase);
    }
    if upper.contains("NEON") || upper.contains("NEONDB") {
        found.insert(Provider::Neon);
    }
}

/// Check config file content for provider-specific patterns.
fn check_config_content(content: &str, filename: &str, found: &mut HashSet<Provider>) {
    let lower_name = filename.to_lowercase();
    let lower_content = content.to_lowercase();

    // vercel.json always means Vercel
    if lower_name.contains("vercel") {
        found.insert(Provider::Vercel);
        return;
    }
    // wrangler.toml → Cloudflare Workers
    if lower_name.contains("wrangler") {
        found.insert(Provider::Cloudflare);
        return;
    }
    // .stripe-cli.json → Stripe
    if lower_name.contains("stripe") {
        found.insert(Provider::Stripe);
        return;
    }

    // Content-based checks for generic files
    check_env_content(content, found);

    // Package dependencies
    if lower_content.contains("\"stripe\"") || lower_content.contains("stripe = ") {
        found.insert(Provider::Stripe);
    }
    if lower_content.contains("\"@vercel/") || lower_content.contains("vercel") {
        found.insert(Provider::Vercel);
    }
    if lower_content.contains("cloudflare") || lower_content.contains("workers-types") {
        found.insert(Provider::Cloudflare);
    }
    if lower_content.contains("supabase") {
        found.insert(Provider::Supabase);
    }
    if lower_content.contains("neondb") || lower_content.contains("@neondatabase") {
        found.insert(Provider::Neon);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_stripe_from_env() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join(".env"), b"STRIPE_SECRET_KEY=sk_test_xxx\n").unwrap();
        let providers = detect_providers(tmp.path());
        assert!(providers.contains(&Provider::Stripe));
    }

    #[test]
    fn test_detect_vercel_from_config() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("vercel.json"), b"{}").unwrap();
        let providers = detect_providers(tmp.path());
        assert!(providers.contains(&Provider::Vercel));
    }

    #[test]
    fn test_detect_empty_project() {
        let tmp = TempDir::new().unwrap();
        let providers = detect_providers(tmp.path());
        assert!(providers.is_empty());
    }
}
