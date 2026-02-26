import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';

// ─── Provider ─────────────────────────────────────────────────────────────────

/// Sprint EE CS.7 — fetches the share list for a session.
final _shareListProvider =
    FutureProvider.autoDispose.family<ShareListResult, String>((ref, sessionId) async {
  final client = ref.read(daemonProvider.notifier).client;
  final raw = await client.call<Map<String, dynamic>>('session.shareList', {
    'session_id': sessionId,
  });
  return ShareListResult.fromJson(raw);
});

// ─── Screen ───────────────────────────────────────────────────────────────────

/// Sprint EE CS.7 — Mobile Shared Session viewer screen.
///
/// Shows active share tokens for a session and allows revoking them.
class SharedSessionScreen extends ConsumerWidget {
  const SharedSessionScreen({super.key, required this.sessionId});

  final String sessionId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final shareListAsync = ref.watch(_shareListProvider(sessionId));

    return Scaffold(
      appBar: AppBar(
        title: const Text('Shared Session'),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            onPressed: () => ref.invalidate(_shareListProvider(sessionId)),
          ),
        ],
      ),
      body: shareListAsync.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(
          child: Text('Failed to load shares: $e',
              style: const TextStyle(color: Colors.redAccent)),
        ),
        data: (result) => result.shares.isEmpty
            ? const Center(
                child: Text(
                  'No active shares.\nUse "Share Session" to invite collaborators.',
                  textAlign: TextAlign.center,
                  style: TextStyle(color: Colors.white54),
                ),
              )
            : _ShareList(
                sessionId: sessionId,
                result: result,
                onRevoked: () => ref.invalidate(_shareListProvider(sessionId)),
              ),
      ),
      floatingActionButton: FloatingActionButton.extended(
        icon: const Icon(Icons.share),
        label: const Text('Share'),
        onPressed: () => _showShareDialog(context, ref),
      ),
    );
  }

  Future<void> _showShareDialog(BuildContext context, WidgetRef ref) async {
    final expiryCtrl = TextEditingController(text: '3600');
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Share Session'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Text(
              'Create a share token so another device or user can view this session.',
              style: TextStyle(color: Colors.white70),
            ),
            const SizedBox(height: 16),
            TextField(
              controller: expiryCtrl,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(
                labelText: 'Expires in (seconds)',
                border: OutlineInputBorder(),
              ),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx, false),
            child: const Text('Cancel'),
          ),
          ElevatedButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: const Text('Create Share'),
          ),
        ],
      ),
    );

    if (confirmed == true && context.mounted) {
      try {
        final client = ref.read(daemonProvider.notifier).client;
        final expiresIn = int.tryParse(expiryCtrl.text) ?? 3600;
        final raw = await client.call<Map<String, dynamic>>('session.share', {
          'session_id': sessionId,
          'expires_in': expiresIn,
        });
        final token = raw['shareToken'] as String? ?? '';
        if (context.mounted) {
          ref.invalidate(_shareListProvider(sessionId));
          _showTokenDialog(context, token);
        }
      } catch (e) {
        if (context.mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text('Failed to create share: $e')),
          );
        }
      }
    }
  }

  void _showTokenDialog(BuildContext context, String token) {
    showDialog<void>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Share Token'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Text('Share this token with the other user:',
                style: TextStyle(color: Colors.white70)),
            const SizedBox(height: 12),
            SelectableText(
              token,
              style: const TextStyle(
                fontFamily: 'monospace',
                fontSize: 13,
                color: Colors.amber,
              ),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx),
            child: const Text('Done'),
          ),
        ],
      ),
    );
  }
}

// ─── Share List ───────────────────────────────────────────────────────────────

class _ShareList extends StatelessWidget {
  const _ShareList({
    required this.sessionId,
    required this.result,
    required this.onRevoked,
  });

  final String sessionId;
  final ShareListResult result;
  final VoidCallback onRevoked;

  @override
  Widget build(BuildContext context) {
    return ListView.builder(
      padding: const EdgeInsets.all(16),
      itemCount: result.shares.length,
      itemBuilder: (context, index) =>
          _ShareTile(share: result.shares[index], onRevoked: onRevoked),
    );
  }
}

class _ShareTile extends ConsumerWidget {
  const _ShareTile({required this.share, required this.onRevoked});

  final SessionShare share;
  final VoidCallback onRevoked;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final expiresAt = DateTime.tryParse(share.expiresAt);
    final expiresIn = expiresAt?.difference(DateTime.now());
    final expired = expiresIn != null && expiresIn.isNegative;

    return Card(
      margin: const EdgeInsets.only(bottom: 8),
      child: ListTile(
        leading: Icon(
          Icons.link,
          color: expired ? Colors.red : Colors.green,
          size: 20,
        ),
        title: Text(
          '${share.shareToken.substring(0, 12)}…',
          style: const TextStyle(fontFamily: 'monospace', fontSize: 13),
        ),
        subtitle: Text(
          expired
              ? 'Expired'
              : expiresIn != null
                  ? 'Expires in ${_formatDuration(expiresIn)}'
                  : 'No expiry',
          style: TextStyle(
            fontSize: 11,
            color: expired ? Colors.red : Colors.white54,
          ),
        ),
        trailing: IconButton(
          icon: const Icon(Icons.cancel_outlined, size: 18, color: Colors.red),
          tooltip: 'Revoke',
          onPressed: () => _revoke(context, ref),
        ),
      ),
    );
  }

  Future<void> _revoke(BuildContext context, WidgetRef ref) async {
    try {
      final client = ref.read(daemonProvider.notifier).client;
      await client.call<Map<String, dynamic>>('session.revokeShare', {
        'share_token': share.shareToken,
      });
      onRevoked();
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Failed to revoke: $e')),
        );
      }
    }
  }

  String _formatDuration(Duration d) {
    if (d.inHours > 0) return '${d.inHours}h ${d.inMinutes.remainder(60)}m';
    if (d.inMinutes > 0) return '${d.inMinutes}m';
    return '${d.inSeconds}s';
  }
}
