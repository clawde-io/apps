import 'package:flutter/material.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Global key so [SnackbarService] can show snackbars without BuildContext.
final scaffoldMessengerKey = GlobalKey<ScaffoldMessengerState>();

/// Centralized snackbar / toast service callable from anywhere —
/// including providers and services that have no BuildContext.
class SnackbarService {
  SnackbarService._();
  static final instance = SnackbarService._();

  ScaffoldMessengerState? get _messenger =>
      scaffoldMessengerKey.currentState;

  void showError(String message) => _show(message, ClawdTheme.error);
  void showSuccess(String message) => _show(message, ClawdTheme.success);
  void showInfo(String message) => _show(message, ClawdTheme.info);

  void _show(String message, Color color) {
    _messenger?.showSnackBar(
      SnackBar(
        content: Text(message),
        backgroundColor: color,
        behavior: SnackBarBehavior.floating,
        duration: const Duration(seconds: 4),
      ),
    );
  }
}
