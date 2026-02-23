import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';

final _appVersionProvider = FutureProvider<String>((ref) async {
  final info = await PackageInfo.fromPlatform();
  return 'v${info.version}';
});

class SettingsScreen extends ConsumerWidget {
  const SettingsScreen({super.key});

  static final _repoUrl = Uri.parse('https://github.com/clawde-io/apps');

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Scaffold(
      appBar: AppBar(title: const Text('Settings')),
      body: ListView(
        children: [
          const Padding(
            padding: EdgeInsets.fromLTRB(16, 16, 16, 8),
            child: Text(
              'Daemon',
              style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600),
            ),
          ),
          ListTile(
            title: const Text('Connection'),
            subtitle: Text(
              ref.watch(settingsProvider).valueOrNull?.daemonUrl ??
                  'ws://127.0.0.1:4300',
              style: const TextStyle(fontSize: 12),
            ),
            trailing: const ConnectionStatusIndicator(),
            onTap: () => ref.read(daemonProvider.notifier).reconnect(),
          ),
          const Divider(),
          const Padding(
            padding: EdgeInsets.fromLTRB(16, 16, 16, 8),
            child: Text(
              'About',
              style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600),
            ),
          ),
          ListTile(
            title: const Text('ClawDE'),
            subtitle: Text(
              ref.watch(_appVersionProvider).valueOrNull ?? 'v...',
            ),
          ),
          ListTile(
            title: const Text('Source'),
            subtitle: const Text('github.com/clawde-io/apps'),
            trailing: const Icon(Icons.open_in_new, size: 16),
            onTap: () => launchUrl(_repoUrl,
                mode: LaunchMode.externalApplication),
          ),
        ],
      ),
    );
  }
}
