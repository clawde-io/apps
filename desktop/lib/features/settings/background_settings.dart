// SPDX-License-Identifier: MIT
// Sprint II ST.6 — Background Mode settings section.

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Settings section for background mode and start-at-login options.
///
/// Placed inside the "General" subsection of the Settings screen.
class BackgroundModeSettings extends ConsumerWidget {
  const BackgroundModeSettings({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final settings = ref.watch(settingsProvider);
    final backgroundEnabled = settings.valueOrNull?.backgroundMode ?? true;
    final startAtLogin = settings.valueOrNull?.startAtLogin ?? false;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(0, 0, 0, 8),
          child: Text(
            'BACKGROUND MODE',
            style: TextStyle(
              color: Colors.white.withValues(alpha: 0.45),
              fontSize: 11,
              fontWeight: FontWeight.w600,
              letterSpacing: 0.8,
            ),
          ),
        ),
        _SettingsTile(
          title: 'Keep running when window is closed',
          subtitle:
              'The daemon continues running in the background. '
              'Access it from the system tray.',
          value: backgroundEnabled,
          onChanged: (v) =>
              ref.read(settingsProvider.notifier).setBackgroundMode(v),
        ),
        const SizedBox(height: 8),
        _SettingsTile(
          title: 'Start at login',
          subtitle:
              'Automatically start the ClawDE daemon when you log in. '
              'Installs a platform service (launchd / systemd).',
          value: startAtLogin,
          onChanged: (v) =>
              ref.read(settingsProvider.notifier).setStartAtLogin(v),
        ),
      ],
    );
  }
}

// ─── Private helpers ─────────────────────────────────────────────────────────

class _SettingsTile extends StatelessWidget {
  const _SettingsTile({
    required this.title,
    required this.subtitle,
    required this.value,
    required this.onChanged,
  });

  final String title;
  final String subtitle;
  final bool value;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: SwitchListTile(
        title: const Text(
          '',
          // real title set below via subtitle slot — avoids const mismatch
        ),
        subtitle: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              title,
              style: const TextStyle(color: Colors.white, fontSize: 14),
            ),
            const SizedBox(height: 2),
            Text(
              subtitle,
              style: TextStyle(
                color: Colors.white.withValues(alpha: 0.55),
                fontSize: 12,
              ),
            ),
          ],
        ),
        value: value,
        activeThumbColor: ClawdTheme.claw,
        activeTrackColor: ClawdTheme.clawDark,
        onChanged: onChanged,
        contentPadding:
            const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
      ),
    );
  }
}
