import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';

// ─── Provider ─────────────────────────────────────────────────────────────────

final _pulseProvider = FutureProvider.autoDispose<Map<String, dynamic>>(
  (ref) async {
    final client = ref.read(daemonProvider.notifier).client;
    return client.call<Map<String, dynamic>>('project.pulse', {'days': 7});
  },
);

// ─── Screen ──────────────────────────────────────────────────────────────────

/// Sprint DD PP.7 — Project Pulse screen.
///
/// Shows semantic change velocity charts and recent event feed.
class ProjectPulseScreen extends ConsumerWidget {
  const ProjectPulseScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final pulseAsync = ref.watch(_pulseProvider);

    return Padding(
      padding: const EdgeInsets.all(24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            'Project Pulse',
            style: TextStyle(
                fontSize: 20,
                fontWeight: FontWeight.w700,
                color: Colors.white),
          ),
          const SizedBox(height: 4),
          const Text(
            'Semantic change velocity for the last 7 days.',
            style: TextStyle(fontSize: 13, color: Colors.white54),
          ),
          const SizedBox(height: 24),
          Expanded(
            child: pulseAsync.when(
              loading: () => const Center(
                child: CircularProgressIndicator(),
              ),
              error: (e, _) => Center(
                child: Text('Failed to load pulse: $e',
                    style: const TextStyle(color: Colors.redAccent)),
              ),
              data: (pulse) => _PulseContent(pulse: pulse),
            ),
          ),
        ],
      ),
    );
  }
}

class _PulseContent extends StatelessWidget {
  const _PulseContent({required this.pulse});

  final Map<String, dynamic> pulse;

  @override
  Widget build(BuildContext context) {
    final velocity =
        pulse['velocity'] as Map<String, dynamic>? ?? {};
    final events = (pulse['events'] as List?)
            ?.cast<Map<String, dynamic>>() ??
        [];

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // ── Velocity cards ────────────────────────────────────────────────
        _VelocityGrid(velocity: velocity),
        const SizedBox(height: 24),
        const Divider(),
        const SizedBox(height: 16),

        // ── Recent events ─────────────────────────────────────────────────
        Row(
          children: [
            const Icon(Icons.timeline, size: 16, color: ClawdTheme.clawLight),
            const SizedBox(width: 6),
            Text(
              'Recent Changes',
              style: Theme.of(context)
                  .textTheme
                  .titleSmall
                  ?.copyWith(color: Colors.white),
            ),
          ],
        ),
        const SizedBox(height: 12),
        Expanded(
          child: events.isEmpty
              ? const Center(
                  child: Text(
                    'No semantic events recorded yet.\nMake some commits to populate the pulse.',
                    textAlign: TextAlign.center,
                    style: TextStyle(color: Colors.white38),
                  ),
                )
              : ListView.separated(
                  itemCount: events.length,
                  separatorBuilder: (_, __) =>
                      const Divider(height: 1),
                  itemBuilder: (context, i) {
                    final event = events[i];
                    return _EventRow(event: event);
                  },
                ),
        ),
      ],
    );
  }
}

class _VelocityGrid extends StatelessWidget {
  const _VelocityGrid({required this.velocity});

  final Map<String, dynamic> velocity;

  @override
  Widget build(BuildContext context) {
    final items = [
      (
        label: 'Features',
        count: velocity['features'] as int? ?? 0,
        icon: Icons.add_circle_outline,
        color: Colors.green,
      ),
      (
        label: 'Bug Fixes',
        count: velocity['bugs'] as int? ?? 0,
        icon: Icons.bug_report_outlined,
        color: Colors.orange,
      ),
      (
        label: 'Refactors',
        count: velocity['refactors'] as int? ?? 0,
        icon: Icons.sync_alt,
        color: Colors.blue,
      ),
      (
        label: 'Tests',
        count: velocity['tests'] as int? ?? 0,
        icon: Icons.check_circle_outline,
        color: Colors.teal,
      ),
    ];

    return Row(
      children: items
          .map(
            (item) => Expanded(
              child: Padding(
                padding: const EdgeInsets.only(right: 8),
                child: _VelocityCard(
                  label: item.label,
                  count: item.count,
                  icon: item.icon,
                  color: item.color,
                ),
              ),
            ),
          )
          .toList(),
    );
  }
}

class _VelocityCard extends StatelessWidget {
  const _VelocityCard({
    required this.label,
    required this.count,
    required this.icon,
    required this.color,
  });

  final String label;
  final int count;
  final IconData icon;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.1),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: color.withValues(alpha: 0.3)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon, size: 20, color: color),
          const SizedBox(height: 8),
          Text(
            '$count',
            style: TextStyle(
                fontSize: 28,
                fontWeight: FontWeight.w700,
                color: color),
          ),
          Text(
            label,
            style: const TextStyle(
                fontSize: 12, color: Colors.white54),
          ),
        ],
      ),
    );
  }
}

class _EventRow extends StatelessWidget {
  const _EventRow({required this.event});

  final Map<String, dynamic> event;

  @override
  Widget build(BuildContext context) {
    final eventType = event['eventType'] as String? ?? '';
    final summaryText = event['summaryText'] as String? ?? '';
    final createdAt = event['createdAt'] as String? ?? '';
    final files =
        (event['affectedFiles'] as List?)?.cast<String>() ?? [];

    final (color, icon) = _typeStyle(eventType);

    return ListTile(
      dense: true,
      leading: Icon(icon, size: 18, color: color),
      title: Text(
        summaryText.isNotEmpty
            ? summaryText
            : eventType.replaceAll('_', ' '),
        style: const TextStyle(fontSize: 13),
      ),
      subtitle: Text(
        '${files.take(2).join(', ')}${files.length > 2 ? ' +${files.length - 2} more' : ''} · $createdAt',
        style: const TextStyle(fontSize: 11, color: Colors.white38),
      ),
    );
  }

  (Color, IconData) _typeStyle(String type) => switch (type) {
        'feature_added' => (Colors.green, Icons.add_circle_outline),
        'bug_fixed' => (Colors.orange, Icons.bug_report_outlined),
        'refactored' => (Colors.blue, Icons.sync_alt),
        'test_added' => (Colors.teal, Icons.check_circle_outline),
        'config_changed' => (Colors.purple, Icons.settings_outlined),
        'dependency_updated' => (Colors.amber, Icons.archive_outlined),
        _ => (Colors.white54, Icons.circle_outlined),
      };
}
