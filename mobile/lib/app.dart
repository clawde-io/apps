import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde_mobile/features/sessions/sessions_screen.dart';
import 'package:clawde_mobile/features/session_detail/session_detail_screen.dart';
import 'package:clawde_mobile/features/hosts/hosts_screen.dart';
import 'package:clawde_mobile/features/hosts/host_provider.dart';
import 'package:clawde_mobile/features/settings/settings_screen.dart';
import 'package:clawde_mobile/features/dashboard/agent_dashboard_screen.dart';
import 'package:clawde_mobile/features/offline/offline_screen.dart';
import 'package:clawde_mobile/services/notification_service.dart';

final _router = GoRouter(
  routes: [
    ShellRoute(
      builder: (context, state, child) => _MobileShell(child: child),
      routes: [
        GoRoute(
          path: '/',
          builder: (_, __) => const SessionsScreen(),
        ),
        GoRoute(
          path: '/session/:id',
          builder: (context, state) => SessionDetailScreen(
            sessionId: state.pathParameters['id']!,
          ),
        ),
        GoRoute(
          path: '/hosts',
          builder: (_, __) => const HostsScreen(),
        ),
        GoRoute(
          path: '/tasks',
          builder: (_, __) => const AgentDashboardScreen(),
        ),
        GoRoute(
          path: '/settings',
          builder: (_, __) => const SettingsScreen(),
        ),
      ],
    ),
  ],
);

class ClawDEMobileApp extends StatelessWidget {
  const ClawDEMobileApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp.router(
      title: 'ClawDE',
      debugShowCheckedModeBanner: false,
      theme: ClawdTheme.dark(),
      routerConfig: _router,
    );
  }
}

/// Bottom-tab shell for mobile navigation.
/// Listens for daemon push events and surfaces update banners / snackbars.
/// Also handles notification deep links (MN-04) and tab badges (MN-05).
class _MobileShell extends ConsumerStatefulWidget {
  const _MobileShell({required this.child});
  final Widget child;

  @override
  ConsumerState<_MobileShell> createState() => _MobileShellState();
}

class _MobileShellState extends ConsumerState<_MobileShell> {
  bool _updateAvailable = false;

  static const _kLastSessionKey = 'last_active_session_id';

