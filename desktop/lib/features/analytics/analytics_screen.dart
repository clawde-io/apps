// SPDX-License-Identifier: MIT
import 'dart:math' as math;

import 'package:clawd_ui/clawd_ui.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:clawde/features/analytics/analytics_providers.dart';

// ─── Analytics Screen ─────────────────────────────────────────────────────────

/// Settings → Analytics (AN.T04).
///
/// Displays:
/// - Summary cards row: lines written, AI assist %, total sessions, total cost.
/// - Custom BarChart: daily session counts for the last 30 days.
/// - Custom PieChart: provider/model breakdown.
///
/// Charts are drawn with [CustomPainter] — no third-party chart library needed.
class AnalyticsScreen extends ConsumerWidget {
  const AnalyticsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final analyticsAsync = ref.watch(personalAnalyticsProvider);
    final breakdownAsync = ref.watch(providerBreakdownProvider);

    return Semantics(
      label: 'Analytics dashboard',
      child: SingleChildScrollView(
        padding: const EdgeInsets.all(32),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const _Header(
              title: 'Analytics',
              subtitle: 'Personal usage metrics and provider breakdown',
            ),
            const SizedBox(height: 24),

            // ── Summary cards ──────────────────────────────────────────────
            analyticsAsync.when(
              loading: () => const _SummaryCardsLoading(),
              error: (e, _) => _ErrorCard('Could not load analytics: $e'),
              data: (analytics) {
                final totalSessions = analytics.sessionsPerDay
                    .fold(0, (sum, d) => sum + d.count);
                final totalCost = breakdownAsync.valueOrNull
                        ?.fold(0.0, (sum, b) => sum + b.costUsd) ??
                    0.0;
                return _SummaryCards(
                  linesWritten: analytics.linesWritten,
                  aiAssistPercent: analytics.aiAssistPercent,
                  totalSessions: totalSessions,
                  totalCostUsd: totalCost,
                );
              },
            ),

            const SizedBox(height: 32),

            // ── Daily sessions chart ───────────────────────────────────────
            const _SectionLabel('Sessions per day — last 30 days'),
            const SizedBox(height: 12),
            analyticsAsync.when(
              loading: () => const _ChartPlaceholder(),
              error: (_, __) => const _ChartPlaceholder(),
              data: (analytics) =>
                  _DailySessionsChart(days: analytics.sessionsPerDay),
            ),

            const SizedBox(height: 32),

            // ── Provider breakdown pie chart ───────────────────────────────
            const _SectionLabel('Provider breakdown'),
            const SizedBox(height: 12),
            breakdownAsync.when(
              loading: () => const _ChartPlaceholder(),
              error: (_, __) => const _ChartPlaceholder(),
              data: (breakdown) => _ProviderPieChart(breakdown: breakdown),
            ),
          ],
        ),
      ),
    );
  }
}

// ─── Summary Cards ────────────────────────────────────────────────────────────

class _SummaryCards extends StatelessWidget {
  const _SummaryCards({
    required this.linesWritten,
    required this.aiAssistPercent,
    required this.totalSessions,
    required this.totalCostUsd,
  });

  final int linesWritten;
  final double aiAssistPercent;
  final int totalSessions;
  final double totalCostUsd;

  String _fmt(int n) {
    if (n >= 1000000) return '${(n / 1000000).toStringAsFixed(1)}M';
    if (n >= 1000) return '${(n / 1000).toStringAsFixed(1)}K';
    return '$n';
  }

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Expanded(
          child: _SummaryCard(
            icon: Icons.code,
            label: 'Lines Written',
            value: _fmt(linesWritten),
            semanticLabel: '$linesWritten lines written',
          ),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: _SummaryCard(
            icon: Icons.auto_awesome,
            label: 'AI Assist',
            value: '${aiAssistPercent.toStringAsFixed(1)}%',
            semanticLabel:
                '${aiAssistPercent.toStringAsFixed(1)} percent AI assisted',
          ),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: _SummaryCard(
            icon: Icons.chat_bubble_outline,
            label: 'Sessions',
            value: '$totalSessions',
            semanticLabel: '$totalSessions total sessions',
          ),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: _SummaryCard(
            icon: Icons.attach_money,
            label: 'Total Cost',
            value: '\$${totalCostUsd.toStringAsFixed(2)}',
            semanticLabel:
                '\$${totalCostUsd.toStringAsFixed(2)} total estimated cost',
          ),
        ),
      ],
    );
  }
}

class _SummaryCardsLoading extends StatelessWidget {
  const _SummaryCardsLoading();

  @override
  Widget build(BuildContext context) {
    return Row(
      children: List.generate(
        4,
        (_) => Expanded(
          child: Padding(
            padding: const EdgeInsets.only(right: 12),
            child: Container(
              height: 80,
              decoration: BoxDecoration(
                color: ClawdTheme.surfaceElevated,
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: ClawdTheme.surfaceBorder),
              ),
              child: const Center(
                  child: CircularProgressIndicator(strokeWidth: 2)),
            ),
          ),
        ),
      ),
    );
  }
}

