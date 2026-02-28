import 'dart:io';
import 'package:flutter/widgets.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';

/// Handles local push notifications for ClawDE mobile.
/// MN-01: Infrastructure + permission request.
/// MN-02: Tool call pending notifications.
/// MN-03: Session error/complete notifications.
/// MN-04: Deep-link routing on notification tap.
class NotificationService {
  NotificationService._();
  static final instance = NotificationService._();

  final _plugin = FlutterLocalNotificationsPlugin();

  /// Set by [init] — called when user taps a notification.
  void Function(String sessionId)? onNotificationTapped;

  static const _channelId = 'clawd_general';
  static const _channelName = 'ClawDE';
  static const _channelDesc = 'ClawDE session updates and approvals';

  // Notification IDs
  static const _toolCallIdBase = 1000;
  static const _sessionErrorIdBase = 2000;
  static const _sessionCompleteIdBase = 3000;

  Future<void> init() async {
    const androidInit = AndroidInitializationSettings('@mipmap/ic_launcher');
    const iosInit = DarwinInitializationSettings(
      requestAlertPermission: false, // request separately
      requestBadgePermission: false,
      requestSoundPermission: false,
    );

    await _plugin.initialize(
      const InitializationSettings(android: androidInit, iOS: iosInit),
      onDidReceiveNotificationResponse: _onTap,
    );

    // Handle notification that launched the app from terminated state.
    final launchDetails =
        await _plugin.getNotificationAppLaunchDetails();
    if (launchDetails?.didNotificationLaunchApp == true) {
      final payload = launchDetails!.notificationResponse?.payload;
      if (payload != null) {
        // Delay until router is ready.
        WidgetsBinding.instance.addPostFrameCallback((_) {
          onNotificationTapped?.call(payload);
        });
      }
    }
  }

  /// Requests iOS notification permissions. Call after app is running.
  Future<void> requestPermissions() async {
    if (Platform.isIOS) {
      await _plugin
          .resolvePlatformSpecificImplementation<
              IOSFlutterLocalNotificationsPlugin>()
          ?.requestPermissions(alert: true, badge: true, sound: true);
    } else if (Platform.isAndroid) {
      await _plugin
          .resolvePlatformSpecificImplementation<
              AndroidFlutterLocalNotificationsPlugin>()
          ?.requestNotificationsPermission();
    }
  }

  /// MN-02: Show a notification that [count] tool calls need approval.
  Future<void> showToolCallPending(
    String sessionId,
    String sessionName,
    int count,
  ) async {
    // Only notify when in background (approximated by lifecycle state).
    final lifecycle = WidgetsBinding.instance.lifecycleState;
    if (lifecycle == null || lifecycle == AppLifecycleState.resumed) {
      return;
    }
    final id = _toolCallIdBase + sessionId.hashCode.abs() % 900;
    await _plugin.show(
      id,
      'ClawDE needs your approval',
      '$sessionName — $count tool call${count == 1 ? '' : 's'} awaiting',
      _details(),
      payload: sessionId,
    );
  }

  /// MN-03: Show a notification that a session encountered an error.
  Future<void> showSessionError(
    String sessionId,
    String sessionName,
    String error,
  ) async {
    final lifecycle = WidgetsBinding.instance.lifecycleState;
    if (lifecycle == null || lifecycle == AppLifecycleState.resumed) {
      return;
    }
    final id = _sessionErrorIdBase + sessionId.hashCode.abs() % 900;
    await _plugin.show(
      id,
      'Session error',
      '$sessionName: $error',
      _details(),
      payload: sessionId,
    );
  }

  /// MN-03: Show a notification that a session completed.
  Future<void> showSessionComplete(
    String sessionId,
    String sessionName,
  ) async {
    final lifecycle = WidgetsBinding.instance.lifecycleState;
    if (lifecycle == null || lifecycle == AppLifecycleState.resumed) {
      return;
    }
    final id = _sessionCompleteIdBase + sessionId.hashCode.abs() % 900;
    await _plugin.show(
      id,
      'Session complete',
      '$sessionName finished.',
      _details(),
      payload: sessionId,
    );
  }

  NotificationDetails _details() {
    const android = AndroidNotificationDetails(
      _channelId,
      _channelName,
      channelDescription: _channelDesc,
      importance: Importance.high,
      priority: Priority.high,
    );
    const ios = DarwinNotificationDetails(
      presentAlert: true,
      presentSound: true,
    );
    return const NotificationDetails(android: android, iOS: ios);
  }

  void _onTap(NotificationResponse response) {
    final sessionId = response.payload;
    if (sessionId != null) {
      onNotificationTapped?.call(sessionId);
    }
  }
}
