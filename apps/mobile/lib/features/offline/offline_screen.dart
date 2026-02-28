import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

// ── Session summary cache ────────────────────────────────────────────────────

const _kCachedSessionsKey = 'offline.cached_sessions';
const _kMaxCachedSessions = 10;

/// A lightweight session summary for offline display.
class CachedSessionSummary {
  final String id;
  final String repoPath;
  final String title;
  final String provider;
  final String status;
  final int messageCount;
  final DateTime cachedAt;

  const CachedSessionSummary({
    required this.id,
    required this.repoPath,
    required this.title,
    required this.provider,
    required this.status,
    required this.messageCount,
    required this.cachedAt,
  });

  Map<String, dynamic> toJson() => {
        'id': id,
        'repoPath': repoPath,
        'title': title,
        'provider': provider,
        'status': status,
        'messageCount': messageCount,
        'cachedAt': cachedAt.toIso8601String(),
      };

  factory CachedSessionSummary.fromJson(Map<String, dynamic> json) =>
      CachedSessionSummary(
        id: json['id'] as String,
        repoPath: json['repoPath'] as String,
        title: json['title'] as String? ?? '',
        provider: json['provider'] as String? ?? 'claude',
        status: json['status'] as String? ?? 'idle',
        messageCount: json['messageCount'] as int? ?? 0,
        cachedAt: DateTime.parse(json['cachedAt'] as String),
      );

  factory CachedSessionSummary.fromSession(Session session) =>
      CachedSessionSummary(
        id: session.id,
        repoPath: session.repoPath,
        title: session.title,
        provider: session.provider.name,
        status: session.status.name,
        messageCount: session.messageCount,
        cachedAt: DateTime.now(),
      );
}

/// Persists the last N session summaries for offline viewing.
class SessionCacheNotifier extends AsyncNotifier<List<CachedSessionSummary>> {
  @override
  Future<List<CachedSessionSummary>> build() async {
    // When sessions update and we are connected, cache them.
    ref.listen(sessionListProvider, (_, next) {
      next.whenData((sessions) {
        if (ref.read(daemonProvider).isConnected) {
          _cacheFromLive(sessions);
        }
      });
    });

    return _loadFromDisk();
  }

  // M14: Wrap the full disk-load in try/finally so that if SharedPreferences
  // itself throws, the provider still settles to a valid empty list rather
  // than staying in a loading state.
  Future<List<CachedSessionSummary>> _loadFromDisk() async {
    try {
      final prefs = await SharedPreferences.getInstance();
      final raw = prefs.getString(_kCachedSessionsKey);
      if (raw == null) return [];
      try {
        final list = jsonDecode(raw) as List<dynamic>;
        return list
            .map((e) =>
                CachedSessionSummary.fromJson(e as Map<String, dynamic>))
            .toList();
      } catch (_) {
        return [];
      }
    } catch (_) {
      // SharedPreferences unavailable or any unexpected error — return empty.
      return [];
    }
  }

  Future<void> _cacheFromLive(List<Session> sessions) async {
    final summaries = sessions
        .take(_kMaxCachedSessions)
        .map(CachedSessionSummary.fromSession)
        .toList();
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(
      _kCachedSessionsKey,
      jsonEncode(summaries.map((s) => s.toJson()).toList()),
    );
    state = AsyncValue.data(summaries);
  }
}

final sessionCacheProvider =
    AsyncNotifierProvider<SessionCacheNotifier, List<CachedSessionSummary>>(
  SessionCacheNotifier.new,
);

// ── Message queue provider ───────────────────────────────────────────────────

const _kMessageQueueKey = 'offline.message_queue';

/// A message that was composed while offline and is waiting to be sent.
class QueuedMessage {
  final String sessionId;
  final String content;
  final DateTime queuedAt;

  const QueuedMessage({
    required this.sessionId,
    required this.content,
    required this.queuedAt,
  });

  Map<String, dynamic> toJson() => {
        'sessionId': sessionId,
        'content': content,
        'queuedAt': queuedAt.toIso8601String(),
      };

