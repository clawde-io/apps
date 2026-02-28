import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawde_mobile/app.dart';
import 'package:clawde_mobile/services/notification_service.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();

  // MN-01: Init notification service (handles cold-start deep links).
  await NotificationService.instance.init();

  runApp(const ProviderScope(child: ClawDEMobileApp()));
}
