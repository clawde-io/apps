import 'package:flutter/material.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Placeholder screen for the Pack Marketplace (coming soon).
class PacksScreen extends StatelessWidget {
  const PacksScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        // ── Header ───────────────────────────────────────────────────────────
        Container(
          height: 56,
          padding: const EdgeInsets.symmetric(horizontal: 20),
          decoration: const BoxDecoration(
            color: ClawdTheme.surfaceElevated,
            border: Border(
              bottom: BorderSide(color: ClawdTheme.surfaceBorder),
            ),
          ),
          child: const Row(
            children: [
              Text(
                'Packs',
                style: TextStyle(
                  fontSize: 16,
                  fontWeight: FontWeight.w700,
                  color: Colors.white,
                ),
              ),
              SizedBox(width: 8),
              _ComingSoonBadge(),
            ],
          ),
        ),

        // ── Body ─────────────────────────────────────────────────────────────
        Expanded(
          child: Center(
            child: Padding(
              padding: const EdgeInsets.all(48),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Container(
                    width: 80,
                    height: 80,
                    decoration: BoxDecoration(
                      color: ClawdTheme.claw.withValues(alpha: 0.1),
                      borderRadius: BorderRadius.circular(20),
                    ),
                    child: Icon(
                      Icons.extension,
                      size: 40,
                      color: ClawdTheme.clawLight.withValues(alpha: 0.6),
                    ),
                  ),
                  const SizedBox(height: 24),
                  const Text(
                    'Pack Marketplace',
                    style: TextStyle(
                      fontSize: 22,
                      fontWeight: FontWeight.w700,
                      color: Colors.white,
                    ),
                  ),
                  const SizedBox(height: 8),
                  const Text(
                    'Coming Soon',
                    style: TextStyle(
                      fontSize: 16,
                      fontWeight: FontWeight.w600,
                      color: ClawdTheme.clawLight,
                    ),
                  ),
                  const SizedBox(height: 20),
                  SizedBox(
                    width: 400,
                    child: Text(
                      'Packs are curated bundles of prompts, tools, and configurations '
                      'that extend ClawDE for specific workflows. Install community '
                      'packs or create your own to share with your team.',
                      style: TextStyle(
                        fontSize: 13,
                        height: 1.6,
                        color: Colors.white.withValues(alpha: 0.5),
                      ),
                      textAlign: TextAlign.center,
                    ),
                  ),
                  const SizedBox(height: 32),

                  // ── Feature preview cards ──────────────────────────────────
                  const SizedBox(
                    width: 480,
                    child: Row(
                      children: [
                        Expanded(
                          child: _PreviewCard(
                            icon: Icons.download_outlined,
                            title: 'Install',
                            description:
                                'Browse and install packs from the marketplace',
                          ),
                        ),
                        SizedBox(width: 12),
                        Expanded(
                          child: _PreviewCard(
                            icon: Icons.build_outlined,
                            title: 'Create',
                            description:
                                'Build your own packs with custom prompts and tools',
                          ),
                        ),
                        SizedBox(width: 12),
                        Expanded(
                          child: _PreviewCard(
                            icon: Icons.share_outlined,
                            title: 'Share',
                            description:
                                'Publish packs for the community or your team',
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ],
    );
  }
}

// ── Coming Soon badge ─────────────────────────────────────────────────────────

class _ComingSoonBadge extends StatelessWidget {
  const _ComingSoonBadge();

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
      decoration: BoxDecoration(
        color: ClawdTheme.warning.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(
          color: ClawdTheme.warning.withValues(alpha: 0.3),
        ),
      ),
      child: const Text(
        'Coming Soon',
        style: TextStyle(
          fontSize: 10,
          fontWeight: FontWeight.w600,
          color: ClawdTheme.warning,
        ),
      ),
    );
  }
}

// ── Preview card ──────────────────────────────────────────────────────────────

class _PreviewCard extends StatelessWidget {
  const _PreviewCard({
    required this.icon,
    required this.title,
    required this.description,
  });

  final IconData icon;
  final String title;
  final String description;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: Column(
        children: [
          Icon(icon, size: 24, color: ClawdTheme.clawLight),
          const SizedBox(height: 10),
          Text(
            title,
            style: const TextStyle(
              fontSize: 13,
              fontWeight: FontWeight.w600,
              color: Colors.white,
            ),
          ),
          const SizedBox(height: 6),
          Text(
            description,
            style: TextStyle(
              fontSize: 11,
              color: Colors.white.withValues(alpha: 0.4),
              height: 1.4,
            ),
            textAlign: TextAlign.center,
          ),
        ],
      ),
    );
  }
}