  factory QueuedMessage.fromJson(Map<String, dynamic> json) => QueuedMessage(
        sessionId: json['sessionId'] as String,
        content: json['content'] as String,
        queuedAt: DateTime.parse(json['queuedAt'] as String),
      );
}

/// Manages the offline message queue. Messages are buffered here when the
/// daemon is unreachable and flushed automatically when the connection restores.
///
/// This is complementary to the per-session pending queue in
/// [MessageListNotifier] (clawd_core). The difference: this queue persists
/// across app restarts via SharedPreferences, while the core queue is in-memory
/// only and drains on reconnect within the same session.
class MessageQueueNotifier extends AsyncNotifier<List<QueuedMessage>> {
  @override
  Future<List<QueuedMessage>> build() async {
    // Flush when daemon reconnects.
    ref.listen(daemonProvider, (prev, next) {
      if (next.isConnected && prev?.isConnected != true) {
        _flush();
      }
    });

    return _loadFromDisk();
  }

  Future<List<QueuedMessage>> _loadFromDisk() async {
    final prefs = await SharedPreferences.getInstance();
    final raw = prefs.getString(_kMessageQueueKey);
    if (raw == null) return [];
    try {
      final list = jsonDecode(raw) as List<dynamic>;
      return list
          .map((e) => QueuedMessage.fromJson(e as Map<String, dynamic>))
          .toList();
    } catch (_) {
      return [];
    }
  }

  Future<void> _persistQueue(List<QueuedMessage> queue) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(
      _kMessageQueueKey,
      jsonEncode(queue.map((m) => m.toJson()).toList()),
    );
  }

  /// Enqueue a message for later delivery.
  Future<void> enqueue(String sessionId, String content) async {
    final msg = QueuedMessage(
      sessionId: sessionId,
      content: content,
      queuedAt: DateTime.now(),
    );
    final current = state.valueOrNull ?? [];
    final updated = [...current, msg];
    await _persistQueue(updated);
    state = AsyncValue.data(updated);
  }

  /// Flush queued messages through the daemon. Called on reconnect.
  Future<void> _flush() async {
    final queue = state.valueOrNull ?? [];
    if (queue.isEmpty) return;

    final remaining = <QueuedMessage>[];
    for (final msg in queue) {
      try {
        // Delegate to the per-session message provider which handles the RPC.
        await ref
            .read(messageListProvider(msg.sessionId).notifier)
            .send(msg.content);
      } catch (_) {
        // If send fails, keep the message in queue for next attempt.
        remaining.add(msg);
      }
    }
    await _persistQueue(remaining);
    state = AsyncValue.data(remaining);
  }

  /// Remove all queued messages (e.g. user clears the queue manually).
  Future<void> clearQueue() async {
    await _persistQueue([]);
    state = const AsyncValue.data([]);
  }
}

final messageQueueProvider =
    AsyncNotifierProvider<MessageQueueNotifier, List<QueuedMessage>>(
  MessageQueueNotifier.new,
);

// ── Offline banner widget ────────────────────────────────────────────────────

/// A persistent banner shown at the top of the app when the daemon is
/// disconnected. Integrates into the _MobileShell body column.
class OfflineBanner extends ConsumerWidget {
  const OfflineBanner({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final daemon = ref.watch(daemonProvider);
    final queueCount =
        ref.watch(messageQueueProvider).valueOrNull?.length ?? 0;

    // Only show when not connected.
    if (daemon.isConnected) return const SizedBox.shrink();

    final isConnecting = daemon.status == DaemonStatus.connecting;
    final attempt = daemon.reconnectAttempt;

    return Container(
      color: ClawdTheme.warning.withValues(alpha: 0.12),
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      child: Row(
        children: [
          Icon(
            isConnecting ? Icons.sync : Icons.cloud_off,
            size: 16,
            color: ClawdTheme.warning,
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  isConnecting
                      ? 'Reconnecting${attempt > 0 ? ' (attempt $attempt)' : ''}...'
                      : 'Daemon unreachable',
                  style: const TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                    color: ClawdTheme.warning,
                  ),
                ),
                if (queueCount > 0)
                  Text(
                    '$queueCount message${queueCount == 1 ? '' : 's'} queued',
                    style: TextStyle(
                      fontSize: 11,
                      color: ClawdTheme.warning.withValues(alpha: 0.7),
                    ),
                  ),
              ],
            ),
          ),
          if (!isConnecting)
            TextButton(
              onPressed: () =>
                  ref.read(daemonProvider.notifier).reconnect(),
              style: TextButton.styleFrom(
                foregroundColor: ClawdTheme.warning,
                padding:
                    const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
              ),
              child: const Text('Retry', style: TextStyle(fontSize: 12)),
            ),
        ],
      ),
    );
  }
}

