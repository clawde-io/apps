import 'dart:developer' as dev;

import 'package:flutter/foundation.dart';
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

/// Icon state for the system tray.
enum TrayIconState {
  /// Daemon running, no active relay client.
  running,

  /// Daemon running and at least one remote relay client is connected.
  connected,

  /// Daemon failed to start or encountered an unrecoverable error.
  error,
}

/// System tray service for ClawDE desktop.
///
/// Manages the tray icon, tooltip, and context menu.  Supports three
/// icon states ([TrayIconState]) that reflect daemon/relay health.
///
/// Usage:
/// ```dart
/// await TrayService.instance.init(onQuit: () async {
///   await DaemonManager.instance.shutdown();
///   await windowManager.destroy();
/// });
/// ```
class TrayService with TrayListener {
  TrayService._();
  static final TrayService instance = TrayService._();

  TrayIconState _state = TrayIconState.running;
  AsyncCallback? _onQuit;

  TrayIconState get state => _state;

  /// Initialise the tray icon and menu.
  ///
  /// [onQuit] is called when the user clicks "Quit" from the tray menu.
  /// It should shut down the daemon and destroy the window.
  Future<void> init({required AsyncCallback onQuit}) async {
    _onQuit = onQuit;
    trayManager.addListener(this);
    await _applyState(_state);
    await trayManager.setToolTip('ClawDE');
    await _rebuildMenu();
    dev.log('tray service initialized', name: 'TrayService');
  }

  /// Update the tray icon to reflect the new [state].
  Future<void> setState(TrayIconState state) async {
    if (_state == state) return;
    _state = state;
    await _applyState(state);
    await _rebuildMenu(); // rebuild to update state label if needed
  }

  Future<void> _applyState(TrayIconState state) async {
    // Tray icons are stored in flutter assets (22–32 px PNGs recommended).
    // Replace placeholder icons with final branded assets before release.
    final path = switch (state) {
      TrayIconState.connected => 'assets/tray_icon_connected.png',
      TrayIconState.error => 'assets/tray_icon_error.png',
      TrayIconState.running => 'assets/tray_icon.png',
    };
    await trayManager.setIcon(path);
  }

  Future<void> _rebuildMenu() async {
    final stateLabel = switch (_state) {
      TrayIconState.connected => '● Connected',
      TrayIconState.error => '⚠ Daemon error',
      TrayIconState.running => '● Running',
    };

    final menu = Menu(
      items: [
        MenuItem(
          key: 'status',
          label: stateLabel,
          disabled: true, // informational only
        ),
        MenuItem.separator(),
        MenuItem(
          key: 'show',
          label: 'Show ClawDE',
        ),
        MenuItem.separator(),
        MenuItem(
          key: 'quit',
          label: 'Quit ClawDE',
        ),
      ],
    );
    await trayManager.setContextMenu(menu);
  }

  // ─── TrayListener ────────────────────────────────────────────────────────

  @override
  void onTrayIconMouseDown() {
    _toggleWindow();
  }

  @override
  void onTrayIconRightMouseDown() {
    trayManager.popUpContextMenu();
  }

  @override
  void onTrayMenuItemClick(MenuItem menuItem) {
    switch (menuItem.key) {
      case 'show':
        _showWindow();
      case 'quit':
        _quit();
    }
  }

  // ─── Window helpers ───────────────────────────────────────────────────────

  void _toggleWindow() {
    windowManager.isVisible().then((visible) {
      if (visible) {
        windowManager.hide();
      } else {
        _showWindow();
      }
    });
  }

  Future<void> _showWindow() async {
    await windowManager.show();
    await windowManager.focus();
  }

  Future<void> _quit() async {
    try {
      await _onQuit?.call();
    } catch (e) {
      dev.log('quit error: $e', name: 'TrayService');
    }
  }
}
