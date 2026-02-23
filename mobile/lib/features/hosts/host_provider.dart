import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:clawd_core/clawd_core.dart';

const _kHostsKey = 'hosts.list';
const _kActiveHostKey = 'hosts.active_id';

class DaemonHost {
  final String id;
  final String name;
  final String url;
  final DateTime? lastConnected;

  /// Pairing token received from the daemon during QR device trust pairing.
  /// Used for relay authentication on subsequent connections.
  final String? pairingToken;

  /// Whether this host was paired via QR code (device trust established).
  bool get isPaired => pairingToken != null && pairingToken!.isNotEmpty;

  const DaemonHost({
    required this.id,
    required this.name,
    required this.url,
    this.lastConnected,
    this.pairingToken,
  });

  DaemonHost copyWith({
    String? id,
    String? name,
    String? url,
    DateTime? lastConnected,
    String? pairingToken,
  }) =>
      DaemonHost(
        id: id ?? this.id,
        name: name ?? this.name,
        url: url ?? this.url,
        lastConnected: lastConnected ?? this.lastConnected,
        pairingToken: pairingToken ?? this.pairingToken,
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'name': name,
        'url': url,
        'lastConnected': lastConnected?.toIso8601String(),
        if (pairingToken != null) 'pairingToken': pairingToken,
      };

  factory DaemonHost.fromJson(Map<String, dynamic> json) => DaemonHost(
        id: json['id'] as String,
        name: json['name'] as String,
        url: json['url'] as String,
        lastConnected: json['lastConnected'] != null
            ? DateTime.tryParse(json['lastConnected'] as String)
            : null,
        pairingToken: json['pairingToken'] as String?,
      );
}

class HostListNotifier extends AsyncNotifier<List<DaemonHost>> {
  @override
  Future<List<DaemonHost>> build() async {
    final prefs = await SharedPreferences.getInstance();
    final raw = prefs.getString(_kHostsKey);
    if (raw == null) return [];
    final list = jsonDecode(raw) as List<dynamic>;
    return list
        .map((e) => DaemonHost.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  Future<void> add(DaemonHost host) async {
    final current = state.valueOrNull ?? [];
    final updated = [...current, host];
    await _persist(updated);
    state = AsyncValue.data(updated);
  }

  Future<void> remove(String id) async {
    final current = state.valueOrNull ?? [];
    final updated = current.where((h) => h.id != id).toList();
    await _persist(updated);
    state = AsyncValue.data(updated);
  }

  Future<void> updatePairingToken(String id, String token) async {
    final current = state.valueOrNull ?? [];
    final updated = current.map((h) {
      if (h.id == id) return h.copyWith(pairingToken: token);
      return h;
    }).toList();
    await _persist(updated);
    state = AsyncValue.data(updated);
  }

  Future<void> markConnected(String id) async {
    final current = state.valueOrNull ?? [];
    final updated = current.map((h) {
      if (h.id == id) return h.copyWith(lastConnected: DateTime.now());
      return h;
    }).toList();
    await _persist(updated);
    state = AsyncValue.data(updated);
  }

  Future<void> _persist(List<DaemonHost> hosts) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(
        _kHostsKey, jsonEncode(hosts.map((h) => h.toJson()).toList()));
  }
}

final hostListProvider =
    AsyncNotifierProvider<HostListNotifier, List<DaemonHost>>(
  HostListNotifier.new,
);

/// Tracks which host is currently active (by ID).
/// Persisted active host ID is loaded in [HostListNotifier.build].
final activeHostIdProvider = StateProvider<String?>((ref) => null);

/// Loads the persisted active host ID and exposes it reactively.
final persistedActiveHostProvider = FutureProvider<String?>((ref) async {
  final prefs = await SharedPreferences.getInstance();
  return prefs.getString(_kActiveHostKey);
});

/// Switches the active host: updates settings daemon URL and reconnects.
Future<void> switchHost(WidgetRef ref, DaemonHost host) async {
  final prefs = await SharedPreferences.getInstance();
  await prefs.setString(_kActiveHostKey, host.id);
  ref.read(activeHostIdProvider.notifier).state = host.id;
  await ref.read(settingsProvider.notifier).setDaemonUrl(host.url);
  await ref.read(hostListProvider.notifier).markConnected(host.id);
}
