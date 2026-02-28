import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:go_router/go_router.dart';
import 'package:clawde/router.dart';
import 'package:clawde/services/updater_service.dart';
import 'package:clawde/features/settings/remote_access_settings.dart';

enum _Section { connection, remoteAccess, providers, models, appearance, resources, doctor, about }

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
              _Section.remoteAccess => const RemoteAccessSettings(),
              _Section.providers => const _ProvidersPane(),
              _Section.models => const _ModelsPane(),
              _Section.appearance => const _AppearancePane(),
              _Section.resources => const _ResourcesPane(),
              _Section.doctor => const _DoctorPane(),
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
        _Section.remoteAccess => 'Remote Access',
        _Section.providers => 'Providers',
        _Section.models => 'Models',
        _Section.appearance => 'Appearance',
        _Section.resources => 'Resources',
        _Section.doctor => 'Doctor',
        _Section.about => 'About',
      };

  IconData get _icon => switch (section) {
        _Section.connection => Icons.wifi,
        _Section.remoteAccess => Icons.devices,
        _Section.providers => Icons.auto_awesome,
        _Section.models => Icons.auto_awesome_mosaic,
        _Section.appearance => Icons.palette_outlined,
        _Section.resources => Icons.memory,
        _Section.doctor => Icons.health_and_safety_outlined,
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

/// Available locales — must match supportedLocales in app.dart.
const _kLocales = [
  (code: 'en', label: 'English'),
  (code: 'fr', label: 'Français'),
  (code: 'ja', label: '日本語'),
];

class _AppearancePane extends StatefulWidget {
  const _AppearancePane();

  @override
  State<_AppearancePane> createState() => _AppearancePaneState();
}

class _AppearancePaneState extends State<_AppearancePane> {
  // Default to the system locale code, clamped to supported locales.
  late String _selectedLocale;

  @override
  void initState() {
    super.initState();
    final systemCode = WidgetsBinding.instance.platformDispatcher.locale.languageCode;
    _selectedLocale = _kLocales.any((l) => l.code == systemCode)
        ? systemCode
        : 'en';
  }

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
        const Row(
          children: [
            Icon(Icons.dark_mode, size: 16, color: ClawdTheme.claw),
            SizedBox(width: 8),
            Text('Dark', style: TextStyle(fontSize: 13, color: Colors.white)),
          ],
        ),
        const SizedBox(height: 28),
        const Divider(),
        const SizedBox(height: 20),
        const _Label('Language'),
        const SizedBox(height: 6),
        const Text(
          'Choose the display language. Restart required to apply fully.',
          style: TextStyle(fontSize: 11, color: Colors.white38),
        ),
        const SizedBox(height: 12),
        SizedBox(
          width: 260,
          child: DropdownButtonFormField<String>(
            initialValue: _selectedLocale,
            decoration: const InputDecoration(
              border: OutlineInputBorder(),
              contentPadding:
                  EdgeInsets.symmetric(horizontal: 12, vertical: 10),
            ),
            items: _kLocales
                .map(
                  (l) => DropdownMenuItem(
                    value: l.code,
                    child: Text(
                      '${l.label} (${l.code})',
                      style: const TextStyle(fontSize: 13),
                    ),
                  ),
                )
                .toList(),
            onChanged: (v) {
              if (v != null) setState(() => _selectedLocale = v);
            },
          ),
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

// ── Resources pane ────────────────────────────────────────────────────────────

/// Ephemeral config state used by the resources settings sliders.
class _ResConfig {
  const _ResConfig({
    required this.maxMemoryPercent,
    required this.maxConcurrentActive,
    required this.idleToWarmSecs,
    required this.warmToColdSecs,
  });

  final int maxMemoryPercent;
  final int maxConcurrentActive;
  final int idleToWarmSecs;
  final int warmToColdSecs;

  static const p8gb = _ResConfig(
    maxMemoryPercent: 60,
    maxConcurrentActive: 2,
    idleToWarmSecs: 60,
    warmToColdSecs: 180,
  );
  static const p16gb = _ResConfig(
    maxMemoryPercent: 70,
    maxConcurrentActive: 4,
    idleToWarmSecs: 120,
    warmToColdSecs: 300,
  );
  static const p32gb = _ResConfig(
    maxMemoryPercent: 75,
    maxConcurrentActive: 6,
    idleToWarmSecs: 180,
    warmToColdSecs: 600,
  );
  static const p64gb = _ResConfig(
    maxMemoryPercent: 80,
    maxConcurrentActive: 10,
    idleToWarmSecs: 300,
    warmToColdSecs: 1200,
  );

  String toToml() {
    final concStr =
        maxConcurrentActive == 0 ? '0  # 0 = auto-calculate' : '$maxConcurrentActive';
    return '[resources]\nmax_memory_percent = $maxMemoryPercent\nmax_concurrent_active = $concStr\nidle_to_warm_secs = $idleToWarmSecs\nwarm_to_cold_secs = $warmToColdSecs';
  }

  @override
  bool operator ==(Object other) =>
      other is _ResConfig &&
      maxMemoryPercent == other.maxMemoryPercent &&
      maxConcurrentActive == other.maxConcurrentActive &&
      idleToWarmSecs == other.idleToWarmSecs &&
      warmToColdSecs == other.warmToColdSecs;

  @override
  int get hashCode => Object.hash(
      maxMemoryPercent, maxConcurrentActive, idleToWarmSecs, warmToColdSecs);
}

class _ResourcesPane extends ConsumerStatefulWidget {
  const _ResourcesPane();

  @override
  ConsumerState<_ResourcesPane> createState() => _ResourcesPaneState();
}

class _ResourcesPaneState extends ConsumerState<_ResourcesPane> {
  _ResConfig _cfg = _ResConfig.p16gb;
  bool _presetApplied = false;

  String _fmtSecs(int s) {
    if (s < 60) return '${s}s';
    final m = s ~/ 60;
    final rem = s % 60;
    return rem == 0 ? '${m}m' : '${m}m ${rem}s';
  }

  int _roundToStep(double v, int step) => ((v / step).round() * step);

  @override
  Widget build(BuildContext context) {
    final statsAsync = ref.watch(resourceStatsProvider);
    final stats = statsAsync.value;

    // Auto-select preset once we have RAM data.
    if (stats != null && !_presetApplied) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        setState(() {
          _presetApplied = true;
          final gb = stats.ram.totalBytes / 1024 / 1024 / 1024;
          _cfg = gb < 12
              ? _ResConfig.p8gb
              : gb < 24
                  ? _ResConfig.p16gb
                  : gb < 48
                      ? _ResConfig.p32gb
                      : _ResConfig.p64gb;
        });
      });
    }

    final budgetMb = stats != null
        ? (stats.ram.totalBytes * _cfg.maxMemoryPercent ~/ 100 ~/ 1024 ~/ 1024)
        : null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const _Header(
          title: 'Resource Governor',
          subtitle: 'Control how much system memory the daemon and sessions use',
        ),
        const SizedBox(height: 20),

        // Live RAM overview
        statsAsync.when(
          loading: () => const LinearProgressIndicator(),
          error: (_, __) => const Text(
            'Resource stats unavailable — daemon may be disconnected',
            style: TextStyle(fontSize: 12, color: Colors.white38),
          ),
          data: (s) => s != null
              ? _RamOverviewCard(stats: s)
              : const Text(
                  'Resource stats unavailable',
                  style: TextStyle(fontSize: 12, color: Colors.white38),
                ),
        ),

        const SizedBox(height: 24),

        // Hardware presets
        const _Label('Hardware Preset'),
        const SizedBox(height: 4),
        const Text(
          'Tune defaults for your machine size. Auto-detected from live RAM data.',
          style: TextStyle(fontSize: 11, color: Colors.white38),
        ),
        const SizedBox(height: 10),
        Row(
          children: [
            _PresetChip(
              label: '8 GB',
              isSelected: _cfg == _ResConfig.p8gb,
              onTap: () => setState(() => _cfg = _ResConfig.p8gb),
            ),
            const SizedBox(width: 8),
            _PresetChip(
              label: '16 GB',
              isSelected: _cfg == _ResConfig.p16gb,
              onTap: () => setState(() => _cfg = _ResConfig.p16gb),
            ),
            const SizedBox(width: 8),
            _PresetChip(
              label: '32 GB',
              isSelected: _cfg == _ResConfig.p32gb,
              onTap: () => setState(() => _cfg = _ResConfig.p32gb),
            ),
            const SizedBox(width: 8),
            _PresetChip(
              label: '64 GB',
              isSelected: _cfg == _ResConfig.p64gb,
              onTap: () => setState(() => _cfg = _ResConfig.p64gb),
            ),
          ],
        ),

        const SizedBox(height: 24),

        // Memory limit slider
        const _Label('Memory Limit'),
        const SizedBox(height: 2),
        _SliderRow(
          label: '${_cfg.maxMemoryPercent}% of RAM',
          extra: budgetMb != null ? '≈ $budgetMb MB budget' : null,
          value: _cfg.maxMemoryPercent.toDouble(),
          min: 10,
          max: 90,
          divisions: 16,
          onChanged: (v) => setState(() => _cfg = _ResConfig(
                maxMemoryPercent: v.round(),
                maxConcurrentActive: _cfg.maxConcurrentActive,
                idleToWarmSecs: _cfg.idleToWarmSecs,
                warmToColdSecs: _cfg.warmToColdSecs,
              )),
        ),

        const SizedBox(height: 16),

        // Active session limit slider
        const _Label('Active Session Limit'),
        const SizedBox(height: 2),
        _SliderRow(
          label: _cfg.maxConcurrentActive == 0
              ? 'Auto'
              : '${_cfg.maxConcurrentActive} sessions',
          value: _cfg.maxConcurrentActive.toDouble(),
          min: 0,
          max: 16,
          divisions: 16,
          onChanged: (v) => setState(() => _cfg = _ResConfig(
                maxMemoryPercent: _cfg.maxMemoryPercent,
                maxConcurrentActive: v.round(),
                idleToWarmSecs: _cfg.idleToWarmSecs,
                warmToColdSecs: _cfg.warmToColdSecs,
              )),
        ),

        const SizedBox(height: 16),

        // Idle → Warm slider
        const _Label('Idle → Warm after'),
        const SizedBox(height: 2),
        _SliderRow(
          label: _fmtSecs(_cfg.idleToWarmSecs),
          value: _cfg.idleToWarmSecs.toDouble(),
          min: 30,
          max: 600,
          divisions: 19,
          onChanged: (v) => setState(() => _cfg = _ResConfig(
                maxMemoryPercent: _cfg.maxMemoryPercent,
                maxConcurrentActive: _cfg.maxConcurrentActive,
                idleToWarmSecs: _roundToStep(v, 30),
                warmToColdSecs: _cfg.warmToColdSecs,
              )),
        ),

        const SizedBox(height: 16),

        // Warm → Cold slider
        const _Label('Warm → Cold after'),
        const SizedBox(height: 2),
        _SliderRow(
          label: _fmtSecs(_cfg.warmToColdSecs),
          value: _cfg.warmToColdSecs.toDouble(),
          min: 60,
          max: 1800,
          divisions: 29,
          onChanged: (v) => setState(() => _cfg = _ResConfig(
                maxMemoryPercent: _cfg.maxMemoryPercent,
                maxConcurrentActive: _cfg.maxConcurrentActive,
                idleToWarmSecs: _cfg.idleToWarmSecs,
                warmToColdSecs: _roundToStep(v, 60),
              )),
        ),

        const SizedBox(height: 24),
        const Divider(),
        const SizedBox(height: 16),

        // TOML config snippet
        const _Label('Apply to config.toml'),
        const SizedBox(height: 4),
        const Text(
          "Paste this into your daemon's config.toml and restart to apply.",
          style: TextStyle(fontSize: 11, color: Colors.white38),
        ),
        const SizedBox(height: 10),
        Container(
          padding: const EdgeInsets.all(14),
          decoration: BoxDecoration(
            color: const Color(0xFF0a0a0f),
            borderRadius: BorderRadius.circular(8),
            border: Border.all(color: ClawdTheme.surfaceBorder),
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: SelectableText(
                  _cfg.toToml(),
                  style: const TextStyle(
                    fontSize: 12,
                    fontFamily: 'monospace',
                    color: Colors.white70,
                    height: 1.6,
                  ),
                ),
              ),
              IconButton(
                onPressed: () {
                  Clipboard.setData(ClipboardData(text: _cfg.toToml()));
                  ScaffoldMessenger.of(context).showSnackBar(
                    const SnackBar(
                      content: Text('Config copied'),
                      duration: Duration(seconds: 2),
                      behavior: SnackBarBehavior.floating,
                    ),
                  );
                },
                icon: const Icon(Icons.copy, size: 16),
                color: Colors.white38,
                tooltip: 'Copy',
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
              ),
            ],
          ),
        ),
        const SizedBox(height: 8),
        const Text(
          'macOS: ~/Library/Application Support/clawd/config.toml',
          style: TextStyle(fontSize: 10, color: Colors.white24),
        ),
        const Text(
          'Linux: ~/.local/share/clawd/config.toml',
          style: TextStyle(fontSize: 10, color: Colors.white24),
        ),
      ],
    );
  }
}

