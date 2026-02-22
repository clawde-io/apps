import 'package:flutter/material.dart';
import 'package:clawde/router.dart';
import 'package:clawde/theme/app_theme.dart';
import 'package:clawde/services/snackbar_service.dart';

class ClawDEApp extends StatelessWidget {
  const ClawDEApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp.router(
      title: 'ClawDE',
      theme: AppTheme.dark(),
      routerConfig: appRouter,
      scaffoldMessengerKey: scaffoldMessengerKey,
      debugShowCheckedModeBanner: false,
    );
  }
}
