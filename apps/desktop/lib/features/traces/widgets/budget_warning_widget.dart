// budget_warning_widget.dart — Budget warning banner (Sprint PP OB.10).
//
// Listens for `budget_warning` and `budget_exceeded` push events and shows a
// dismissable banner. The banner auto-reappears if a new event fires.

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';

enum _BudgetState { ok, warning, exceeded }

class BudgetWarningWidget extends ConsumerStatefulWidget {
  const BudgetWarningWidget({super.key});

  @override
  ConsumerState<BudgetWarningWidget> createState() =>
      _BudgetWarningWidgetState();
}

class _BudgetWarningWidgetState extends ConsumerState<BudgetWarningWidget> {
  _BudgetState _state = _BudgetState.ok;
  int _pct = 0;
  bool _dismissed = false;

  @override
  void initState() {
    super.initState();
    ref.listenManual(daemonPushEventsProvider, (_, next) {
      next.whenData((event) {
        final method = event['method'] as String?;
        final params = event['params'] as Map<String, dynamic>? ?? {};
        final pct = (params['threshold_pct'] as num?)?.toInt() ?? 0;

        if (method == 'budget_exceeded') {
          setState(() {
            _state = _BudgetState.exceeded;
            _pct = pct;
            _dismissed = false;
          });
        } else if (method == 'budget_warning') {
          setState(() {
            _state = _BudgetState.warning;
            _pct = pct;
            _dismissed = false;
          });
        }
      });
    });
  }

  @override
  Widget build(BuildContext context) {
    if (_state == _BudgetState.ok || _dismissed) return const SizedBox.shrink();

    final isExceeded = _state == _BudgetState.exceeded;
    final color = isExceeded ? Colors.red : Colors.amber;
    final icon = isExceeded ? Icons.block : Icons.warning_amber_outlined;
    final message = isExceeded
        ? 'Budget exceeded ($_pct% of limit). New sessions paused.'
        : 'Budget warning: $_pct% of configured limit reached.';

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      color: color.withValues(alpha: 0.12),
      child: Row(
        children: [
          Icon(icon, size: 16, color: color),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              message,
              style: TextStyle(fontSize: 12, color: color),
            ),
          ),
          IconButton(
            icon: const Icon(Icons.close, size: 14),
            color: color.withValues(alpha: 0.6),
            padding: EdgeInsets.zero,
            constraints: const BoxConstraints(minWidth: 24, minHeight: 24),
            onPressed: () => setState(() => _dismissed = true),
          ),
        ],
      ),
    );
  }
}
