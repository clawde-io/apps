// about_page.dart — Settings → About page.
//
// Sprint NN AG.9: Shows daemon version, air-gap status, and license validity.

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';

/// Settings → About
class AboutPage extends ConsumerWidget {
  const AboutPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final daemonInfo = ref.watch(daemonInfoProvider);
    final connectivity = ref.watch(connectivityProvider);

    return Scaffold(
      appBar: AppBar(title: const Text('About')),
      body: ListView(
        padding: const EdgeInsets.all(24),
        children: [
          _AboutSection(
            title: 'ClawDE',
            children: [
              _InfoRow(label: 'App version', value: _appVersion()),
              daemonInfo.when(
                data: (info) => Column(
                  children: [
                    _InfoRow(label: 'Daemon version', value: info['version'] as String? ?? 'unknown'),
                    _InfoRow(label: 'Daemon ID', value: _shortId(info['daemon_id'] as String? ?? '')),
                  ],
                ),
                loading: () => const _InfoRow(label: 'Daemon version', value: 'Loading…'),
                error: (_, __) => const _InfoRow(label: 'Daemon version', value: 'Unavailable'),
              ),
            ],
          ),
          const SizedBox(height: 24),
          connectivity.when(
            data: (status) => _ConnectionSection(status: status),
            loading: () => const SizedBox.shrink(),
            error: (_, __) => const SizedBox.shrink(),
          ),
          const SizedBox(height: 24),
          _AboutSection(
            title: 'Open Source',
            children: [
              const _InfoRow(label: 'License', value: 'MIT'),
              _LinkRow(
                label: 'Source code',
                url: 'https://github.com/clawde-io/apps',
              ),
              _LinkRow(
                label: 'Report an issue',
                url: 'https://github.com/clawde-io/apps/issues',
              ),
            ],
          ),
        ],
      ),
    );
  }

  String _appVersion() {
    // In production: read from package_info_plus
    return '0.2.x';
  }

  String _shortId(String id) {
    if (id.length > 12) return '${id.substring(0, 12)}…';
    return id;
  }
}

// ── Air-Gap + Connection Status ───────────────────────────────────────────────

class _ConnectionSection extends StatelessWidget {
  const _ConnectionSection({required this.status});
  final ConnectivityState status;

  @override
  Widget build(BuildContext context) {
    return _AboutSection(
      title: 'Connection',
      children: [
        _InfoRow(
          label: 'Mode',
          value: status.mode.toUpperCase(),
          valueColor: _modeColor(status.mode),
        ),
        if (status.airGap)
          _InfoRow(
            label: 'Air-Gap',
            value: 'Enabled',
            valueColor: Colors.amber,
          ),
        if (status.airGap) _LicenseValidityRow(status: status),
        if (status.rttMs > 0)
          _InfoRow(
            label: 'Latency',
            value: '${status.rttMs}ms',
            valueColor: status.degraded ? Colors.red : Colors.green,
          ),
      ],
    );
  }

  Color _modeColor(String mode) {
    switch (mode) {
      case 'local':
        return Colors.green;
      case 'direct':
        return Colors.blue;
      case 'relay':
        return Colors.amber;
      default:
        return Colors.red;
    }
  }
}

class _LicenseValidityRow extends StatelessWidget {
  const _LicenseValidityRow({required this.status});
  final ConnectivityState status;

  @override
  Widget build(BuildContext context) {
    // In production: read license expiry from daemon's daemon.info RPC
    return const _InfoRow(
      label: 'License',
      value: 'Enterprise — Valid',
      valueColor: Colors.green,
    );
  }
}

// ── Shared Widgets ────────────────────────────────────────────────────────────

class _AboutSection extends StatelessWidget {
  const _AboutSection({required this.title, required this.children});
  final String title;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          title,
          style: Theme.of(context)
              .textTheme
              .labelMedium
              ?.copyWith(color: Colors.white54, letterSpacing: 1.2),
        ),
        const SizedBox(height: 8),
        Container(
          decoration: BoxDecoration(
            border: Border.all(color: Colors.white12),
            borderRadius: BorderRadius.circular(8),
          ),
          child: Column(children: children),
        ),
      ],
    );
  }
}

class _InfoRow extends StatelessWidget {
  const _InfoRow({required this.label, required this.value, this.valueColor});
  final String label;
  final String value;
  final Color? valueColor;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Text(label, style: const TextStyle(color: Colors.white70)),
          Text(
            value,
            style: TextStyle(color: valueColor ?? Colors.white),
          ),
        ],
      ),
    );
  }
}

class _LinkRow extends StatelessWidget {
  const _LinkRow({required this.label, required this.url});
  final String label;
  final String url;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Text(label, style: const TextStyle(color: Colors.white70)),
          Text(
            url.replaceFirst('https://', ''),
            style: const TextStyle(color: Colors.blue),
          ),
        ],
      ),
    );
  }
}
