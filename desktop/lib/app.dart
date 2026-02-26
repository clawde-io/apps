import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawde/router.dart';
import 'package:clawde/theme/app_theme.dart';
import 'package:clawde/services/snackbar_service.dart';
import 'package:clawde/services/tray_service.dart';
import 'package:clawde/features/command_palette/command_palette.dart';
import 'package:clawde/features/chat/widgets/new_session_dialog.dart';

class ClawDEApp extends ConsumerStatefulWidget {
  const ClawDEApp({super.key});

  @override
  ConsumerState<ClawDEApp> createState() => _ClawDEAppState();
}

class _ClawDEAppState extends ConsumerState<ClawDEApp> {
  @override
  void initState() {
    super.initState();
    // Wire daemon state changes to the system tray icon. Registered once in
    // initState so the listener is never duplicated on rebuild.
    ref.listenManual<DaemonState>(daemonProvider, (_, next) {
      final trayState = switch (next.status) {
        DaemonStatus.connected => TrayIconState.connected,
        DaemonStatus.error => TrayIconState.error,
        _ => TrayIconState.running,
      };
      TrayService.instance.setState(trayState);
    });
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp.router(
      title: 'ClawDE',
      theme: AppTheme.dark(),
      routerConfig: appRouter,
      scaffoldMessengerKey: scaffoldMessengerKey,
      debugShowCheckedModeBanner: false,
      localizationsDelegates: const [
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      supportedLocales: const [
        Locale('en'),
        Locale('fr'),
        Locale('ja'),
      ],
      builder: (context, child) {
        return _GlobalShortcuts(
          ref: ref,
          child: child ?? const SizedBox.shrink(),
        );
      },
    );
  }
}

/// Wraps the entire app to intercept global keyboard shortcuts.
///
/// Uses [CallbackShortcuts] from Flutter's Shortcuts/Actions framework.
/// Shortcuts are platform-aware: Meta (Cmd) on macOS, Control on others.
class _GlobalShortcuts extends StatelessWidget {
  const _GlobalShortcuts({required this.ref, required this.child});

  final WidgetRef ref;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final isMac = Theme.of(context).platform == TargetPlatform.macOS;

    // Helper to create a SingleActivator with the platform modifier key.
    SingleActivator meta(LogicalKeyboardKey key, {bool shift = false}) =>
        SingleActivator(key, meta: isMac, control: !isMac, shift: shift);

    return CallbackShortcuts(
      bindings: {
        // ── Session shortcuts ──────────────────────────────────────────────
        // Cmd+N: New session
        meta(LogicalKeyboardKey.keyN): () {
          final ctx = appRouter.routerDelegate.navigatorKey.currentContext;
          if (ctx != null) {
            showDialog<void>(
              context: ctx,
              builder: (_) => const NewSessionDialog(),
            );
          }
        },

        // Cmd+W: Close current session
        meta(LogicalKeyboardKey.keyW): () {
          final sessionId = ref.read(activeSessionIdProvider);
          if (sessionId != null) {
            ref.read(sessionListProvider.notifier).close(sessionId);
            ref.read(activeSessionIdProvider.notifier).state = null;
          }
        },

        // Cmd+P: Pause current session
        meta(LogicalKeyboardKey.keyP): () {
          final sessionId = ref.read(activeSessionIdProvider);
          if (sessionId != null) {
            ref.read(sessionListProvider.notifier).pause(sessionId);
          }
        },

        // Cmd+.: Cancel current generation
        meta(LogicalKeyboardKey.period): () {
          final sessionId = ref.read(activeSessionIdProvider);
          if (sessionId != null) {
            ref.read(sessionListProvider.notifier).cancel(sessionId);
          }
        },

        // Cmd+[: Previous session
        meta(LogicalKeyboardKey.bracketLeft): () {
          _switchSession(ref, -1);
        },

        // Cmd+]: Next session
        meta(LogicalKeyboardKey.bracketRight): () {
          _switchSession(ref, 1);
        },

        // ── Navigation tab shortcuts ───────────────────────────────────────
        // Cmd+1 through Cmd+8 for nav tabs
        meta(LogicalKeyboardKey.digit1): () => _goToTab(0),
        meta(LogicalKeyboardKey.digit2): () => _goToTab(1),
        meta(LogicalKeyboardKey.digit3): () => _goToTab(2),
        meta(LogicalKeyboardKey.digit4): () => _goToTab(3),
        meta(LogicalKeyboardKey.digit5): () => _goToTab(4),
        meta(LogicalKeyboardKey.digit6): () => _goToTab(5),
        meta(LogicalKeyboardKey.digit7): () => _goToTab(6),
        meta(LogicalKeyboardKey.digit8): () => _goToTab(7),

        // ── Command palette ────────────────────────────────────────────────
        // Cmd+Shift+P: Command palette
        meta(LogicalKeyboardKey.keyP, shift: true): () {
          final ctx = appRouter.routerDelegate.navigatorKey.currentContext;
          if (ctx != null) {
            showCommandPalette(ctx, ref);
          }
        },

        // ── Search ─────────────────────────────────────────────────────────
        // Cmd+K: Open search
        meta(LogicalKeyboardKey.keyK): () => appRouter.go(routeSearch),

        // ── Escape ─────────────────────────────────────────────────────────
        // Escape: Cancel generation (if session is running)
        const SingleActivator(LogicalKeyboardKey.escape): () {
          final sessionId = ref.read(activeSessionIdProvider);
          if (sessionId != null) {
            final session = ref.read(activeSessionProvider);
            if (session?.status == SessionStatus.running) {
              ref.read(sessionListProvider.notifier).cancel(sessionId);
            }
          }
        },
      },
      child: Focus(
        autofocus: true,
        child: child,
      ),
    );
  }

  /// Navigate to a tab by index. Matches the _routes order in AppShell.
  static void _goToTab(int index) {
    const routes = [
      routeChat,
      routeSessions,
      routeFiles,
      routeGit,
      routeDashboard,
      routeSearch,
      routePacks,
      routeSettings,
    ];
    if (index >= 0 && index < routes.length) {
      appRouter.go(routes[index]);
    }
  }

  /// Switch to the previous or next session in the list.
  static void _switchSession(WidgetRef ref, int delta) {
    final sessions = ref.read(sessionListProvider).valueOrNull ?? [];
    if (sessions.isEmpty) return;

    final currentId = ref.read(activeSessionIdProvider);
    int currentIndex = -1;
    if (currentId != null) {
      currentIndex = sessions.indexWhere((s) => s.id == currentId);
    }

    final newIndex = (currentIndex + delta).clamp(0, sessions.length - 1);
    ref.read(activeSessionIdProvider.notifier).state = sessions[newIndex].id;
    appRouter.go(routeChat);
  }
}
