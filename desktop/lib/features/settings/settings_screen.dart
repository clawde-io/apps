import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde/services/updater_service.dart';

enum _Section { connection, providers, appearance, about }

class SettingsScreen extends ConsumerStatefulWidget {
  const SettingsScreen({super.key});

  @override
  ConsumerState<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends ConsumerState<SettingsScreen> {
  _Section _active = _Section.connection;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        // ── Section list (left 200px) ─────────────────────────────────────
        SizedBox(
          width: 200,
          child: Container(
            decoration: const BoxDecoration(
              color: ClawdTheme.surfaceElevated,
              border: Border(
                right: BorderSide(color: ClawdTheme.surfaceBorder),
              ),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Padding(
                  padding: EdgeInsets.fromLTRB(20, 20, 20, 12),
                  child: Text(
                    'Settings',
                    style: TextStyle(
                        fontSize: 16,
                        fontWeight: FontWeight.w700,
                        color: Colors.white),
                  ),
                ),
                const Divider(height: 1),
                const SizedBox(height: 8),
                ..._Section.values.map((s) => _SectionTile(
                      section: s,
                      isActive: _active == s,
                      onTap: () => setState(() => _active = s),
                    )),
              ],
            ),
          ),
        ),
        // ── Content pane (right, scrollable) ─────────────────────────────
        Expanded(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(32),
            child: switch (_active) {
              _Section.connection => const _ConnectionPane(),
              _Section.providers => const _ProvidersPane(),
              _Section.appearance => const _AppearancePane(),
              _Section.about => const _AboutPane(),
            },
          ),
        ),
      ],
    );
  }
}

// ── Section nav tile ──────────────────────────────────────────────────────────

class _SectionTile extends StatelessWidget {
  const _SectionTile({
    required this.section,
    required this.isActive,
    required this.onTap,
  });

  final _Section section;
  final bool isActive;
  final VoidCallback onTap;

  String get _label => switch (section) {
        _Section.connection => 'Connection',
        _Section.providers => 'Providers',
        _Section.appearance => 'Appearance',
        _Section.about => 'About',
      };

