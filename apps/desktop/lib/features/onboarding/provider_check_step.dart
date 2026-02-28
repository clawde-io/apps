/// Step 1 of the onboarding wizard — detected provider status cards.
///
/// Displays claude, codex, and cursor as cards with status chips showing
/// whether each provider is installed and authenticated (PO.T01, PO.T06).
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_ui/clawd_ui.dart';

import 'package:clawde/features/onboarding/providers_onboarding_providers.dart';

/// Provider detection step — full-width widget used in [OnboardingScreen].
class ProviderCheckStep extends ConsumerWidget {
  const ProviderCheckStep({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final statusAsync = ref.watch(providerStatusProvider);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Text(
          'Detected AI Providers',
          style: TextStyle(
            fontSize: 20,
            fontWeight: FontWeight.w700,
            color: Colors.white,
          ),
        ),
        const SizedBox(height: 8),
        const Text(
          'ClawDE detected the following AI provider CLIs on your machine. '
          'You can add more accounts in Settings → Accounts after setup.',
          style: TextStyle(fontSize: 13, color: Colors.white54),
        ),
        const SizedBox(height: 24),
        statusAsync.when(
          loading: () => const _LoadingCards(),
          error: (e, _) => _ErrorCard(error: e.toString()),
          data: (providers) => Column(
            children: [
              _ProviderCard(
                name: 'claude',
                displayName: 'Claude Code',
                icon: Icons.smart_toy_outlined,
                status: providers['claude'],
                description:
                    'Anthropic\'s Claude Code CLI — primary for code generation '
                    'and architecture.',
              ),
              const SizedBox(height: 12),
              _ProviderCard(
                name: 'codex',
                displayName: 'OpenAI Codex',
                icon: Icons.code_outlined,
                status: providers['codex'],
                description:
                    'OpenAI\'s Codex CLI — preferred for debugging and code review.',
              ),
              const SizedBox(height: 12),
              _ProviderCard(
                name: 'cursor',
                displayName: 'Cursor',
                icon: Icons.computer_outlined,
                status: providers['cursor'],
                description:
                    'Cursor AI IDE integration — for Cursor-managed sessions.',
              ),
            ],
          ),
        ),
        const SizedBox(height: 24),
        const _InstallHint(),
      ],
    );
  }
}

// ─── Provider card ────────────────────────────────────────────────────────────

class _ProviderCard extends StatelessWidget {
  const _ProviderCard({
    required this.name,
    required this.displayName,
    required this.icon,
    required this.description,
    this.status,
  });

  final String name;
  final String displayName;
  final IconData icon;
  final String description;
  final ProviderStatus? status;

  @override
  Widget build(BuildContext context) {
    final installed = status?.installed ?? false;
    final authenticated = status?.authenticated ?? false;
    final version = status?.version;
    final accounts = status?.accountsCount ?? 0;

    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(
          color: installed
              ? ClawdTheme.claw.withValues(alpha: 0.4)
              : ClawdTheme.surfaceBorder,
        ),
      ),
      child: Row(
        children: [
          // Icon
          Container(
            width: 40,
            height: 40,
            decoration: BoxDecoration(
              color: installed
                  ? ClawdTheme.claw.withValues(alpha: 0.15)
                  : Colors.white.withValues(alpha: 0.05),
              borderRadius: BorderRadius.circular(8),
            ),
            child: Icon(
              icon,
              size: 20,
              color: installed ? ClawdTheme.claw : Colors.white30,
            ),
          ),
          const SizedBox(width: 16),

          // Details
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Text(
                      displayName,
                      style: const TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.w600,
                        color: Colors.white,
                      ),
                    ),
                    const SizedBox(width: 8),
                    if (version != null)
                      Text(
                        'v$version',
                        style: const TextStyle(
                          fontSize: 11,
                          color: Colors.white38,
                        ),
                      ),
                  ],
                ),
                const SizedBox(height: 2),
                Text(
                  description,
                  style: const TextStyle(
                    fontSize: 12,
                    color: Colors.white54,
                  ),
                ),
                if (installed && accounts > 0) ...[
                  const SizedBox(height: 4),
                  Text(
                    '$accounts account${accounts == 1 ? '' : 's'} configured',
                    style: const TextStyle(
                      fontSize: 11,
                      color: Colors.white38,
                    ),
                  ),
                ],
              ],
            ),
          ),

          // Status chips
          Column(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              _StatusChip(
                label: installed ? 'Installed' : 'Not found',
                color: installed ? Colors.green : Colors.white30,
              ),
              if (installed) ...[
                const SizedBox(height: 4),
                _StatusChip(
                  label: authenticated ? 'Authenticated' : 'Needs auth',
                  color: authenticated ? Colors.green : Colors.orange,
                ),
              ],
            ],
          ),
        ],
      ),
    );
  }
}

class _StatusChip extends StatelessWidget {
  const _StatusChip({required this.label, required this.color});

  final String label;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(4),
        border: Border.all(color: color.withValues(alpha: 0.4)),
      ),
      child: Text(
        label,
        style: TextStyle(
          fontSize: 11,
          color: color,
          fontWeight: FontWeight.w500,
        ),
      ),
    );
  }
}

// ─── Loading / Error ──────────────────────────────────────────────────────────

class _LoadingCards extends StatelessWidget {
  const _LoadingCards();

  @override
  Widget build(BuildContext context) {
    return Column(
      children: List.generate(
        3,
        (_) => Container(
          margin: const EdgeInsets.only(bottom: 12),
          height: 80,
          decoration: BoxDecoration(
            color: ClawdTheme.surfaceElevated,
            borderRadius: BorderRadius.circular(8),
            border: Border.all(color: ClawdTheme.surfaceBorder),
          ),
          child: const Center(
            child: SizedBox(
              width: 20,
              height: 20,
              child: CircularProgressIndicator(strokeWidth: 2),
            ),
          ),
        ),
      ),
    );
  }
}

class _ErrorCard extends StatelessWidget {
  const _ErrorCard({required this.error});
  final String error;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: Colors.red.withValues(alpha: 0.1),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: Colors.red.withValues(alpha: 0.3)),
      ),
      child: Text(
        'Could not detect providers: $error',
        style: const TextStyle(fontSize: 13, color: Colors.redAccent),
      ),
    );
  }
}

class _InstallHint extends StatelessWidget {
  const _InstallHint();

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: Colors.white.withValues(alpha: 0.03),
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: const Row(
        children: [
          Icon(Icons.info_outline, size: 14, color: Colors.white38),
          SizedBox(width: 8),
          Expanded(
            child: Text(
              'Don\'t see a provider? Install its CLI and restart ClawDE. '
              'ClawDE works with one provider — you don\'t need all three.',
              style: TextStyle(fontSize: 12, color: Colors.white38),
            ),
          ),
        ],
      ),
    );
  }
}