class _RamOverviewCard extends StatelessWidget {
  const _RamOverviewCard({required this.stats});
  final ResourceStats stats;

  @override
  Widget build(BuildContext context) {
    final ram = stats.ram;
    final pct = ram.usedPercent.clamp(0, 100);
    final barColor = pct > 90
        ? Colors.red
        : pct > 75
            ? Colors.orange
            : ClawdTheme.claw;

    return Container(
      padding: const EdgeInsets.all(14),
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
              const Icon(Icons.memory, size: 14, color: Colors.white38),
              const SizedBox(width: 6),
              const Text(
                'System RAM',
                style: TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                    color: Colors.white70),
              ),
              const Spacer(),
              Text(
                '${ram.usedMb} / ${ram.totalGb}',
                style: const TextStyle(fontSize: 11, color: Colors.white38),
              ),
            ],
          ),
          const SizedBox(height: 8),
          ClipRRect(
            borderRadius: BorderRadius.circular(2),
            child: LinearProgressIndicator(
              value: pct / 100,
              backgroundColor: ClawdTheme.surfaceBorder,
              valueColor: AlwaysStoppedAnimation<Color>(barColor),
              minHeight: 6,
            ),
          ),
          const SizedBox(height: 10),
          Row(
            children: [
              _ResStat('Used', ram.usedMb),
              const SizedBox(width: 20),
              _ResStat('Daemon', ram.daemonMb),
              const SizedBox(width: 20),
              _ResStat(
                'Sessions',
                '${stats.sessions.active}A · ${stats.sessions.warm}W · ${stats.sessions.cold}C',
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _ResStat extends StatelessWidget {
  const _ResStat(this.label, this.value);
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(label,
            style: const TextStyle(fontSize: 9, color: Colors.white24)),
        Text(value,
            style: const TextStyle(fontSize: 11, color: Colors.white70)),
      ],
    );
  }
}