  IconData get _icon => switch (section) {
        _Section.connection => Icons.wifi,
        _Section.providers => Icons.auto_awesome,
        _Section.appearance => Icons.palette_outlined,
        _Section.about => Icons.info_outline,
      };

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      child: Container(
        height: 36,
        padding: const EdgeInsets.symmetric(horizontal: 16),
        decoration: BoxDecoration(
          color: isActive
              ? ClawdTheme.claw.withValues(alpha: 0.15)
              : Colors.transparent,
          border: isActive
              ? const Border(
                  left: BorderSide(color: ClawdTheme.claw, width: 2))
              : null,
        ),
        child: Row(
          children: [
            Icon(_icon,
                size: 15,
                color: isActive ? ClawdTheme.clawLight : Colors.white54),
            const SizedBox(width: 10),
            Text(
              _label,
              style: TextStyle(
                fontSize: 13,
                color: isActive ? ClawdTheme.clawLight : Colors.white70,
                fontWeight:
                    isActive ? FontWeight.w600 : FontWeight.normal,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

// ── Connection pane ───────────────────────────────────────────────────────────

class _ConnectionPane extends ConsumerStatefulWidget {
  const _ConnectionPane();

  @override
  ConsumerState<_ConnectionPane> createState() => _ConnectionPaneState();
}

class _ConnectionPaneState extends ConsumerState<_ConnectionPane> {
  TextEditingController? _urlCtrl;
  bool _init = false;

  @override
  void dispose() {
    _urlCtrl?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final settingsAsync = ref.watch(settingsProvider);
    final daemonState = ref.watch(daemonProvider);

    return settingsAsync.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (e, _) => Text('Error: $e'),
      data: (settings) {
        if (!_init) {
          _urlCtrl = TextEditingController(text: settings.daemonUrl);
          _init = true;
        }
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const _Header(
              title: 'Connection',
              subtitle: 'Configure how ClawDE connects to the daemon',
            ),
            const SizedBox(height: 24),
            const _Label('Daemon URL'),
            const SizedBox(height: 6),
            Row(
              children: [
                Expanded(
                  child: TextField(
                    controller: _urlCtrl!,
                    style: const TextStyle(fontSize: 13),
                    decoration: const InputDecoration(
                      hintText: 'ws://127.0.0.1:4300',
                      border: OutlineInputBorder(),
                      contentPadding: EdgeInsets.symmetric(
                          horizontal: 12, vertical: 10),
                    ),
                    onSubmitted: (v) => ref
                        .read(settingsProvider.notifier)
                        .setDaemonUrl(v.trim()),
                  ),
                ),
                const SizedBox(width: 8),
                FilledButton(
                  onPressed: () => ref
                      .read(settingsProvider.notifier)
                      .setDaemonUrl(_urlCtrl!.text.trim()),
                  style: FilledButton.styleFrom(
                      backgroundColor: ClawdTheme.claw),
                  child: const Text('Save'),
                ),
              ],
            ),
            const SizedBox(height: 20),
            SwitchListTile(
              title: const Text('Auto-reconnect',
                  style: TextStyle(fontSize: 13)),
              subtitle: const Text(
                'Automatically reconnect when the daemon drops',
                style: TextStyle(fontSize: 11, color: Colors.white38),
              ),
              value: settings.autoReconnect,
              onChanged: (v) => ref
                  .read(settingsProvider.notifier)
                  .setAutoReconnect(v),
              activeThumbColor: ClawdTheme.claw,
              contentPadding: EdgeInsets.zero,
            ),
            const SizedBox(height: 24),
            const Divider(),
            const SizedBox(height: 16),
            _DaemonCard(daemonState: daemonState),
            const SizedBox(height: 24),
            const Divider(),
            const SizedBox(height: 16),
            _QrSection(url: settings.daemonUrl),
          ],
        );
      },
    );
  }
}

class _QrSection extends StatelessWidget {
  const _QrSection({required this.url});
  final String url;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Text(
          'Scan from Mobile',
          style: TextStyle(
              fontSize: 13, fontWeight: FontWeight.w600, color: Colors.white70),
        ),
        const SizedBox(height: 4),
        const Text(
          'Open ClawDE on your phone and scan this code to connect.',
          style: TextStyle(fontSize: 11, color: Colors.white38),
        ),
        const SizedBox(height: 16),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              padding: const EdgeInsets.all(8),
              decoration: BoxDecoration(
                color: Colors.white,
                borderRadius: BorderRadius.circular(8),
              ),
              child: QrImageView(
                data: url,
                version: QrVersions.auto,
                size: 160,
              ),
            ),
            const SizedBox(width: 16),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    url,
                    style: const TextStyle(
                        fontSize: 12,
                        fontFamily: 'monospace',
                        color: Colors.white70),
                  ),
                  const SizedBox(height: 12),
                  OutlinedButton.icon(
                    onPressed: () =>
                        Clipboard.setData(ClipboardData(text: url)),
                    icon: const Icon(Icons.copy, size: 14),
                    label: const Text('Copy URL'),
                    style: OutlinedButton.styleFrom(
                      foregroundColor: Colors.white60,
                      side:
                          const BorderSide(color: ClawdTheme.surfaceBorder),
                      padding: const EdgeInsets.symmetric(
                          horizontal: 12, vertical: 8),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class _DaemonCard extends ConsumerWidget {
  const _DaemonCard({required this.daemonState});
  final DaemonState daemonState;

  String _fmtUptime(int s) {
    if (s < 60) return '${s}s';
    final m = s ~/ 60;
    if (m < 60) return '${m}m';
    return '${m ~/ 60}h ${m % 60}m';
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final info = daemonState.daemonInfo;
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: ClawdTheme.surface,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Container(
                width: 8,
                height: 8,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: daemonState.isConnected ? Colors.green : Colors.red,
                ),
              ),
              const SizedBox(width: 8),
              Text(
                daemonState.isConnected
                    ? 'Daemon connected'
                    : 'Daemon disconnected',
                style: TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  color: daemonState.isConnected ? Colors.green : Colors.red,
                ),
              ),
              const Spacer(),
              if (!daemonState.isConnected)
                TextButton.icon(
                  onPressed: () =>
                      ref.read(daemonProvider.notifier).reconnect(),
                  icon: const Icon(Icons.refresh, size: 14),
                  label: const Text('Reconnect Now'),
                  style: TextButton.styleFrom(
                      foregroundColor: ClawdTheme.clawLight),
                ),
            ],
          ),
          if (info != null) ...[
            const SizedBox(height: 12),
            const Divider(height: 1),
            const SizedBox(height: 12),
            _Row2('Version', 'v${info.version}'),
            const SizedBox(height: 6),
            _Row2('Uptime', _fmtUptime(info.uptime)),
            const SizedBox(height: 6),
            _Row2('Port', ':${info.port}'),
            const SizedBox(height: 6),
            _Row2('Active sessions', '${info.activeSessions}'),
          ],
        ],
      ),
    );
  }
}

