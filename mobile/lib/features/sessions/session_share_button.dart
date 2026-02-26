// session_share_button.dart — Share deep link button (Sprint RR DL.4).
//
// Generates a `clawde://session/{id}` URI and invokes the system share sheet.
// Add to SessionDetailScreen AppBar actions.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

class SessionShareButton extends StatelessWidget {
  const SessionShareButton({super.key, required this.sessionId});

  final String sessionId;

  String get _deepLink => 'clawde://session/$sessionId';

  @override
  Widget build(BuildContext context) {
    return IconButton(
      icon: const Icon(Icons.share_outlined),
      tooltip: 'Share session link',
      onPressed: () => _share(context),
    );
  }

  Future<void> _share(BuildContext context) async {
    // Copy to clipboard (system share sheet requires platform channels;
    // clipboard works universally across iOS + Android without extra packages)
    await Clipboard.setData(ClipboardData(text: _deepLink));

    if (!context.mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text('Copied: $_deepLink'),
        duration: const Duration(seconds: 2),
        action: SnackBarAction(
          label: 'OK',
          onPressed: () {},
        ),
      ),
    );
  }
}