class _PresetChip extends StatelessWidget {
  const _PresetChip({
    required this.label,
    required this.isSelected,
    required this.onTap,
  });
  final String label;
  final bool isSelected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(6),
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 150),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 7),
        decoration: BoxDecoration(
          color: isSelected
              ? ClawdTheme.claw.withValues(alpha: 0.15)
              : ClawdTheme.surfaceElevated,
          borderRadius: BorderRadius.circular(6),
          border: Border.all(
            color: isSelected ? ClawdTheme.claw : ClawdTheme.surfaceBorder,
          ),
        ),
        child: Text(
          label,
          style: TextStyle(
            fontSize: 12,
            color: isSelected ? ClawdTheme.clawLight : Colors.white54,
            fontWeight: isSelected ? FontWeight.w600 : FontWeight.normal,
          ),
        ),
      ),
    );
  }
}

class _SliderRow extends StatelessWidget {
  const _SliderRow({
    required this.label,
    required this.value,
    required this.min,
    required this.max,
    required this.divisions,
    required this.onChanged,
    this.extra,
  });
  final String label;
  final String? extra;
  final double value;
  final double min;
  final double max;
  final int divisions;
  final ValueChanged<double> onChanged;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Expanded(
          child: SliderTheme(
            data: SliderTheme.of(context).copyWith(
              trackHeight: 3,
              thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 6),
              overlayShape:
                  const RoundSliderOverlayShape(overlayRadius: 12),
              activeTrackColor: ClawdTheme.claw,
              thumbColor: ClawdTheme.claw,
              inactiveTrackColor: ClawdTheme.surfaceBorder,
              overlayColor: ClawdTheme.claw.withValues(alpha: 0.15),
            ),
            child: Slider(
              value: value.clamp(min, max),
              min: min,
              max: max,
              divisions: divisions,
              onChanged: onChanged,
            ),
          ),
        ),
        SizedBox(
          width: 120,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Text(label,
                  style: const TextStyle(
                      fontSize: 12, color: Colors.white70)),
              if (extra != null)
                Text(extra!,
                    style: const TextStyle(
                        fontSize: 10, color: Colors.white38)),
            ],
          ),
        ),
      ],
    );
  }
}