// ── Providers pane ────────────────────────────────────────────────────────────

class _ProvidersPane extends ConsumerWidget {
  const _ProvidersPane();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final settingsAsync = ref.watch(settingsProvider);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const _Header(
          title: 'AI Providers',
          subtitle: 'Set your default provider for new sessions',
        ),
        const SizedBox(height: 24),
        settingsAsync.when(
          loading: () => const CircularProgressIndicator(),
          error: (e, _) => Text('Error: $e'),
          data: (settings) => RadioGroup<ProviderType>(
            groupValue: settings.defaultProvider,
            onChanged: (v) {
              if (v != null) {
                ref.read(settingsProvider.notifier).setDefaultProvider(v);
              }
            },
            child: Column(
              children: ProviderType.values.map((p) {
                final (name, desc, color) = switch (p) {
                  ProviderType.claude => (
                      'Claude',
                      'Best for code generation and architecture',
                      ClawdTheme.claudeColor
                    ),
                  ProviderType.codex => (
                      'Codex',
                      'Best for debugging and explanation',
                      ClawdTheme.codexColor
                    ),
                  ProviderType.cursor => (
                      'Cursor',
                      'Best for navigation and search',
                      ClawdTheme.cursorColor
                    ),
                };
                final isSelected = settings.defaultProvider == p;
                return InkWell(
                  onTap: () => ref
                      .read(settingsProvider.notifier)
                      .setDefaultProvider(p),
                  borderRadius: BorderRadius.circular(8),
                  child: Container(
                    margin: const EdgeInsets.only(bottom: 8),
                    padding: const EdgeInsets.all(14),
                    decoration: BoxDecoration(
                      color: isSelected
                          ? color.withValues(alpha: 0.08)
                          : ClawdTheme.surfaceElevated,
                      borderRadius: BorderRadius.circular(8),
                      border: Border.all(
                        color: isSelected ? color : ClawdTheme.surfaceBorder,
                      ),
                    ),
                    child: Row(
                      children: [
                        Radio<ProviderType>(
                          value: p,
                          materialTapTargetSize:
                              MaterialTapTargetSize.shrinkWrap,
                        ),
                        const SizedBox(width: 8),
                        ProviderBadge(provider: p),
                        const SizedBox(width: 12),
                        Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(name,
                                style: TextStyle(
                                    fontSize: 13,
                                    fontWeight: FontWeight.w600,
                                    color: color)),
                            Text(desc,
                                style: const TextStyle(
                                    fontSize: 11, color: Colors.white38)),
                          ],
                        ),
                      ],
                    ),
                  ),
                );
              }).toList(),
            ),
          ),
        ),
      ],
    );
  }
}

// ── Appearance pane ───────────────────────────────────────────────────────────

class _AppearancePane extends StatelessWidget {
  const _AppearancePane();

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const _Header(
          title: 'Appearance',
          subtitle: 'Customize the look of ClawDE',
        ),
        const SizedBox(height: 24),
        const _Label('Theme'),
        const SizedBox(height: 10),
        SegmentedButton<String>(
          segments: const [
            ButtonSegment(
              value: 'dark',
              icon: Icon(Icons.dark_mode, size: 16),
              label: Text('Dark'),
            ),
            ButtonSegment(
              value: 'light',
              icon: Icon(Icons.light_mode, size: 16),
              label: Text('Light'),
              tooltip: 'Coming soon',
            ),
          ],
          selected: const {'dark'},
          onSelectionChanged: (_) {},
          style: SegmentedButton.styleFrom(
            selectedBackgroundColor:
                ClawdTheme.claw.withValues(alpha: 0.2),
          ),
        ),
        const SizedBox(height: 8),
        const Text(
          'Light theme coming in a future release.',
          style: TextStyle(fontSize: 11, color: Colors.white38),
        ),
      ],
    );
  }
}