  @override
  void initState() {
    super.initState();
    // MN-01: Request permissions after first frame.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      NotificationService.instance.requestPermissions();
      // FA-C2: Restore active daemon host so the right URL is used from first connect.
      _restoreActiveHost();
      // SH-05: Restore last active session.
      _restoreLastSession();
    });
    // MN-04: Wire deep-link handler so tapping a notification navigates to session.
    NotificationService.instance.onNotificationTapped = (sessionId) {
      _router.go('/session/$sessionId');
      _persistLastSession(sessionId);
    };
    // React to daemon push events. Registered once in initState so the
    // listener is not re-subscribed on every rebuild.
    ref.listenManual(daemonPushEventsProvider, (_, next) {
      next.whenData((event) {
        final method = event['method'] as String?;
        final params = event['params'] as Map<String, dynamic>?;
        switch (method) {
          case 'daemon.updateAvailable':
            if (!_updateAvailable) {
              setState(() => _updateAvailable = true);
            }
          case 'session.accountLimited':
            final sessionId = params?['sessionId'] as String?;
            _showAccountLimitedSnackbar(sessionId);
          case 'session.accountSwitched':
            _showAccountSwitchedSnackbar(params?['to'] as String?);

          // MN-02: Tool call arrived — notify if in background.
          case 'session.toolCallCreated':
            final sessionId = params?['sessionId'] as String?;
            if (sessionId != null) {
              final name = _sessionName(sessionId) ?? 'Session';
              final count = ref.read(pendingToolCallCountProvider(sessionId));
              NotificationService.instance
                  .showToolCallPending(sessionId, name, count + 1);
            }

          // MN-03: Session status changed — notify on error or complete.
          case 'session.statusChanged':
            final sessionId = params?['sessionId'] as String?;
            final status = params?['status'] as String?;
            if (sessionId != null && status != null) {
              final name = _sessionName(sessionId) ?? 'Session';
              if (status == 'error') {
                NotificationService.instance
                    .showSessionError(sessionId, name, 'AI session failed.');
              } else if (status == 'completed') {
                NotificationService.instance
                    .showSessionComplete(sessionId, name);
              }
            }
        }
      });
    });
  }

  /// FA-C2: Restore the previously active daemon host on cold start.
  /// Reads the persisted host ID, finds the host, and reconnects to it.
  Future<void> _restoreActiveHost() async {
    final hostId = await ref.read(persistedActiveHostProvider.future);
    if (hostId == null || !mounted) return;
    final hosts = await ref.read(hostListProvider.future);
    final host = hosts.where((h) => h.id == hostId).firstOrNull;
    if (host != null && mounted) {
      ref.read(activeHostIdProvider.notifier).state = host.id;
      await ref.read(settingsProvider.notifier).setDaemonUrl(host.url);
    }
  }

  /// SH-05: Navigate to the last active session on app restart.
  Future<void> _restoreLastSession() async {
    final prefs = await SharedPreferences.getInstance();
    final id = prefs.getString(_kLastSessionKey);
    if (id != null && mounted) {
      _router.go('/session/$id');
    }
  }

  /// SH-05: Save the active session ID so it can be restored on next launch.
  Future<void> _persistLastSession(String sessionId) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_kLastSessionKey, sessionId);
  }

  String? _sessionName(String sessionId) {
    final sessions = ref.read(sessionListProvider).valueOrNull;
    final session = sessions?.where((s) => s.id == sessionId).firstOrNull;
    return session?.repoPath.split('/').last;
  }

  @override
  Widget build(BuildContext context) {
    // MN-05: total pending tool calls for badge on Sessions tab.
    final sessions = ref.watch(sessionListProvider).valueOrNull ?? [];
    final totalPending = sessions.fold<int>(
      0,
      (sum, s) => sum + ref.watch(pendingToolCallCountProvider(s.id)),
    );

    final location = GoRouterState.of(context).uri.toString();

    return Scaffold(
      body: Column(
        children: [
          if (_updateAvailable)
            _UpdateBanner(onDismiss: () {
              setState(() => _updateAvailable = false);
            }),
          const OfflineBanner(),
          Expanded(child: widget.child),
        ],
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: _indexFor(location),
        onDestinationSelected: (i) => _navigate(context, i),
        destinations: [
          NavigationDestination(
            // MN-05: Badge shows pending tool call count.
            icon: totalPending > 0
                ? Badge(
                    label: Text('$totalPending'),
                    child: const Icon(Icons.chat_bubble_outline),
                  )
                : const Icon(Icons.chat_bubble_outline),
            selectedIcon: totalPending > 0
                ? Badge(
                    label: Text('$totalPending'),
                    child: const Icon(Icons.chat_bubble),
                  )
                : const Icon(Icons.chat_bubble),
            label: 'Sessions',
          ),
          const NavigationDestination(
            icon: Icon(Icons.view_kanban_outlined),
            selectedIcon: Icon(Icons.view_kanban),
            label: 'Tasks',
          ),
          const NavigationDestination(
            icon: Icon(Icons.wifi_outlined),
            selectedIcon: Icon(Icons.wifi),
            label: 'Hosts',
          ),
          const NavigationDestination(
            icon: Icon(Icons.settings_outlined),
            selectedIcon: Icon(Icons.settings),
            label: 'Settings',
          ),
        ],
      ),
    );
  }

  void _showAccountLimitedSnackbar(String? sessionId) {
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: const Text('Account rate-limited. Tap to switch manually.'),
        backgroundColor: ClawdTheme.warning,
        action: SnackBarAction(
          label: 'OK',
          textColor: Colors.white,
          onPressed: () {},
        ),
      ),
    );
  }

  void _showAccountSwitchedSnackbar(String? toAccount) {
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(
        content: Text('Switched to next account automatically.'),
        backgroundColor: ClawdTheme.success,
      ),
    );
  }

  int _indexFor(String location) {
    if (location.startsWith('/tasks')) return 1;
    if (location.startsWith('/hosts')) return 2;
    if (location.startsWith('/settings')) return 3;
    return 0;
  }

  void _navigate(BuildContext context, int index) {
    switch (index) {
      case 0:
        context.go('/');
      case 1:
        context.go('/tasks');
      case 2:
        context.go('/hosts');
      case 3:
        context.go('/settings');
    }
  }
}

class _UpdateBanner extends StatelessWidget {
  const _UpdateBanner({required this.onDismiss});
  final VoidCallback onDismiss;

  @override
  Widget build(BuildContext context) {
    return Container(
      color: ClawdTheme.claw,
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      child: Row(
        children: [
          const Icon(Icons.system_update, size: 16, color: Colors.white),
          const SizedBox(width: 8),
          const Expanded(
            child: Text(
              'A new version of ClawDE is available.',
              style: TextStyle(fontSize: 12, color: Colors.white),
            ),
          ),
          IconButton(
            icon: const Icon(Icons.close, size: 16, color: Colors.white70),
            onPressed: onDismiss,
            padding: EdgeInsets.zero,
            constraints: const BoxConstraints(),
          ),
        ],
      ),
    );
  }
}
