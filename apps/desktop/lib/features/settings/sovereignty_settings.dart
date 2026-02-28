import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';

// ─── Provider ─────────────────────────────────────────────────────────────────

final sovereigntyReportProvider =
    FutureProvider.autoDispose<Map<String, dynamic>>(
  (ref) async {
    final client = ref.read(daemonProvider.notifier).client;
    return client.call<Map<String, dynamic>>('sovereignty.report', {});
  },
);

// ─── Widget ───────────────────────────────────────────────────────────────────

/// Sprint DD TS.6 — "AI Tools Active on This Codebase" settings section.
///
/// Shows which other AI tools (Copilot, Cursor, Continue…) have been detected
/// writing files to the project in the last 7 days.
class SovereigntySettings extends ConsumerWidget {
  const SovereigntySettings({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final reportAsync = ref.watch(sovereigntyReportProvider);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'AI Tools Active on This Codebase',
          style: Theme.of(context)
              .textTheme
              .titleSmall
              ?.copyWith(color: Colors.white),
        ),
        const SizedBox(height: 4),
        const Text(
          'Other AI tools detected writing to this project in the last 7 days.',
          style: TextStyle(fontSize: 11, color: Colors.white38),
        ),
        const SizedBox(height: 16),
        reportAsync.when(
          loading: () => const LinearProgressIndicator(),
          error: (e, _) => Text('Failed to load: $e',
              style: const TextStyle(color: Colors.redAccent, fontSize: 12)),
          data: (report) {
            final tools = (report['tools'] as List?)
                    ?.cast<Map<String, dynamic>>() ??
                [];
            if (tools.isEmpty) {
              return const Text(
                'No other AI tools detected. This codebase is ClawDE-exclusive.',
                style: TextStyle(
                    fontSize: 12,
                    color: Colors.white54,
                    fontStyle: FontStyle.italic),
              );
            }
            return Column(
              children: tools
                  .map((tool) => _ToolRow(tool: tool))
                  .toList(),
            );
          },
        ),
      ],
    );
  }
}

class _ToolRow extends StatelessWidget {
  const _ToolRow({required this.tool});

  final Map<String, dynamic> tool;

  @override
  Widget build(BuildContext context) {
    final toolId = tool['toolId'] as String? ?? '';
    final eventCount = tool['eventCount'] as int? ?? 0;
    final lastSeen = tool['lastSeen'] as String? ?? '';
    final files =
        (tool['filesTouched'] as List?)?.cast<String>() ?? [];

    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Container(
        padding: const EdgeInsets.all(12),
        decoration: BoxDecoration(
          color: ClawdTheme.surfaceElevated,
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: ClawdTheme.surfaceBorder),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Icon(Icons.extension, size: 18, color: Colors.amber),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    toolId,
                    style: const TextStyle(
                        fontSize: 13,
                        fontWeight: FontWeight.w600,
                        color: Colors.amber),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    '$eventCount event${eventCount == 1 ? '' : 's'} · ${files.length} file${files.length == 1 ? '' : 's'} · last seen $lastSeen',
                    style: const TextStyle(
                        fontSize: 11, color: Colors.white54),
                  ),
                  if (files.isNotEmpty) ...[
                    const SizedBox(height: 4),
                    Text(
                      files.take(3).join(', ') +
                          (files.length > 3
                              ? ' +${files.length - 3} more'
                              : ''),
                      style: const TextStyle(
                          fontSize: 10,
                          color: Colors.white38,
                          fontFamily: 'monospace'),
                    ),
                  ],
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