// ── About pane ────────────────────────────────────────────────────────────────

class _AboutPane extends StatefulWidget {
  const _AboutPane();

  @override
  State<_AboutPane> createState() => _AboutPaneState();
}

class _AboutPaneState extends State<_AboutPane> {
  String _version = '…';

  @override
  void initState() {
    super.initState();
    PackageInfo.fromPlatform().then((info) {
      if (mounted) setState(() => _version = 'v${info.version}');
    });
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const _Header(
          title: 'About ClawDE',
          subtitle: 'Version info and project links',
        ),
        const SizedBox(height: 32),
        Row(
          children: [
            Container(
              width: 48,
              height: 48,
              decoration: BoxDecoration(
                color: ClawdTheme.claw,
                borderRadius: BorderRadius.circular(12),
              ),
              child: const Icon(Icons.terminal,
                  color: Colors.white, size: 26),
            ),
            const SizedBox(width: 16),
            const Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text('ClawDE',
                    style: TextStyle(
                        fontSize: 22,
                        fontWeight: FontWeight.w700,
                        color: Colors.white)),
                Text('Your IDE. Your Rules.',
                    style: TextStyle(fontSize: 13, color: Colors.white38)),
              ],
            ),
          ],
        ),
        const SizedBox(height: 24),
        Container(
          padding: const EdgeInsets.all(16),
          decoration: BoxDecoration(
            color: ClawdTheme.surfaceElevated,
            borderRadius: BorderRadius.circular(8),
            border: Border.all(color: ClawdTheme.surfaceBorder),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _Row2('Desktop version', _version),
              const SizedBox(height: 8),
              const _Row2('License', 'MIT'),
              const SizedBox(height: 8),
              const _Row2('Source', 'github.com/clawde-io/apps'),
            ],
          ),
        ),
        const SizedBox(height: 16),
        InkWell(
          onTap: () => launchUrl(
            Uri.parse('https://github.com/clawde-io/apps'),
            mode: LaunchMode.externalApplication,
          ),
          borderRadius: BorderRadius.circular(8),
          child: Container(
            padding:
                const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
            decoration: BoxDecoration(
              color: ClawdTheme.surfaceElevated,
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: ClawdTheme.surfaceBorder),
            ),
            child: const Row(
              children: [
                Icon(Icons.code, size: 16, color: Colors.white54),
                SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text('View on GitHub',
                          style: TextStyle(
                              fontSize: 13, color: Colors.white)),
                      Text('github.com/clawde-io/apps',
                          style: TextStyle(
                              fontSize: 11, color: Colors.white38)),
                    ],
                  ),
                ),
                Icon(Icons.open_in_new, size: 14, color: Colors.white38),
              ],
            ),
          ),
        ),
        const SizedBox(height: 12),
        SizedBox(
          width: double.infinity,
          child: OutlinedButton.icon(
            onPressed: () => UpdaterService.instance.checkForUpdates(),
            icon: const Icon(Icons.system_update_alt, size: 16),
            label: const Text('Check for Updates…'),
            style: OutlinedButton.styleFrom(
              foregroundColor: Colors.white70,
              side: const BorderSide(color: ClawdTheme.surfaceBorder),
              padding: const EdgeInsets.symmetric(vertical: 12),
            ),
          ),
        ),
      ],
    );
  }
}

// ── Shared helpers ────────────────────────────────────────────────────────────

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
            style:
                const TextStyle(fontSize: 12, color: Colors.white38)),
        const SizedBox(height: 8),
        const Divider(),
      ],
    );
  }
}

class _Label extends StatelessWidget {
  const _Label(this.text);
  final String text;

  @override
  Widget build(BuildContext context) {
    return Text(text,
        style: const TextStyle(
            fontSize: 12,
            fontWeight: FontWeight.w600,
            color: Colors.white60));
  }
}

class _Row2 extends StatelessWidget {
  const _Row2(this.label, this.value);
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        SizedBox(
          width: 140,
          child: Text(label,
              style:
                  const TextStyle(fontSize: 12, color: Colors.white38)),
        ),
        Text(value,
            style:
                const TextStyle(fontSize: 12, color: Colors.white70)),
      ],
    );
  }
}
