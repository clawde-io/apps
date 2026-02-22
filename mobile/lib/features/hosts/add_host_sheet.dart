import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:multicast_dns/multicast_dns.dart';
import 'package:clawd_client/clawd_client.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde_mobile/features/hosts/host_provider.dart';
import 'package:clawde_mobile/features/hosts/qr_scanner_sheet.dart';

class AddHostSheet extends ConsumerStatefulWidget {
  const AddHostSheet({super.key, this.prefillUrl});

  /// Optional pre-filled URL (e.g. from QR scan).
  final String? prefillUrl;

  @override
  ConsumerState<AddHostSheet> createState() => _AddHostSheetState();
}

class _AddHostSheetState extends ConsumerState<AddHostSheet> {
  final _nameController = TextEditingController();
  late final TextEditingController _urlController;
  bool _testing = false;
  bool _saving = false;
  bool _discovering = false;
  String? _testResult;
  final List<String> _discovered = [];

  @override
  void initState() {
    super.initState();
    _urlController =
        TextEditingController(text: widget.prefillUrl ?? 'ws://');
  }

  @override
  void dispose() {
    _nameController.dispose();
    _urlController.dispose();
    super.dispose();
  }

  bool get _isValid {
    final url = _urlController.text.trim();
    return _nameController.text.trim().isNotEmpty &&
        (url.startsWith('ws://') || url.startsWith('wss://')) &&
        url.length > 8;
  }

  Future<void> _test() async {
    if (!_isValid) return;
    setState(() {
      _testing = true;
      _testResult = null;
    });
    try {
      final client = ClawdClient(url: _urlController.text.trim());
      await client.connect();
      await client.call<Map<String, dynamic>>('daemon.status');
      client.disconnect();
      if (mounted) setState(() => _testResult = 'Connected successfully');
    } catch (e) {
      if (mounted) setState(() => _testResult = 'Failed: $e');
    } finally {
      if (mounted) setState(() => _testing = false);
    }
  }

  Future<void> _discover() async {
    setState(() {
      _discovering = true;
      _discovered.clear();
    });
    try {
      final client = MDnsClient();
      await client.start();

      // Look for _clawd._tcp mDNS services (3-second timeout)
      await Future.any([
        Future.delayed(const Duration(seconds: 3)),
        () async {
          await for (final ptr in client
              .lookup<PtrResourceRecord>(ResourceRecordQuery.serverPointer(
                  '_clawd._tcp.local'))) {
            await for (final srv in client.lookup<SrvResourceRecord>(
                ResourceRecordQuery.service(ptr.domainName))) {
              await for (final ip in client.lookup<IPAddressResourceRecord>(
                  ResourceRecordQuery.addressIPv4(srv.target))) {
                final url = 'ws://${ip.address.address}:${srv.port}';
                if (mounted && !_discovered.contains(url)) {
                  setState(() => _discovered.add(url));
                }
              }
            }
          }
        }(),
      ]);
      client.stop();
    } catch (_) {
      // mDNS failure is non-fatal — fall through to empty state
    } finally {
      if (mounted) setState(() => _discovering = false);
    }
  }

  Future<void> _save() async {
    if (!_isValid) return;
    setState(() => _saving = true);
    try {
      final host = DaemonHost(
        id: DateTime.now().millisecondsSinceEpoch.toString(),
        name: _nameController.text.trim(),
        url: _urlController.text.trim(),
      );
      await ref.read(hostListProvider.notifier).add(host);
      if (mounted) Navigator.pop(context);
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  void _openQrScanner() {
    Navigator.pop(context); // close this sheet first
    Navigator.of(context).push(
      MaterialPageRoute<void>(
        fullscreenDialog: true,
        builder: (_) => const QrScannerSheet(),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final bottomInset = MediaQuery.viewInsetsOf(context).bottom;

    return Padding(
      padding: EdgeInsets.fromLTRB(16, 16, 16, bottomInset + 16),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Text(
                'Add Host',
                style: TextStyle(fontSize: 18, fontWeight: FontWeight.w600),
              ),
              const Spacer(),
              TextButton.icon(
                onPressed: _openQrScanner,
                icon: const Icon(Icons.qr_code_scanner, size: 16),
                label: const Text('Scan QR'),
                style: TextButton.styleFrom(
                    foregroundColor: ClawdTheme.clawLight),
              ),
            ],
          ),
          const SizedBox(height: 16),
          TextField(
            controller: _nameController,
            decoration: const InputDecoration(
              labelText: 'Name',
              hintText: 'Home Desktop',
            ),
            onChanged: (_) => setState(() {}),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _urlController,
            decoration: const InputDecoration(
              labelText: 'WebSocket URL',
              hintText: 'ws://192.168.1.100:4300',
            ),
            keyboardType: TextInputType.url,
            onChanged: (_) => setState(() => _testResult = null),
          ),
          if (_testResult != null) ...[
            const SizedBox(height: 8),
            Text(
              _testResult!,
              style: TextStyle(
                fontSize: 12,
                color: _testResult!.startsWith('Connected')
                    ? Colors.green
                    : Colors.red,
              ),
            ),
          ],
          const SizedBox(height: 12),
          // LAN discovery
          OutlinedButton.icon(
            onPressed: _discovering ? null : _discover,
            icon: _discovering
                ? const SizedBox(
                    width: 14,
                    height: 14,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.search, size: 16),
            label: Text(_discovering ? 'Scanning LAN…' : 'Discover on LAN'),
            style: OutlinedButton.styleFrom(
              foregroundColor: Colors.white60,
              side: const BorderSide(color: ClawdTheme.surfaceBorder),
            ),
          ),
          if (_discovered.isNotEmpty) ...[
            const SizedBox(height: 8),
            ...(_discovered.map((url) => ListTile(
                  dense: true,
                  leading: const Icon(Icons.devices, size: 16),
                  title: Text(url,
                      style: const TextStyle(fontSize: 12)),
                  onTap: () => setState(() {
                    _urlController.text = url;
                    _testResult = null;
                  }),
                ))),
          ],
          if (!_discovering && _discovered.isEmpty && _testResult == null)
            const SizedBox.shrink(),
          const SizedBox(height: 16),
          Row(
            children: [
              OutlinedButton.icon(
                onPressed: _testing || !_isValid ? null : _test,
                icon: _testing
                    ? const SizedBox(
                        width: 14,
                        height: 14,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.wifi_find, size: 16),
                label: const Text('Test'),
                style: OutlinedButton.styleFrom(
                  foregroundColor: Colors.white70,
                  side: const BorderSide(color: ClawdTheme.surfaceBorder),
                ),
              ),
              const Spacer(),
              TextButton(
                onPressed: () => Navigator.pop(context),
                child: const Text('Cancel'),
              ),
              const SizedBox(width: 8),
              FilledButton(
                onPressed: _saving || !_isValid ? null : _save,
                style: FilledButton.styleFrom(
                    backgroundColor: ClawdTheme.claw),
                child: _saving
                    ? const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(
                            strokeWidth: 2, color: Colors.white),
                      )
                    : const Text('Save'),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
