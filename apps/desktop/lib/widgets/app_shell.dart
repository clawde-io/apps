import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde/router.dart';
import 'package:clawde/widgets/status_bar.dart';
import 'package:clawde/widgets/update_banner.dart';
import 'package:clawde/widgets/grace_period_banner.dart';
import 'package:clawde/features/settings/relay_status_banner.dart';
import 'package:clawde/features/projects/project_selector_header.dart';
import 'package:clawde/features/repo/repo_context_provider.dart';

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
    routeDoctor,
    routeInstructions,
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

    // Watch doctor scan result for warning badge.
    final repoPath = ref.watch(effectiveRepoPathProvider);
    final doctorResult = repoPath == null
        ? null
        : ref.watch(doctorProvider(repoPath)).valueOrNull;
    final hasDoctorWarning =
        doctorResult != null && !doctorResult.isHealthy;

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
            leading: const Padding(
              padding: EdgeInsets.fromLTRB(4, 8, 4, 4),
              child: ProjectSelectorHeader(),
            ),
            destinations: [
              const NavigationRailDestination(
                icon: Icon(Icons.chat_bubble_outline),
                selectedIcon: Icon(Icons.chat_bubble),
                label: Text('Chat'),
              ),
              const NavigationRailDestination(
                icon: Icon(Icons.layers_outlined),
                selectedIcon: Icon(Icons.layers),
                label: Text('Sessions'),
              ),
              const NavigationRailDestination(
                icon: Icon(Icons.folder_outlined),
                selectedIcon: Icon(Icons.folder),
                label: Text('Files'),
              ),
              const NavigationRailDestination(
                icon: Icon(Icons.account_tree_outlined),
                selectedIcon: Icon(Icons.account_tree),
                label: Text('Git'),
              ),
              const NavigationRailDestination(
                icon: Icon(Icons.view_kanban_outlined),
                selectedIcon: Icon(Icons.view_kanban),
                label: Text('Tasks'),
              ),
              const NavigationRailDestination(
                icon: Icon(Icons.search_outlined),
                selectedIcon: Icon(Icons.search),
                label: Text('Search'),
              ),
              const NavigationRailDestination(
                icon: Icon(Icons.extension_outlined),
                selectedIcon: Icon(Icons.extension),
                label: Text('Packs'),
              ),
              NavigationRailDestination(
                icon: _DoctorIcon(hasWarning: hasDoctorWarning, selected: false),
                selectedIcon: _DoctorIcon(hasWarning: hasDoctorWarning, selected: true),
                label: const Text('Doctor'),
              ),
              const NavigationRailDestination(
                icon: Icon(Icons.rule_outlined),
                selectedIcon: Icon(Icons.rule),
                label: Text('Instructions'),
              ),
              const NavigationRailDestination(
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
          Expanded(
            child: RelayStatusBanner(
              child: GracePeriodBanner(
                child: UpdateBanner(child: widget.child),
              ),
            ),
          ),
        ],
      ),
      bottomNavigationBar: const StatusBar(),
    );
  }
}

// ─── Doctor nav icon with optional warning badge ──────────────────────────────

class _DoctorIcon extends StatelessWidget {
  const _DoctorIcon({required this.hasWarning, required this.selected});

  final bool hasWarning;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    return Stack(
      clipBehavior: Clip.none,
      children: [
        Icon(
          selected
              ? Icons.health_and_safety
              : Icons.health_and_safety_outlined,
        ),
        if (hasWarning)
          Positioned(
            top: -2,
            right: -4,
            child: Container(
              width: 8,
              height: 8,
              decoration: const BoxDecoration(
                color: Color(0xFFf97316),
                shape: BoxShape.circle,
              ),
            ),
          ),
      ],
    );
  }
}
