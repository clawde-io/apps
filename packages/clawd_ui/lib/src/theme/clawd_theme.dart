import 'package:flutter/material.dart';

/// ClawDE design tokens and theme factory.
/// Apps call [ClawdTheme.dark()] / [ClawdTheme.light()] when building MaterialApp.
abstract final class ClawdTheme {
  // Brand colours
  static const Color claw = Color(0xFF7C3AED); // purple-600
  static const Color clawLight = Color(0xFFA78BFA); // purple-400
  static const Color clawDark = Color(0xFF5B21B6); // purple-800

  // Surface colours (dark mode)
  static const Color surface = Color(0xFF0F0F12);
  static const Color surfaceElevated = Color(0xFF1A1A24);
  static const Color surfaceBorder = Color(0xFF2A2A38);

  // Message bubble colours
  static const Color userBubble = Color(0xFF1E1B4B); // indigo-950
  static const Color assistantBubble = Color(0xFF111118); // near-black

  // Status colours
  static const Color success = Color(0xFF10B981); // emerald-500
  static const Color warning = Color(0xFFF59E0B); // amber-500
  static const Color error = Color(0xFFEF4444); // red-500
  static const Color info = Color(0xFF3B82F6); // blue-500

  // Provider colours
  static const Color claudeColor = Color(0xFFD97706); // amber — Claude
  static const Color codexColor = Color(0xFF10B981); // emerald — Codex/GPT
  static const Color cursorColor = Color(0xFF3B82F6); // blue — Cursor
  static const Color aiderColor = Color(0xFF8B5CF6); // violet — Aider

  static ThemeData dark() {
    final base = ThemeData.dark(useMaterial3: true);
    return base.copyWith(
      colorScheme: base.colorScheme.copyWith(
        primary: claw,
        secondary: clawLight,
        surface: surface,
        onSurface: Colors.white,
      ),
      scaffoldBackgroundColor: surface,
      cardColor: surfaceElevated,
      dividerColor: surfaceBorder,
      textTheme: base.textTheme.copyWith(
        bodyMedium: const TextStyle(fontSize: 14, height: 1.5),
        bodySmall: const TextStyle(fontSize: 12, height: 1.4),
      ),
    );
  }

  static ThemeData light() {
    final base = ThemeData.light(useMaterial3: true);
    return base.copyWith(
      colorScheme: base.colorScheme.copyWith(
        primary: claw,
        secondary: clawDark,
      ),
    );
  }
}