// ── Models pane (MI.T15) ──────────────────────────────────────────────────────

/// Default model per task type + monthly budget cap (MI.T15).
///
/// Changes are shown as a config.toml snippet — the user copies and applies.
/// There is no live write-back: editing daemon config requires a restart.
class _ModelsPane extends StatefulWidget {
  const _ModelsPane();

  @override
  State<_ModelsPane> createState() => _ModelsPaneState();
}

class _ModelsPaneState extends State<_ModelsPane> {
  // Default model per task type, matching daemon TaskType enum.
  static const _taskTypes = [
    'Architecture',
    'CodeReview',
    'Debugging',
    'Documentation',
    'General',
    'QuickEdit',
    'Search',
  ];

  static const _models = [
    ('Opus', 'claude-opus-4-6'),
    ('Sonnet', 'claude-sonnet-4-6'),
    ('Haiku', 'claude-haiku-4-5-20251001'),
  ];

  // Task type → model ID
  late final Map<String, String> _routing;

  // Monthly budget in USD (0 = no cap)
  double _budgetUsd = 0.0;

  @override
  void initState() {
    super.initState();
    // Sensible defaults matching the daemon router defaults.
    _routing = {
      'Architecture': 'claude-opus-4-6',
      'CodeReview': 'claude-sonnet-4-6',
      'Debugging': 'claude-sonnet-4-6',
      'Documentation': 'claude-haiku-4-5-20251001',
      'General': 'claude-sonnet-4-6',
      'QuickEdit': 'claude-haiku-4-5-20251001',
      'Search': 'claude-haiku-4-5-20251001',
    };
  }