class _SummaryCard extends StatelessWidget {
  const _SummaryCard({
    required this.icon,
    required this.label,
    required this.value,
    required this.semanticLabel,
  });

  final IconData icon;
  final String label;
  final String value;
  final String semanticLabel;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      label: semanticLabel,
      child: Container(
        padding: const EdgeInsets.all(16),
        decoration: BoxDecoration(
          color: ClawdTheme.surfaceElevated,
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: ClawdTheme.surfaceBorder),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(icon, size: 14, color: ClawdTheme.claw),
                const SizedBox(width: 6),
                Text(
                  label,
                  style:
                      const TextStyle(fontSize: 11, color: Colors.white38),
                ),
              ],
            ),
            const SizedBox(height: 8),
            Text(
              value,
              style: const TextStyle(
                fontSize: 20,
                fontWeight: FontWeight.w700,
                color: Colors.white,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

// ─── Daily Sessions BarChart (CustomPainter) ──────────────────────────────────

class _DailySessionsChart extends StatelessWidget {
  const _DailySessionsChart({required this.days});

  final List<DailyCount> days;

  @override
  Widget build(BuildContext context) {
    if (days.isEmpty) return const _ChartPlaceholder();

    return Semantics(
      label: 'Bar chart: session counts per day for the last 30 days',
      child: Container(
        height: 180,
        padding: const EdgeInsets.fromLTRB(8, 12, 8, 8),
        decoration: BoxDecoration(
          color: ClawdTheme.surfaceElevated,
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: ClawdTheme.surfaceBorder),
        ),
        child: CustomPaint(
          painter: _BarChartPainter(days: days),
          child: const SizedBox.expand(),
        ),
      ),
    );
  }
}

class _BarChartPainter extends CustomPainter {
  _BarChartPainter({required this.days});

  final List<DailyCount> days;

  @override
  void paint(Canvas canvas, Size size) {
    if (days.isEmpty) return;

    final maxCount =
        days.map((d) => d.count).fold(1, (a, b) => a > b ? a : b);
    final barPaint = Paint()
      ..color = ClawdTheme.claw
      ..style = PaintingStyle.fill;
    final gridPaint = Paint()
      ..color = ClawdTheme.surfaceBorder
      ..strokeWidth = 1;

    const bottomPad = 20.0;
    const topPad = 4.0;
    final chartH = size.height - bottomPad - topPad;
    final barW = (size.width / days.length) * 0.6;
    final gap = size.width / days.length;

    // Horizontal grid lines (4 lines).
    for (int i = 1; i <= 4; i++) {
      final y = topPad + chartH * (1 - i / 4);
      canvas.drawLine(Offset(0, y), Offset(size.width, y), gridPaint);
    }

    for (int i = 0; i < days.length; i++) {
      final barH = chartH * (days[i].count / maxCount);
      final x = gap * i + (gap - barW) / 2;
      final y = topPad + chartH - barH;
      final rect = RRect.fromRectAndCorners(
        Rect.fromLTWH(x, y, barW, barH),
        topLeft: const Radius.circular(2),
        topRight: const Radius.circular(2),
      );
      canvas.drawRRect(rect, barPaint);
    }

    // Date labels for first, middle, last.
    final labelPainter = TextPainter(textDirection: TextDirection.ltr);
    for (final idx in [0, days.length ~/ 2, days.length - 1]) {
      if (idx >= days.length) continue;
      final parts = days[idx].date.split('-');
      final label = parts.length >= 3 ? '${parts[1]}/${parts[2]}' : days[idx].date;
      labelPainter.text = TextSpan(
        text: label,
        style: const TextStyle(fontSize: 9, color: Colors.white38),
      );
      labelPainter.layout();
      final x = gap * idx + gap / 2 - labelPainter.width / 2;
      labelPainter.paint(canvas, Offset(x, size.height - bottomPad + 4));
    }
  }

  @override
  bool shouldRepaint(_BarChartPainter old) => old.days != days;
}

// ─── Provider PieChart (CustomPainter) ───────────────────────────────────────

class _ProviderPieChart extends StatelessWidget {
  const _ProviderPieChart({required this.breakdown});

  final List<ProviderBreakdown> breakdown;

  static const _providerColors = {
    'claude': ClawdTheme.claudeColor,
    'codex': ClawdTheme.codexColor,
    'cursor': ClawdTheme.cursorColor,
  };

  static const _fallbackColors = [
    Color(0xFF6366f1),
    Color(0xFF22d3ee),
    Color(0xFFa3e635),
  ];

  @override
  Widget build(BuildContext context) {
    if (breakdown.isEmpty) return const _ChartPlaceholder();

    final total =
        breakdown.fold(0, (sum, b) => sum + b.sessions);

    return Semantics(
      label: 'Pie chart showing provider session breakdown',
      child: Container(
        padding: const EdgeInsets.all(16),
        decoration: BoxDecoration(
          color: ClawdTheme.surfaceElevated,
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: ClawdTheme.surfaceBorder),
        ),
        child: Row(
          children: [
            SizedBox(
              height: 160,
              width: 160,
              child: CustomPaint(
                painter: _PieChartPainter(
                  breakdown: breakdown,
                  providerColors: _providerColors,
                  fallbackColors: _fallbackColors,
                ),
                child: const SizedBox.expand(),
              ),
            ),
            const SizedBox(width: 24),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: breakdown.asMap().entries.map((entry) {
                  final b = entry.value;
                  final color = _providerColors[b.provider] ??
                      _fallbackColors[entry.key % _fallbackColors.length];
                  final pct = total > 0
                      ? (b.sessions / total * 100).toStringAsFixed(0)
                      : '0';
                  return Padding(
                    padding: const EdgeInsets.symmetric(vertical: 5),
                    child: Row(
                      children: [
                        Container(
                          width: 10,
                          height: 10,
                          decoration: BoxDecoration(
                            color: color,
                            borderRadius: BorderRadius.circular(2),
                          ),
                        ),
                        const SizedBox(width: 8),
                        Text(
                          b.provider,
                          style: const TextStyle(
                              fontSize: 12, color: Colors.white70),
                        ),
                        const Spacer(),
                        Text(
                          '$pct% · ${b.sessions}',
                          style: const TextStyle(
                              fontSize: 11, color: Colors.white38),
                        ),
                      ],
                    ),
                  );
                }).toList(),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _PieChartPainter extends CustomPainter {
  _PieChartPainter({
    required this.breakdown,
    required this.providerColors,
    required this.fallbackColors,
  });

  final List<ProviderBreakdown> breakdown;
  final Map<String, Color> providerColors;
  final List<Color> fallbackColors;

  @override
  void paint(Canvas canvas, Size size) {
    final total = breakdown.fold(0, (s, b) => s + b.sessions);
    if (total == 0) return;

    final center = Offset(size.width / 2, size.height / 2);
    final outerR = math.min(size.width, size.height) / 2 - 4;
    final innerR = outerR * 0.4;

    double startAngle = -math.pi / 2;
    for (int i = 0; i < breakdown.length; i++) {
      final b = breakdown[i];
      final sweep = 2 * math.pi * b.sessions / total;
      final color =
          providerColors[b.provider] ?? fallbackColors[i % fallbackColors.length];
      final paint = Paint()
        ..color = color
        ..style = PaintingStyle.fill;

      final path = Path()
        ..moveTo(center.dx, center.dy)
        ..arcTo(
          Rect.fromCircle(center: center, radius: outerR),
          startAngle,
          sweep - 0.02, // small gap between slices
          false,
        )
        ..lineTo(center.dx, center.dy);

      canvas.drawPath(path, paint);

      // Knock out inner circle for donut effect.
      final innerPaint = Paint()
        ..color = ClawdTheme.surfaceElevated
        ..style = PaintingStyle.fill;
      canvas.drawCircle(center, innerR, innerPaint);

      startAngle += sweep;
    }
  }

  @override
  bool shouldRepaint(_PieChartPainter old) => old.breakdown != breakdown;
}

// ─── Shared helpers ───────────────────────────────────────────────────────────

class _Header extends StatelessWidget {
  const _Header({required this.title, required this.subtitle});
  final String title;
  final String subtitle;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title,
            style: const TextStyle(
                fontSize: 18,
                fontWeight: FontWeight.w700,
                color: Colors.white)),
        const SizedBox(height: 4),
        Text(subtitle,
            style: const TextStyle(fontSize: 12, color: Colors.white38)),
        const SizedBox(height: 8),
        const Divider(),
      ],
    );
  }
}

class _SectionLabel extends StatelessWidget {
  const _SectionLabel(this.text);
  final String text;

  @override
  Widget build(BuildContext context) {
    return Text(
      text,
      style: const TextStyle(
          fontSize: 12, fontWeight: FontWeight.w600, color: Colors.white60),
    );
  }
}

class _ChartPlaceholder extends StatelessWidget {
  const _ChartPlaceholder();

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 160,
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: const Center(
        child: Text('No data yet',
            style: TextStyle(fontSize: 12, color: Colors.white38)),
      ),
    );
  }
}

class _ErrorCard extends StatelessWidget {
  const _ErrorCard(this.message);
  final String message;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: Colors.red.shade800),
      ),
      child: Text(message,
          style: const TextStyle(fontSize: 12, color: Colors.white54)),
    );
  }
}
