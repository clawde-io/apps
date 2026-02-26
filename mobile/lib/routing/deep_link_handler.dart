// deep_link_handler.dart — Deep link parser and navigator (Sprint RR DL.2-3).
//
// Handles:
//   clawde://session/{id}   → opens session detail screen
//   clawde://task/{id}      → opens session that owns the task, scrolls to task
//
// Uses `app_links` package which works for both custom schemes (iOS + Android)
// and HTTPS universal links / App Links.

import 'dart:async';

import 'package:app_links/app_links.dart';
import 'package:flutter/foundation.dart';
import 'package:go_router/go_router.dart';

// ─── Deep link destination ─────────────────────────────────────────────────

sealed class DeepLinkDestination {
  const DeepLinkDestination();
}

class SessionDeepLink extends DeepLinkDestination {
  const SessionDeepLink(this.sessionId);
  final String sessionId;
}

class TaskDeepLink extends DeepLinkDestination {
  const TaskDeepLink(this.taskId);
  final String taskId;
}

// ─── Parser ───────────────────────────────────────────────────────────────────

DeepLinkDestination? parseDeepLink(Uri uri) {
  if (uri.scheme != 'clawde') return null;

  final host = uri.host;
  final segments = uri.pathSegments;

  if (host == 'session' && segments.isEmpty) return null;
  if (host == 'session') return SessionDeepLink(segments.first);
  if (host == 'task' && segments.isNotEmpty) return TaskDeepLink(segments.first);

  // Also handle clawde:///session/{id} format (path-based)
  if (segments.length >= 2) {
    if (segments[0] == 'session') return SessionDeepLink(segments[1]);
    if (segments[0] == 'task') return TaskDeepLink(segments[1]);
  }

  return null;
}

// ─── Handler service ──────────────────────────────────────────────────────────

class DeepLinkHandler {
  DeepLinkHandler({required GoRouter router}) : _router = router;

  final GoRouter _router;
  final _appLinks = AppLinks();
  StreamSubscription<Uri>? _sub;

  /// Start listening for incoming deep links.
  Future<void> start() async {
    // Handle the initial link that opened the app (cold start)
    try {
      final initial = await _appLinks.getInitialLink();
      if (initial != null) _navigate(initial);
    } catch (e) {
      debugPrint('[DeepLinkHandler] initial link error: $e');
    }

    // Handle links received while the app is running (warm start)
    _sub = _appLinks.uriLinkStream.listen(
      _navigate,
      onError: (e) => debugPrint('[DeepLinkHandler] link stream error: $e'),
    );
  }

  void _navigate(Uri uri) {
    final dest = parseDeepLink(uri);
    if (dest == null) return;

    switch (dest) {
      case SessionDeepLink(:final sessionId):
        _router.push('/sessions/$sessionId');
      case TaskDeepLink(:final taskId):
        // Task IDs aren't directly routable — navigate to the tasks list
        // and let the screen highlight the task by ID.
        _router.push('/tasks?highlight=$taskId');
    }
  }

  void dispose() {
    _sub?.cancel();
  }
}
