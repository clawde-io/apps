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

    if (!raw.startsWith('ws://') && !raw.startsWith('wss://')) {
      setState(() => _errorMsg = 'Not a ClawDE URL: $raw');
      return;
    }

    _handled = true;
    _performPairing(raw);
  }

  /// Perform device pairing with the daemon at the scanned URL.
  ///
  /// 1. Connect to daemon via WebSocket
  /// 2. POST device info via `device.pair` RPC
  /// 3. Receive pairing token
  /// 4. Save host with pairing token
  Future<void> _performPairing(String daemonUrl) async {
    setState(() {
      _pairing = true;
      _pairingStatus = 'Connecting to daemon...';
      _errorMsg = null;
    });

    ClawdClient? client;
    try {
      // 1. Connect to the daemon.
      client = ClawdClient(url: daemonUrl);
      await client.connect();

      if (!mounted) return;
      setState(() => _pairingStatus = 'Pairing device...');

      // 2. Send device pairing request with device info.
      final deviceName = _getDeviceName();
      final platformInfo = _getPlatformInfo();

      final result = await client.call<Map<String, dynamic>>(
        'device.pair',
        {
          'deviceName': deviceName,
          'platform': platformInfo,
          'deviceType': 'mobile',
        },
      );

      final pairingToken = result['token'] as String?;
      final daemonName = result['daemonName'] as String? ?? 'ClawDE Daemon';

      if (pairingToken == null || pairingToken.isEmpty) {
        throw Exception('Daemon returned no pairing token');
      }

      if (!mounted) return;
      setState(() => _pairingStatus = 'Saving host...');

      // 3. Save the host with pairing token.
      final host = DaemonHost(
        id: DateTime.now().millisecondsSinceEpoch.toString(),
        name: daemonName,
        url: daemonUrl,
        pairingToken: pairingToken,
        lastConnected: DateTime.now(),
      );
      await ref.read(hostListProvider.notifier).add(host);

      // 4. Switch to this host.
      await switchHost(ref, host);

      if (!mounted) return;

      // 5. Show success and close.
      setState(() => _pairingStatus = 'Paired successfully.');

      // Brief delay to show success state, then pop.
      await Future<void>.delayed(const Duration(milliseconds: 600));
      if (mounted) {
        Navigator.of(context).pop();
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Paired with $daemonName'),
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
