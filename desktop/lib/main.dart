import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:window_manager/window_manager.dart';
import 'package:clawde/app.dart';
import 'package:clawde/services/updater_service.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await windowManager.ensureInitialized();
  await UpdaterService.instance.init();

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

  // Check for updates 5s after startup (non-blocking)
  Future.delayed(const Duration(seconds: 5),
      () => UpdaterService.instance.checkInBackground());

  runApp(const ProviderScope(child: ClawDEApp()));
}
