// SPDX-License-Identifier: MIT
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

// ─── Init Project Card ────────────────────────────────────────────────────────

/// First-run card prompting the user to run `clawd init` on their project.
///
/// Shown in onboarding flows or whenever a session's repoPath lacks a `.claw/`
/// directory.  The card calls `project.init` via JSON-RPC and reports success.
class InitProjectCard extends ConsumerStatefulWidget {
  const InitProjectCard({
    super.key,
    required this.projectPath,
    this.onInitialized,
  });

  /// Path to the project root to initialize.
  final String projectPath;

  /// Called when initialization completes successfully.
  final VoidCallback? onInitialized;

  @override
  ConsumerState<InitProjectCard> createState() => _InitProjectCardState();
}

class _InitProjectCardState extends ConsumerState<InitProjectCard> {
  bool _running = false;
  bool _done = false;
  String? _error;
  List<String> _created = [];
  String _detectedStack = '';

  Future<void> _runInit() async {
    if (_running) return;
    setState(() {
      _running = true;
      _error = null;
    });

    try {
      final client = ref.read(daemonProvider.notifier).client;
      final result = await client.call<Map<String, dynamic>>(
        'project.init',
        {'path': widget.projectPath},
      );

      final created = (result['created'] as List<dynamic>? ?? []).cast<String>();
      final stack = result['stack'] as String? ?? '';

      setState(() {
        _done = true;
        _created = created;
        _detectedStack = stack;
      });
      widget.onInitialized?.call();
    } catch (e) {
      setState(() => _error = e.toString());
    } finally {
      if (mounted) setState(() => _running = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_done) return _SuccessCard(created: _created, stack: _detectedStack);

    return Container(
      padding: const EdgeInsets.all(20),
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(
          color: ClawdTheme.claw.withValues(alpha: 0.3),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // ── Header ─────────────────────────────────────────────────────
          Row(
            children: [
              Container(
                width: 36,
                height: 36,
                decoration: BoxDecoration(
                  color: ClawdTheme.claw.withValues(alpha: 0.15),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: const Icon(
                  Icons.folder_special_outlined,
                  size: 18,
                  color: ClawdTheme.clawLight,
                ),
              ),
              const SizedBox(width: 12),
              Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Text(
                    'Initialize Project',
                    style: TextStyle(
                      fontSize: 14,
                      fontWeight: FontWeight.w700,
                      color: Colors.white,
                    ),
                  ),
                  Text(
                    'Set up ClawDE AFS structure',
                    style: TextStyle(
                      fontSize: 11,
                      color: Colors.white.withValues(alpha: 0.4),
                    ),
                  ),
                ],
              ),
            ],
          ),
          const SizedBox(height: 14),

          // ── Description ────────────────────────────────────────────────
          Text(
            'Creates the .claw/ directory with tasks, policies, and templates '
            'seeded for your stack. Auto-detects Rust, Next.js, React SPA, '
            'Flutter, or nSelf projects.',
            style: TextStyle(
              fontSize: 12,
              height: 1.5,
              color: Colors.white.withValues(alpha: 0.5),
            ),
          ),
          const SizedBox(height: 6),
          Text(
            widget.projectPath,
            style: TextStyle(
              fontSize: 11,
              fontFamily: 'monospace',
              color: Colors.white.withValues(alpha: 0.3),
            ),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),

          if (_error != null) ...[
            const SizedBox(height: 10),
            Container(
              padding: const EdgeInsets.all(10),
              decoration: BoxDecoration(
                color: Colors.red.shade900.withValues(alpha: 0.3),
                borderRadius: BorderRadius.circular(6),
              ),
              child: Text(
                _error!,
                style: const TextStyle(fontSize: 11, color: Colors.redAccent),
              ),
            ),
          ],

          const SizedBox(height: 14),

          // ── Action ─────────────────────────────────────────────────────
          SizedBox(
            width: double.infinity,
            child: FilledButton.icon(
              onPressed: _running ? null : _runInit,
              icon: _running
                  ? const SizedBox(
                      width: 14,
                      height: 14,
                      child: CircularProgressIndicator(
                        strokeWidth: 2,
                        color: Colors.white,
                      ),
                    )
                  : const Icon(Icons.rocket_launch_outlined, size: 16),
              label: Text(
                _running ? 'Initializing...' : 'Initialize Project',
                style: const TextStyle(fontSize: 13),
              ),
              style: FilledButton.styleFrom(
                backgroundColor: ClawdTheme.claw,
                padding: const EdgeInsets.symmetric(vertical: 12),
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(6),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

// ─── Success Card ─────────────────────────────────────────────────────────────

class _SuccessCard extends StatelessWidget {
  const _SuccessCard({required this.created, required this.stack});

  final List<String> created;
  final String stack;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: const Color(0xFF22c55e).withValues(alpha: 0.08),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(
          color: const Color(0xFF22c55e).withValues(alpha: 0.25),
        ),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Icon(
            Icons.check_circle_outline,
            size: 18,
            color: Color(0xFF22c55e),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Text(
                  'Project initialized',
                  style: TextStyle(
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                    color: Colors.white,
                  ),
                ),
                if (stack.isNotEmpty) ...[
                  const SizedBox(height: 2),
                  Text(
                    'Stack: $stack',
                    style: const TextStyle(
                      fontSize: 11,
                      color: Color(0xFF22c55e),
                    ),
                  ),
                ],
                if (created.isNotEmpty) ...[
                  const SizedBox(height: 6),
                  ...created.map(
                    (item) => Padding(
                      padding: const EdgeInsets.only(bottom: 2),
                      child: Text(
                        '  + $item',
                        style: const TextStyle(
                          fontSize: 11,
                          fontFamily: 'monospace',
                          color: Colors.white38,
                        ),
                      ),
                    ),
                  ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}
