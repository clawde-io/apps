import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'features/sessions/sessions_screen.dart';
import 'features/session_detail/session_detail_screen.dart';
import 'features/settings/settings_screen.dart';

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
class _MobileShell extends ConsumerWidget {
  const _MobileShell({required this.child});
  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final location = GoRouterState.of(context).uri.toString();

    return Scaffold(
      body: child,
      bottomNavigationBar: NavigationBar(
        selectedIndex: _indexFor(location),
        onDestinationSelected: (i) => _navigate(context, i),
        destinations: const [
          NavigationDestination(
            icon: Icon(Icons.chat_bubble_outline),
            selectedIcon: Icon(Icons.chat_bubble),
            label: 'Sessions',
          ),
          NavigationDestination(
            icon: Icon(Icons.settings_outlined),
            selectedIcon: Icon(Icons.settings),
            label: 'Settings',
          ),
        ],
      ),
    );
  }

  int _indexFor(String location) {
    if (location.startsWith('/settings')) return 1;
    return 0;
  }

  void _navigate(BuildContext context, int index) {
    switch (index) {
      case 0:
        context.go('/');
      case 1:
        context.go('/settings');
    }
  }
}
