import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:clawd_proto/clawd_proto.dart';

const _kDaemonUrl = 'settings.daemon_url';
const _kDefaultProvider = 'settings.default_provider';
const _kAutoReconnect = 'settings.auto_reconnect';
const _kTheme = 'settings.theme';

const _defaultDaemonUrl = 'ws://127.0.0.1:4300';
const _defaultTheme = 'dark';

class AppSettings {
  final String daemonUrl;
  final ProviderType defaultProvider;
  final bool autoReconnect;
  final String theme;

  const AppSettings({
    this.daemonUrl = _defaultDaemonUrl,
    this.defaultProvider = ProviderType.claude,
    this.autoReconnect = true,
    this.theme = _defaultTheme,
  });

  AppSettings copyWith({
    String? daemonUrl,
    ProviderType? defaultProvider,
    bool? autoReconnect,
    String? theme,
  }) =>
      AppSettings(
        daemonUrl: daemonUrl ?? this.daemonUrl,
        defaultProvider: defaultProvider ?? this.defaultProvider,
        autoReconnect: autoReconnect ?? this.autoReconnect,
        theme: theme ?? this.theme,
      );
}

class SettingsNotifier extends AsyncNotifier<AppSettings> {
  @override
  Future<AppSettings> build() async {
    final prefs = await SharedPreferences.getInstance();
    return AppSettings(
      daemonUrl: prefs.getString(_kDaemonUrl) ?? _defaultDaemonUrl,
      defaultProvider: () {
        final stored = prefs.getString(_kDefaultProvider) ?? ProviderType.claude.name;
        try {
          return ProviderType.values.byName(stored);
        } catch (_) {
          return ProviderType.claude;
        }
      }(),
      autoReconnect: prefs.getBool(_kAutoReconnect) ?? true,
      theme: prefs.getString(_kTheme) ?? _defaultTheme,
    );
  }

  Future<void> setDaemonUrl(String url) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_kDaemonUrl, url);
    state = AsyncValue.data((state.valueOrNull ?? const AppSettings()).copyWith(daemonUrl: url));
  }

  Future<void> setDefaultProvider(ProviderType provider) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_kDefaultProvider, provider.name);
    state = AsyncValue.data((state.valueOrNull ?? const AppSettings()).copyWith(defaultProvider: provider));
  }

  Future<void> setAutoReconnect(bool value) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool(_kAutoReconnect, value);
    state = AsyncValue.data((state.valueOrNull ?? const AppSettings()).copyWith(autoReconnect: value));
  }

  Future<void> setTheme(String theme) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_kTheme, theme);
    state = AsyncValue.data((state.valueOrNull ?? const AppSettings()).copyWith(theme: theme));
  }
}

final settingsProvider = AsyncNotifierProvider<SettingsNotifier, AppSettings>(
  SettingsNotifier.new,
);
