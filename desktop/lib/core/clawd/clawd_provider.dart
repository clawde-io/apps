import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'clawd_client.dart';

final clawdClientProvider = Provider<ClawdClient>((ref) {
  final client = ClawdClient();
  ref.onDispose(client.disconnect);
  return client;
});

// Connection state
final clawdConnectedProvider = StateProvider<bool>((ref) => false);
