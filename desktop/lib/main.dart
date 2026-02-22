import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:window_manager/window_manager.dart';
import 'package:clawde/app.dart';
import 'package:clawde/services/daemon_manager.dart';
import 'package:clawde/services/updater_service.dart';
import 'package:clawd_core/clawd_core.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await windowManager.ensureInitialized();
  await UpdaterService.instance.init();
  await DaemonManager.instance.ensureRunning();

  const WindowOptions windowOptions = WindowOptions(
    minimumSize: Size(900, 600),
    size: Size(1280, 800),
    center: true,
    title: 'ClawDE',
    titleBarStyle: TitleBarStyle.normal,
  );
  await windowManager.waitUntilReadyToShow(windowOptions, () async {
    await windowManager.show();
    await windowManager.focus();
  });

  // Intercept window close so we can shut down the daemon gracefully.
  windowManager.addListener(_AppWindowListener());
  await windowManager.setPreventClose(true);

  // Check for updates 5 s after startup (non-blocking).
  Future.delayed(const Duration(seconds: 5),
      () => UpdaterService.instance.checkInBackground());

  runApp(ProviderScope(
    overrides: [
      // Inject the token obtained by DaemonManager so DaemonNotifier does not
      // need to race against the token file appearing on disk.
      bootstrapTokenProvider.overrideWithValue(
        DaemonManager.instance.tokenOverride,
      ),
    ],
    child: const ClawDEApp(),
  ));
}

class _AppWindowListener extends WindowListener {
  @override
  Future<void> onWindowClose() async {
    await DaemonManager.instance.shutdown();
    await windowManager.destroy();
  }
}
