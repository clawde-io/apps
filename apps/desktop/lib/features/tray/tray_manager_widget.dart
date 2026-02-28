// SPDX-License-Identifier: MIT
// Sprint II ST.1 — TrayManagerWidget.
//
// Watches Riverpod session state and keeps the system tray menu
// up-to-date with the live session list and daemon status.

import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:flutter/foundation.dart' show AsyncCallback;
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

/// Widget that bridges Riverpod session state into the system tray menu.
///
/// Embed once near the root of the widget tree (below [ProviderScope]).
/// It is transparent — renders its [child] unchanged — and uses
/// [ref.listen] hooks to push session/daemon changes to [TrayManager].
///
/// Menu structure:
/// ```
/// ● Connected  (status indicator — disabled)
/// ─────────────────
/// Sessions:
///   • My session (running)
///   • Idle session
/// ─────────────────
/// + New Session
/// Show ClawDE
/// ─────────────────
/// Quit ClawDE
/// ```
class TrayManagerWidget extends ConsumerStatefulWidget {
  const TrayManagerWidget({
    super.key,
    required this.child,
    this.onNewSession,
    this.onShowSession,
    this.onQuit,
  });

  final Widget child;

  /// Called when the user taps "New Session" in the tray menu.
  final VoidCallback? onNewSession;

  /// Called when the user taps a session in the tray menu.
  final ValueChanged<String>? onShowSession;

  /// Called when the user taps "Quit ClawDE".
  final AsyncCallback? onQuit;

  @override
  ConsumerState<TrayManagerWidget> createState() => _TrayManagerWidgetState();
}

class _TrayManagerWidgetState extends ConsumerState<TrayManagerWidget>
    with TrayListener {
  List<Session> _sessions = [];
  DaemonStatus _daemonStatus = DaemonStatus.disconnected;

  @override
  void initState() {
    super.initState();
    trayManager.addListener(this);

    // Listen to daemon status changes — update tray status label.
    ref.listenManual<DaemonState>(daemonProvider, (_, next) {
      _daemonStatus = next.status;
      _rebuildMenu();
    });

    // Listen to session list changes — refresh quick-launch session list.
    ref.listenManual<AsyncValue<List<Session>>>(sessionListProvider, (_, next) {
      _sessions = next.valueOrNull ?? [];
      _rebuildMenu();
    });
  }

  @override
  void dispose() {
    trayManager.removeListener(this);
    super.dispose();
  }

  // ─── TrayListener ──────────────────────────────────────────────────────────

  @override
  void onTrayMenuItemClick(MenuItem menuItem) {
    final key = menuItem.key ?? '';
    if (key == 'new_session') {
      _show();
      widget.onNewSession?.call();
    } else if (key == 'show') {
      _show();
    } else if (key == 'quit') {
      widget.onQuit?.call();
    } else if (key.startsWith('session:')) {
      final sessionId = key.substring('session:'.length);
      _show();
      widget.onShowSession?.call(sessionId);
    }
  }

  @override
  void onTrayIconMouseDown() => _show();

  @override
  void onTrayIconRightMouseDown() => trayManager.popUpContextMenu();

  // ─── Private helpers ───────────────────────────────────────────────────────

  Future<void> _show() async {
    await windowManager.show();
    await windowManager.focus();
  }

  /// Rebuild the tray context menu to reflect the latest sessions + status.
  Future<void> _rebuildMenu() async {
    final statusLabel = switch (_daemonStatus) {
      DaemonStatus.connected => '● Connected',
      DaemonStatus.error => '⚠ Daemon error',
      _ => '● Running',
    };

    final sessionItems = _sessions.take(5).map((s) {
      final icon = switch (s.status) {
        SessionStatus.running => '▶ ',
        SessionStatus.paused => '⏸ ',
        _ => '  ',
      };
      return MenuItem(
        key: 'session:${s.id}',
        label: '$icon${s.title.isEmpty ? '(untitled)' : s.title}',
      );
    }).toList();

    final items = <MenuItem>[
      MenuItem(key: 'status', label: statusLabel, disabled: true),
      MenuItem.separator(),
      if (sessionItems.isNotEmpty) ...[
        MenuItem(key: 'sessions_header', label: 'Sessions', disabled: true),
        ...sessionItems,
        MenuItem.separator(),
      ],
      MenuItem(key: 'new_session', label: '+ New Session'),
      MenuItem(key: 'show', label: 'Show ClawDE'),
      MenuItem.separator(),
      MenuItem(key: 'quit', label: 'Quit ClawDE'),
    ];

    await trayManager.setContextMenu(Menu(items: items));
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
