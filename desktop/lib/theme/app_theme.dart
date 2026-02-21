import 'package:flutter/material.dart';

class AppTheme {
  AppTheme._();

  // Brand colors
  static const Color primary = Color(0xFF6B7BFF); // ClawDE indigo
  static const Color secondary = Color(0xFF4ECDC4); // Teal accent
  static const Color surface = Color(0xFF141414);
  static const Color background = Color(0xFF0A0A0A);
  static const Color error = Color(0xFFFF4B4B);
  static const Color textPrimary = Color(0xFFF0F0F0);
  static const Color textMuted = Color(0xFF8A8A8A);
  static const Color border = Color(0xFF2A2A2A);

  // Git status colors (matches web/desktop)
  static const Color gitClean = Color(0xFF4CAF50); // green
  static const Color gitModified = Color(0xFFFF9800); // orange
  static const Color gitStaged = Color(0xFFFFEB3B); // yellow
  static const Color gitDeleted = Color(0xFFFF4B4B); // red
  static const Color gitUntracked = Color(0xFF9E9E9E); // grey

  static ThemeData dark() {
    return ThemeData(
      brightness: Brightness.dark,
      scaffoldBackgroundColor: background,
      colorScheme: const ColorScheme.dark(
        primary: primary,
        secondary: secondary,
        surface: surface,
        error: error,
      ),
      fontFamily: 'monospace',
      useMaterial3: true,
      dividerColor: border,
    );
  }
}