  String _toToml() {
    final lines = StringBuffer('[model_intelligence]\n');
    if (_budgetUsd > 0) {
      lines.writeln('monthly_budget_usd = ${_budgetUsd.toStringAsFixed(2)}');
    }
    lines.writeln();
    lines.writeln('[routing]');
    for (final task in _taskTypes) {
      final model = _routing[task] ?? 'claude-sonnet-4-6';
      final key = task.toLowerCase().replaceAll(' ', '_');
      lines.writeln('$key = "$model"');
    }
    return lines.toString().trimRight();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const _Header(
          title: 'Model Intelligence',
          subtitle: 'Default model per task type and monthly budget cap',
        ),
        const SizedBox(height: 20),

        // Task type routing table
        const _Label('Default model per task type'),
        const SizedBox(height: 4),
        const Text(
          'The daemon auto-routes each message based on task type. Override here.',
          style: TextStyle(fontSize: 11, color: Colors.white38),
        ),
        const SizedBox(height: 12),
        ...(_taskTypes.map((task) {
          final current = _routing[task] ?? 'claude-sonnet-4-6';
          return Padding(
            padding: const EdgeInsets.only(bottom: 8),
            child: Row(
              children: [
                SizedBox(
                  width: 130,
                  child: Text(
                    task,
                    style: const TextStyle(fontSize: 12, color: Colors.white70),
                  ),
                ),
                ..._models.map(((String, String) opt) {
                  final (label, id) = opt;
                  final selected = current == id;
                  return Padding(
                    padding: const EdgeInsets.only(right: 6),
                    child: InkWell(
                      onTap: () =>
                          setState(() => _routing[task] = id),
                      borderRadius: BorderRadius.circular(6),
                      child: AnimatedContainer(
                        duration: const Duration(milliseconds: 120),
                        padding: const EdgeInsets.symmetric(
                            horizontal: 10, vertical: 5),
                        decoration: BoxDecoration(
                          color: selected
                              ? Colors.amber.withValues(alpha: 0.15)
                              : ClawdTheme.surfaceElevated,
                          borderRadius: BorderRadius.circular(6),
                          border: Border.all(
                            color: selected
                                ? Colors.amber
                                : ClawdTheme.surfaceBorder,
                          ),
                        ),
                        child: Text(
                          label,
                          style: TextStyle(
                            fontSize: 11,
                            color: selected
                                ? Colors.amber
                                : Colors.white54,
                            fontWeight: selected
                                ? FontWeight.w700
                                : FontWeight.normal,
                          ),
                        ),
                      ),
                    ),
                  );
                }),
              ],
            ),
          );
        })),

        const SizedBox(height: 24),
        const Divider(),
        const SizedBox(height: 16),

        // Monthly budget cap slider
        const _Label('Monthly budget cap'),
        const SizedBox(height: 4),
        const Text(
          'Daemon emits budgetWarning at 80% and budgetExceeded at 100% (blocks new turns).',
          style: TextStyle(fontSize: 11, color: Colors.white38),
        ),
        const SizedBox(height: 10),
        _SliderRow(
          label: _budgetUsd == 0
              ? 'No cap'
              : '\$${_budgetUsd.toStringAsFixed(2)}/mo',
          value: _budgetUsd,
          min: 0,
          max: 100,
          divisions: 20,
          onChanged: (v) => setState(() => _budgetUsd = v),
        ),

        const SizedBox(height: 24),
        const Divider(),
        const SizedBox(height: 16),

        // TOML snippet
        const _Label('Apply to config.toml'),
        const SizedBox(height: 4),
        const Text(
          "Paste into your daemon's config.toml and restart to apply.",
          style: TextStyle(fontSize: 11, color: Colors.white38),
        ),
        const SizedBox(height: 10),
        Container(
          padding: const EdgeInsets.all(14),
          decoration: BoxDecoration(
            color: const Color(0xFF0a0a0f),
            borderRadius: BorderRadius.circular(8),
            border: Border.all(color: ClawdTheme.surfaceBorder),
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: SelectableText(
                  _toToml(),
                  style: const TextStyle(
                    fontSize: 12,
                    fontFamily: 'monospace',
                    color: Colors.white70,
                    height: 1.6,
                  ),
                ),
              ),
              IconButton(
                onPressed: () {
                  Clipboard.setData(ClipboardData(text: _toToml()));
                  ScaffoldMessenger.of(context).showSnackBar(
                    const SnackBar(
                      content: Text('Config copied'),
                      duration: Duration(seconds: 2),
                      behavior: SnackBarBehavior.floating,
                    ),
                  );
                },
                icon: const Icon(Icons.copy, size: 16),
                color: Colors.white38,
                tooltip: 'Copy',
                padding: EdgeInsets.zero,
                constraints:
                    const BoxConstraints(minWidth: 28, minHeight: 28),
              ),
            ],
          ),
        ),
        const SizedBox(height: 24),
        // MI.T16 — link to usage dashboard
        OutlinedButton.icon(
          onPressed: () => context.go(routeUsage),
          icon: const Icon(Icons.receipt_long, size: 14),
          label: const Text('View Usage Dashboard'),
          style: OutlinedButton.styleFrom(
            foregroundColor: Colors.amber,
            side: const BorderSide(color: Colors.amber),
            padding: const EdgeInsets.symmetric(
                horizontal: 16, vertical: 10),
          ),
        ),
      ],
    );
  }
}

