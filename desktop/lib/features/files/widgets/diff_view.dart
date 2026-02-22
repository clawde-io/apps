import 'package:flutter/material.dart';

/// Renders a unified diff string with colored add/remove/context lines.
class DiffView extends StatelessWidget {
  const DiffView({super.key, required this.diff});
  final String diff;

  @override
  Widget build(BuildContext context) {
    final lines = diff.split('\n');
    return SingleChildScrollView(
      child: SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: lines.map(_buildLine).toList(),
        ),
      ),
    );
  }

  Widget _buildLine(String line) {
    final Color bg;
    final Color fg;

    if (line.startsWith('+++') || line.startsWith('---')) {
      bg = Colors.blue.withValues(alpha: 0.08);
      fg = Colors.blue.shade300;
    } else if (line.startsWith('+')) {
      bg = Colors.green.withValues(alpha: 0.12);
      fg = Colors.green.shade300;
    } else if (line.startsWith('-')) {
      bg = Colors.red.withValues(alpha: 0.12);
      fg = Colors.red.shade300;
    } else if (line.startsWith('@@')) {
      bg = Colors.purple.withValues(alpha: 0.12);
      fg = Colors.purple.shade300;
    } else {
      bg = Colors.transparent;
      fg = Colors.white54;
    }

    return Container(
      color: bg,
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 1),
      child: Text(
        line.isEmpty ? ' ' : line,
        style: TextStyle(
          fontSize: 12,
          fontFamily: 'monospace',
          color: fg,
          height: 1.6,
        ),
      ),
    );
  }
}