// ── Offline screen (full page) ───────────────────────────────────────────────

/// Full-page "Daemon Unreachable" screen with animated retry and cached
/// session summaries. Shown when the connection fails and the user navigates
/// to a sessions view with no live data.
///
/// [onRetry] is called when the user taps the Retry button. If null, the
/// screen calls [DaemonNotifier.reconnect] directly.
class OfflineScreen extends ConsumerStatefulWidget {
  const OfflineScreen({super.key, this.onRetry});

  /// Optional callback invoked when the user taps Retry. When provided, this
  /// replaces the default behaviour of calling [DaemonNotifier.reconnect].
  final VoidCallback? onRetry;

  @override
  ConsumerState<OfflineScreen> createState() => _OfflineScreenState();
}

class _OfflineScreenState extends ConsumerState<OfflineScreen>
    with SingleTickerProviderStateMixin {
  late final AnimationController _pulseController;
  late final Animation<double> _pulseAnimation;
  bool _retrying = false;

  @override
  void initState() {
    super.initState();
    _pulseController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1200),
    )..repeat(reverse: true);
    _pulseAnimation = Tween<double>(begin: 0.8, end: 1.0).animate(
      CurvedAnimation(parent: _pulseController, curve: Curves.easeInOut),
    );
  }

  @override
  void dispose() {
    _pulseController.dispose();
    super.dispose();
  }

  // M12: Add if (mounted) guard before the initial setState and use a
  // finally block to guarantee _retrying is always reset even if the
  // reconnect call throws.
  Future<void> _retry() async {
    if (!mounted) return;
    setState(() => _retrying = true);
    try {
      if (widget.onRetry != null) {
        widget.onRetry!();
      } else {
        await ref.read(daemonProvider.notifier).reconnect();
      }
      // Wait a moment for the connection attempt to resolve.
      await Future<void>.delayed(const Duration(seconds: 2));
    } finally {
      // H8: Always clear the loading state, even if reconnect throws.
      if (mounted) setState(() => _retrying = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final daemon = ref.watch(daemonProvider);
    final cachedSessionsAsync = ref.watch(sessionCacheProvider);
    final queueCount =
        ref.watch(messageQueueProvider).valueOrNull?.length ?? 0;

    // If connected, show nothing (parent should navigate away).
    if (daemon.isConnected) {
      return const Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.check_circle, size: 48, color: ClawdTheme.success),
            SizedBox(height: 12),
            Text('Connected', style: TextStyle(fontSize: 16)),
          ],
        ),
      );
    }

    return Scaffold(
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 24),
          child: Column(
            children: [
              const Spacer(flex: 2),

              // Animated icon
              AnimatedBuilder(
                animation: _pulseAnimation,
                builder: (_, child) => Transform.scale(
                  scale: _pulseAnimation.value,
                  child: child,
                ),
                child: Container(
                  width: 96,
                  height: 96,
                  decoration: BoxDecoration(
                    color: ClawdTheme.warning.withValues(alpha: 0.12),
                    shape: BoxShape.circle,
                    border: Border.all(
                      color: ClawdTheme.warning.withValues(alpha: 0.3),
                      width: 2,
                    ),
                  ),
                  child: const Icon(
                    Icons.cloud_off,
                    size: 40,
                    color: ClawdTheme.warning,
                  ),
                ),
              ),
              const SizedBox(height: 24),

              const Text(
                'Daemon Unreachable',
                style: TextStyle(
                  fontSize: 22,
                  fontWeight: FontWeight.w700,
                  color: Colors.white,
                ),
              ),
              const SizedBox(height: 8),
              Text(
                daemon.errorMessage ??
                    'Unable to connect to the clawd daemon.\n'
                        'Check that it is running and reachable.',
                textAlign: TextAlign.center,
                style: TextStyle(
                  fontSize: 14,
                  color: Colors.white.withValues(alpha: 0.5),
                  height: 1.5,
                ),
              ),
              const SizedBox(height: 8),

              // Queue indicator
              if (queueCount > 0)
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
                  decoration: BoxDecoration(
                    color: ClawdTheme.info.withValues(alpha: 0.1),
                    borderRadius: BorderRadius.circular(8),
                    border: Border.all(
                      color: ClawdTheme.info.withValues(alpha: 0.3),
                    ),
                  ),
                  child: Text(
                    '$queueCount message${queueCount == 1 ? '' : 's'} '
                    'queued for delivery',
                    style: const TextStyle(
                      fontSize: 12,
                      color: ClawdTheme.info,
                    ),
                  ),
                ),

              const SizedBox(height: 24),

              // Retry button
              SizedBox(
                width: 180,
                height: 48,
                child: FilledButton.icon(
                  onPressed: _retrying ? null : _retry,
                  icon: _retrying
                      ? const SizedBox(
                          width: 18,
                          height: 18,
                          child: CircularProgressIndicator(
                            strokeWidth: 2,
                            color: Colors.white,
                          ),
                        )
                      : const Icon(Icons.refresh, size: 18),
                  label: Text(_retrying ? 'Connecting...' : 'Retry'),
                  style: FilledButton.styleFrom(
                    backgroundColor: ClawdTheme.claw,
                    foregroundColor: Colors.white,
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(12),
                    ),
                  ),
                ),
              ),

              const Spacer(flex: 1),

              // Cached sessions
              cachedSessionsAsync.when(
                loading: () => const SizedBox.shrink(),
                error: (_, __) => const SizedBox.shrink(),
                data: (cached) {
                  if (cached.isEmpty) return const SizedBox.shrink();
                  return Expanded(
                    flex: 3,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Padding(
                          padding: const EdgeInsets.only(bottom: 8),
                          child: Text(
                            'Last known sessions',
                            style: TextStyle(
                              fontSize: 12,
                              fontWeight: FontWeight.w600,
                              color: Colors.white.withValues(alpha: 0.5),
                            ),
                          ),
                        ),
                        Expanded(
                          child: ListView.separated(
                            itemCount: cached.length,
                            separatorBuilder: (_, __) =>
                                const Divider(height: 1, indent: 12),
                            itemBuilder: (_, i) =>
                                _CachedSessionTile(summary: cached[i]),
                          ),
                        ),
                      ],
                    ),
                  );
                },
              ),

              const SizedBox(height: 16),
            ],
          ),
        ),
      ),
    );
  }
}

// ── Cached session tile ──────────────────────────────────────────────────────

class _CachedSessionTile extends StatelessWidget {
  const _CachedSessionTile({required this.summary});
  final CachedSessionSummary summary;

  Color get _statusColor => switch (summary.status) {
        'running' => ClawdTheme.success,
        'paused' => ClawdTheme.warning,
        'error' => ClawdTheme.error,
        'completed' => Colors.grey,
        _ => Colors.white24,
      };

  @override
  Widget build(BuildContext context) {
    final repoName = summary.repoPath.split('/').last;
    return ListTile(
      contentPadding: const EdgeInsets.symmetric(horizontal: 12, vertical: 2),
      leading: Icon(Icons.circle, size: 10, color: _statusColor),
      title: Text(
        repoName,
        style: const TextStyle(fontSize: 13, fontWeight: FontWeight.w500),
      ),
      subtitle: Text(
        '${summary.messageCount} messages',
        style: const TextStyle(fontSize: 11, color: Colors.white38),
      ),
      trailing: Text(
        summary.status,
        style: TextStyle(fontSize: 11, color: _statusColor),
      ),
      dense: true,
    );
  }
}
