import 'package:go_router/go_router.dart';
import 'package:clawde/widgets/app_shell.dart';
import 'package:clawde/features/chat/chat_screen.dart';
import 'package:clawde/features/sessions/sessions_screen.dart';
import 'package:clawde/features/files/files_screen.dart';
import 'package:clawde/features/git/git_screen.dart';
import 'package:clawde/features/settings/settings_screen.dart';

const routeChat = '/chat';
const routeSessions = '/sessions';
const routeFiles = '/files';
const routeGit = '/git';
const routeSettings = '/settings';

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
        GoRoute(path: routeSettings, builder: (_, __) => const SettingsScreen()),
      ],
    ),
  ],
);
