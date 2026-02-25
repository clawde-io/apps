import 'package:go_router/go_router.dart';
import 'package:clawde/widgets/app_shell.dart';
import 'package:clawde/features/chat/chat_screen.dart';
import 'package:clawde/features/sessions/sessions_screen.dart';
import 'package:clawde/features/files/files_screen.dart';
import 'package:clawde/features/git/git_screen.dart';
import 'package:clawde/features/settings/settings_screen.dart';
import 'package:clawde/features/dashboard/agent_dashboard_screen.dart';
import 'package:clawde/features/search/search_screen.dart';
import 'package:clawde/features/packs/packs_screen.dart';
import 'package:clawde/features/usage/usage_dashboard_screen.dart';

const routeChat = '/chat';
const routeSessions = '/sessions';
const routeFiles = '/files';
const routeGit = '/git';
const routeDashboard = '/dashboard';
const routeSearch = '/search';
const routePacks = '/packs';
const routeSettings = '/settings';
const routeUsage = '/usage';

final appRouter = GoRouter(
  initialLocation: routeChat,
  routes: [
    ShellRoute(
      builder: (context, state, child) => AppShell(child: child),
      routes: [
        GoRoute(path: routeChat, builder: (_, __) => const ChatScreen()),
        GoRoute(path: routeSessions, builder: (_, __) => const SessionsScreen()),
        GoRoute(path: routeFiles, builder: (_, __) => const FilesScreen()),
        GoRoute(path: routeGit, builder: (_, __) => const GitScreen()),
        GoRoute(path: routeDashboard, builder: (_, __) => const AgentDashboardScreen()),
        GoRoute(path: routeSearch, builder: (_, __) => const SearchScreen()),
        GoRoute(path: routePacks, builder: (_, __) => const PacksScreen()),
        GoRoute(path: routeSettings, builder: (_, __) => const SettingsScreen()),
        GoRoute(path: routeUsage, builder: (_, __) => const UsageDashboardScreen()),
      ],
    ),
  ],
);
