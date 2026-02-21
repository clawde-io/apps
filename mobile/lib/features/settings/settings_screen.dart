import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';

class SettingsScreen extends ConsumerWidget {
  const SettingsScreen({super.key});

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
            subtitle: const Text('clawd on localhost:4300'),
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
          const ListTile(
            title: Text('ClawDE'),
            subtitle: Text('v0.1.0'),
          ),
          ListTile(
            title: const Text('Source'),
            subtitle: const Text('github.com/clawde-io/apps'),
            onTap: () {/* open URL */},
          ),
        ],
      ),
    );
  }
}
