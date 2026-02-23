import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde/router.dart';
import 'package:clawde/widgets/status_bar.dart';
import 'package:clawde/widgets/update_banner.dart';

class AppShell extends ConsumerStatefulWidget {
  const AppShell({super.key, required this.child});

  final Widget child;

  @override
  ConsumerState<AppShell> createState() => _AppShellState();
}

class _AppShellState extends ConsumerState<AppShell> {
  static const _routes = [
    routeChat,
    routeSessions,
    routeFiles,
    routeGit,
    routeDashboard,
    routeSearch,
    routePacks,
    routeSettings,
  ];

  int _indexFromRoute(String location) {
    for (int i = 0; i < _routes.length; i++) {
      if (location.startsWith(_routes[i])) return i;
    }
    return 0;
  }

  @override
  Widget build(BuildContext context) {
    final location = GoRouterState.of(context).uri.toString();
    final selectedIndex = _indexFromRoute(location);

    return Scaffold(
      body: Row(
        children: [
          NavigationRail(
            selectedIndex: selectedIndex,
            onDestinationSelected: (i) => context.go(_routes[i]),
            labelType: NavigationRailLabelType.all,
            backgroundColor: ClawdTheme.surfaceElevated,
            indicatorColor: ClawdTheme.claw.withValues(alpha: 0.2),
            selectedIconTheme: const IconThemeData(color: ClawdTheme.clawLight),
            selectedLabelTextStyle: const TextStyle(
              color: ClawdTheme.clawLight,
              fontSize: 12,
              fontWeight: FontWeight.w600,
            ),
            unselectedIconTheme:
                IconThemeData(color: Colors.white.withValues(alpha: 0.5)),
            unselectedLabelTextStyle: TextStyle(
              color: Colors.white.withValues(alpha: 0.5),
              fontSize: 12,
            ),
            destinations: const [
              NavigationRailDestination(
                icon: Icon(Icons.chat_bubble_outline),
                selectedIcon: Icon(Icons.chat_bubble),
                label: Text('Chat'),
              ),
              NavigationRailDestination(
                icon: Icon(Icons.layers_outlined),
                selectedIcon: Icon(Icons.layers),
                label: Text('Sessions'),
              ),
              NavigationRailDestination(
                icon: Icon(Icons.folder_outlined),
                selectedIcon: Icon(Icons.folder),
                label: Text('Files'),
              ),
              NavigationRailDestination(
                icon: Icon(Icons.account_tree_outlined),
                selectedIcon: Icon(Icons.account_tree),
                label: Text('Git'),
              ),
              NavigationRailDestination(
                icon: Icon(Icons.view_kanban_outlined),
                selectedIcon: Icon(Icons.view_kanban),
                label: Text('Tasks'),
              ),
              NavigationRailDestination(
                icon: Icon(Icons.search_outlined),
                selectedIcon: Icon(Icons.search),
                label: Text('Search'),
              ),
              NavigationRailDestination(
                icon: Icon(Icons.extension_outlined),
                selectedIcon: Icon(Icons.extension),
                label: Text('Packs'),
              ),
              NavigationRailDestination(
                icon: Icon(Icons.settings_outlined),
                selectedIcon: Icon(Icons.settings),
                label: Text('Settings'),
              ),
            ],
            trailing: const Expanded(
              child: Align(
                alignment: Alignment.bottomCenter,
                child: Padding(
                  padding: EdgeInsets.only(bottom: 12),
                  child: ConnectionStatusIndicator(),
                ),
              ),
            ),
          ),
          const VerticalDivider(thickness: 1, width: 1),
          Expanded(child: UpdateBanner(child: widget.child)),
        ],
      ),
      bottomNavigationBar: const StatusBar(),
    );
  }
}
