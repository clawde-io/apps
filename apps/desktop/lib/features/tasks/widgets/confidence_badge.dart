import 'package:flutter/material.dart';

/// Color-coded confidence badge shown on task cards.
/// Green >= 0.8, Amber >= 0.5, Red < 0.5.
class ConfidenceBadge extends StatelessWidget {
  const ConfidenceBadge({
    super.key,
    required this.score,
    this.reasoning,
    this.compact = false,
  });

  final double score;
  final String? reasoning;
  final bool compact;

  Color _color(BuildContext context) {
    if (score >= 0.8) return Colors.green;
    if (score >= 0.5) return Colors.orange;
    return Colors.red;
  }

  String get _label {
    final pct = (score * 100).round();
    return compact ? '$pct%' : 'Confidence: $pct%';
  }

  @override
  Widget build(BuildContext context) {
    final color = _color(context);
    final badge = Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: color.withValues(alpha: 0.4)),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.psychology_outlined, size: 12, color: color),
          const SizedBox(width: 4),
          Text(
            _label,
            style: Theme.of(context)
                .textTheme
                .labelSmall
                ?.copyWith(color: color, fontWeight: FontWeight.w600),
          ),
        ],
      ),
    );

    if (reasoning == null || reasoning!.isEmpty) return badge;

    return Tooltip(
      message: reasoning!,
      child: badge,
    );
  }
}