// ── Doctor pane (D64.T25) ─────────────────────────────────────────────────────

/// Settings pane showing project health and active release plans (D64.T25).
///
/// Uses [ReleasePlanTile] from clawd_ui to display release findings
/// (scope: "release") and [DoctorBadge] style score for the current project.
class _DoctorPane extends ConsumerWidget {
  const _DoctorPane();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final activeProject = ref.watch(activeProjectProvider);
    final projectPath = activeProject?.rootPath;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const _Header(
          title: 'Doctor',
          subtitle: 'Project health checks and release plan status',
        ),
        const SizedBox(height: 24),

        if (projectPath == null)
          const Padding(
            padding: EdgeInsets.only(top: 16),
            child: Text(
              'No active project selected.',
              style: TextStyle(fontSize: 13, color: Colors.white38),
            ),
          )
        else ...[
          // Score + scan button row
          _DoctorScoreRow(projectPath: projectPath),
          const SizedBox(height: 24),
          const Divider(),
          const SizedBox(height: 16),

          // Release plans section
          const _Label('Release Plans'),
          const SizedBox(height: 4),
          const Text(
            'Active release plans from .claude/planning/. Approve to unblock git tagging.',
            style: TextStyle(fontSize: 11, color: Colors.white38),
          ),
          const SizedBox(height: 12),
          ReleasePlanTile(projectPath: projectPath),
        ],
      ],
    );
  }
}

class _DoctorScoreRow extends ConsumerWidget {
  const _DoctorScoreRow({required this.projectPath});

  final String projectPath;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final scanAsync = ref.watch(doctorProvider(projectPath));
    final score = scanAsync.valueOrNull?.score;
    final isLoading = scanAsync.isLoading;

    return Row(
      children: [
        DoctorBadge(projectPath: projectPath),
        const SizedBox(width: 12),
        Expanded(
          child: Text(
            score != null
                ? 'Health score: $score / 100'
                : 'Not yet scanned',
            style: const TextStyle(fontSize: 13, color: Colors.white70),
          ),
        ),
        isLoading
            ? const SizedBox(
                width: 20,
                height: 20,
                child: CircularProgressIndicator(strokeWidth: 2),
              )
            : OutlinedButton.icon(
                onPressed: () =>
                    ref.read(doctorProvider(projectPath).notifier).scan(),
                icon: const Icon(Icons.search, size: 14),
                label: const Text('Scan Now'),
                style: OutlinedButton.styleFrom(
                  foregroundColor: Colors.white70,
                  side: const BorderSide(color: ClawdTheme.surfaceBorder),
                  padding:
                      const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
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
