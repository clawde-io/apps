import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mobile_scanner/mobile_scanner.dart';
import 'package:clawd_client/clawd_client.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde_mobile/features/hosts/host_provider.dart';

/// Full-screen QR scanner that parses a `ws://` or `wss://` URL from
/// the scanned code, performs device pairing with the daemon, and saves
/// the host with its pairing token for relay authentication.
class QrScannerSheet extends ConsumerStatefulWidget {
  const QrScannerSheet({super.key});

  @override
  ConsumerState<QrScannerSheet> createState() => _QrScannerSheetState();
}

class _QrScannerSheetState extends ConsumerState<QrScannerSheet> {
  final _controller = MobileScannerController();
  bool _handled = false;
  String? _errorMsg;
  bool _pairing = false;
  String? _pairingStatus;

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _onDetect(BarcodeCapture capture) {
    if (_handled) return;
    final raw = capture.barcodes.firstOrNull?.rawValue;
    if (raw == null) return;

    // Accept either the new JSON payload format or a bare ws:// URL (legacy).
    _QrPayload? payload;
    if (raw.startsWith('{')) {
      try {
        final map = jsonDecode(raw) as Map<String, dynamic>;
        payload = _QrPayload.fromJson(map);
      } catch (_) {
        setState(() => _errorMsg = 'Unrecognised QR code format.');
        return;
      }
    } else if (raw.startsWith('ws://') || raw.startsWith('wss://')) {
      // Legacy bare URL — no PIN, no relay info.
      payload = _QrPayload(url: raw);
    } else {
      setState(() => _errorMsg = 'Not a ClawDE QR code.');
      return;
    }

    _handled = true;
    _performPairing(payload);
  }

  /// Perform device pairing with the daemon described by [payload].
  ///
  /// Flow:
  /// 1. Connect to the daemon WebSocket at [_QrPayload.url].
  /// 2. Call `device.pair` with the PIN, device name, and platform.
  /// 3. Extract `device_token`, `host_name`, `daemon_id`, and `relay_url`
  ///    from the response — these are the canonical field names from
  ///    the Rust `PairResponse` struct in `daemon/src/pairing/model.rs`.
  /// 4. Save the host with all relay coordinates.
  /// 5. Switch to the new host (sets the device_token as the auth token).
  Future<void> _performPairing(_QrPayload payload) async {
    setState(() {
      _pairing = true;
      _pairingStatus = 'Connecting to daemon...';
      _errorMsg = null;
    });

    // Connect without auth — device.pair is the first (and only pre-auth) call.
    // The client is intentionally constructed with no authToken so it skips
    // the daemon.auth step; device.pair is accepted before auth on the daemon.
    ClawdClient? client;
    try {
      client = ClawdClient(url: payload.url, queueWhenOffline: false);
      await client.connect();

      if (!mounted) return;
      setState(() => _pairingStatus = 'Pairing device...');

      final deviceName = _getDeviceName();
      final platformInfo = _getPlatformInfo();

      final params = <String, dynamic>{
        'name': deviceName,
        'platform': platformInfo,
        if (payload.pin != null) 'pin': payload.pin,
      };

      final result = await client.call<Map<String, dynamic>>(
        'device.pair',
        params,
      );

      // Field names match the Rust PairResponse struct exactly.
      final deviceToken = result['device_token'] as String?;
      final hostName =
          result['host_name'] as String? ?? payload.hostName ?? 'ClawDE Host';
      final daemonId =
          result['daemon_id'] as String? ?? payload.daemonId;
      final relayUrl =
          result['relay_url'] as String? ?? payload.relayUrl;

      if (deviceToken == null || deviceToken.isEmpty) {
        throw Exception('Daemon returned no device_token');
      }

      if (!mounted) return;
      setState(() => _pairingStatus = 'Saving host...');

      // Build and persist the host with all pairing data.
      final host = DaemonHost(
        id: DateTime.now().millisecondsSinceEpoch.toString(),
        name: hostName,
        url: payload.url,
        pairingToken: deviceToken,
        relayUrl: relayUrl,
        daemonId: daemonId,
        lastConnected: DateTime.now(),
      );
      await ref.read(hostListProvider.notifier).add(host);

      // Switch — this calls DaemonNotifier.switchToHost which uses the
      // device_token for daemon.auth on all current and future connections.
      await switchHost(ref, host);

      if (!mounted) return;
      setState(() => _pairingStatus = 'Paired successfully.');

      await Future<void>.delayed(const Duration(milliseconds: 600));
      if (mounted) {
        Navigator.of(context).pop();
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Paired with $hostName'),
            backgroundColor: ClawdTheme.success,
          ),
        );
      }
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _pairing = false;
        _handled = false; // allow retry
        _pairingStatus = null;
        _errorMsg = 'Pairing failed: $e';
      });
    } finally {
      client?.disconnect();
    }
  }

  String _getDeviceName() {
    try {
      if (Platform.isIOS) return 'iPhone';
      if (Platform.isAndroid) return 'Android Device';
      return Platform.localHostname;
    } catch (_) {
      return 'Mobile Device';
    }
  }

  String _getPlatformInfo() {
    try {
      if (Platform.isIOS) return 'ios';
      if (Platform.isAndroid) return 'android';
      return Platform.operatingSystem;
    } catch (_) {
      return 'unknown';
    }
  }

  @override
  Widget build(BuildContext context) {
    return Stack(
      children: [
        // Camera preview (hidden behind overlay during pairing).
        MobileScanner(
          controller: _controller,
          onDetect: _onDetect,
          errorBuilder: (context, error, child) {
            if (error.errorCode == MobileScannerErrorCode.permissionDenied) {
              return const Center(
                child: Padding(
                  padding: EdgeInsets.symmetric(horizontal: 32),
                  child: Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      Icon(Icons.no_photography,
                          size: 48, color: Colors.white38),
                      SizedBox(height: 16),
                      Text(
                        'Camera access is required to scan QR codes.\n'
                        'Enable it in device Settings.',
                        textAlign: TextAlign.center,
                        style: TextStyle(color: Colors.white70, fontSize: 14),
                      ),
                    ],
                  ),
                ),
              );
            }
            return const Center(
              child:
                  Icon(Icons.error_outline, size: 48, color: Colors.white38),
            );
          },
        ),

        // Pairing overlay (covers camera when pairing is in progress).
        if (_pairing)
          Container(
            color: ClawdTheme.surface.withValues(alpha: 0.92),
            child: Center(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  const SizedBox(
                    width: 40,
                    height: 40,
                    child: CircularProgressIndicator(
                      strokeWidth: 3,
                      color: ClawdTheme.claw,
                    ),
                  ),
                  const SizedBox(height: 20),
                  Text(
                    _pairingStatus ?? 'Pairing...',
                    style: const TextStyle(
                      fontSize: 15,
                      color: Colors.white70,
                    ),
                  ),
                ],
              ),
            ),
          ),

        // Scan overlay (corner brackets) — hidden during pairing.
        if (!_pairing)
          Center(
            child: SizedBox(
              width: 240,
              height: 240,
              child: CustomPaint(painter: _CornerPainter()),
            ),
          ),

        // Header
        Positioned(
          top: 0,
          left: 0,
          right: 0,
          child: AppBar(
            backgroundColor: Colors.transparent,
            elevation: 0,
            title: Text(_pairing ? 'Pairing...' : 'Scan QR Code'),
            leading: IconButton(
              icon: const Icon(Icons.close),
              onPressed: _pairing ? null : () => Navigator.of(context).pop(),
            ),
          ),
        ),

        // Label (hidden during pairing)
        if (!_pairing)
          const Positioned(
            bottom: 48,
            left: 0,
            right: 0,
            child: Text(
              'Point camera at the QR code in ClawDE desktop',
              textAlign: TextAlign.center,
              style: TextStyle(color: Colors.white70, fontSize: 13),
            ),
          ),

        // Error message with retry button
        if (_errorMsg != null)
          Positioned(
            bottom: 80,
            left: 16,
            right: 16,
            child: Container(
              padding:
                  const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
              decoration: BoxDecoration(
                color: ClawdTheme.error.withValues(alpha: 0.85),
                borderRadius: BorderRadius.circular(8),
              ),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    _errorMsg!,
                    style: const TextStyle(color: Colors.white, fontSize: 12),
                    textAlign: TextAlign.center,
                  ),
                  const SizedBox(height: 8),
                  TextButton(
                    onPressed: () {
                      setState(() {
                        _errorMsg = null;
                        _handled = false;
                      });
                    },
                    style: TextButton.styleFrom(
                      foregroundColor: Colors.white,
                      backgroundColor: Colors.white.withValues(alpha: 0.15),
                      padding: const EdgeInsets.symmetric(
                          horizontal: 16, vertical: 6),
                    ),
                    child: const Text('Try Again',
                        style: TextStyle(fontSize: 12)),
                  ),
                ],
              ),
            ),
          ),
      ],
    );
  }
}

class _CornerPainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = ClawdTheme.claw
      ..strokeWidth = 3
      ..style = PaintingStyle.stroke
      ..strokeCap = StrokeCap.round;

    const len = 24.0;
    final w = size.width;
    final h = size.height;

    // Top-left
    canvas.drawLine(Offset.zero, const Offset(len, 0), paint);
    canvas.drawLine(Offset.zero, const Offset(0, len), paint);
    // Top-right
    canvas.drawLine(Offset(w, 0), Offset(w - len, 0), paint);
    canvas.drawLine(Offset(w, 0), Offset(w, len), paint);
    // Bottom-left
    canvas.drawLine(Offset(0, h), Offset(len, h), paint);
    canvas.drawLine(Offset(0, h), Offset(0, h - len), paint);
    // Bottom-right
    canvas.drawLine(Offset(w, h), Offset(w - len, h), paint);
    canvas.drawLine(Offset(w, h), Offset(w, h - len), paint);
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) => false;
}

// ─── QR payload model ─────────────────────────────────────────────────────────

/// Parsed contents of a ClawDE pairing QR code.
///
/// The daemon encodes a JSON object into the QR code:
/// ```json
/// {
///   "host": "Mac Mini",
///   "url": "ws://192.168.1.5:4300",
///   "relay": "wss://api.clawde.io/relay/ws",
///   "daemonId": "abc123",
///   "pin": "123456"
/// }
/// ```
///
/// Legacy QR codes are bare `ws://` or `wss://` URLs with no extra fields.
class _QrPayload {
  const _QrPayload({
    required this.url,
    this.hostName,
    this.relayUrl,
    this.daemonId,
    this.pin,
  });

  /// Direct WebSocket URL of the daemon (LAN address).
  final String url;

  /// Human-readable name of the host machine.
  final String? hostName;

  /// Relay WebSocket URL for off-LAN fallback.
  final String? relayUrl;

  /// Stable hardware fingerprint of the daemon instance.
  final String? daemonId;

  /// Short-lived 6-digit PIN displayed on the desktop host.
  final String? pin;

  factory _QrPayload.fromJson(Map<String, dynamic> map) {
    final url = map['url'] as String?;
    if (url == null || url.isEmpty) {
      throw const FormatException('QR payload missing "url" field');
    }
    return _QrPayload(
      url: url,
      hostName: map['host'] as String?,
      relayUrl: map['relay'] as String?,
      daemonId: map['daemonId'] as String?,
      pin: map['pin'] as String?,
    );
  }
}
