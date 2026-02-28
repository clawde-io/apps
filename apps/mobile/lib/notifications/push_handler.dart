// push_handler.dart — Push notification setup and tap routing (Sprint RR PN.5).
//
// Initialises firebase_messaging and flutter_local_notifications.
// Notification taps navigate to the correct session or task screen.

import 'package:firebase_messaging/firebase_messaging.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:go_router/go_router.dart';

// ─── Background message handler (must be top-level) ──────────────────────────

@pragma('vm:entry-point')
Future<void> _firebaseBackgroundHandler(RemoteMessage message) async {
  // Background processing — no UI available here
  debugPrint('[Push] background message: ${message.messageId}');
}

// ─── Push handler service ──────────────────────────────────────────────────

class PushHandler {
  PushHandler({required GoRouter router}) : _router = router;

  final GoRouter _router;
  final _messaging = FirebaseMessaging.instance;
  final _localNotifications = FlutterLocalNotificationsPlugin();

  static const _channelId = 'clawd_events';
  static const _channelName = 'Session Events';
  static const _channelDesc = 'Notifications for AI session events';

  Future<void> init() async {
    // Register background handler
    FirebaseMessaging.onBackgroundMessage(_firebaseBackgroundHandler);

    // Request iOS/Android permission
    await _messaging.requestPermission(
      alert: true,
      badge: true,
      sound: true,
    );

    // Android notification channel
    const androidChannel = AndroidNotificationChannel(
      _channelId,
      _channelName,
      description: _channelDesc,
      importance: Importance.high,
    );

    await _localNotifications
        .resolvePlatformSpecificImplementation<
            AndroidFlutterLocalNotificationsPlugin>()
        ?.createNotificationChannel(androidChannel);

    // Initialise local notifications
    const initSettings = InitializationSettings(
      android: AndroidInitializationSettings('@mipmap/ic_launcher'),
      iOS: DarwinInitializationSettings(),
    );

    await _localNotifications.initialize(
      initSettings,
      onDidReceiveNotificationResponse: _onTap,
    );

    // Foreground message display
    FirebaseMessaging.onMessage.listen(_onForegroundMessage);

    // Notification tap when app was in background
    FirebaseMessaging.onMessageOpenedApp.listen(_onMessageTap);

    // Check if the app was opened via notification (terminated state)
    final initial = await _messaging.getInitialMessage();
    if (initial != null) _onMessageTap(initial);
  }

  void _onForegroundMessage(RemoteMessage message) {
    final notification = message.notification;
    if (notification == null) return;

    _localNotifications.show(
      notification.hashCode,
      notification.title,
      notification.body,
      const NotificationDetails(
        android: AndroidNotificationDetails(
          _channelId,
          _channelName,
          channelDescription: _channelDesc,
          importance: Importance.high,
          priority: Priority.high,
        ),
        iOS: DarwinNotificationDetails(
          presentAlert: true,
          presentBadge: true,
          presentSound: true,
        ),
      ),
      payload: _payloadFrom(message.data),
    );
  }

  void _onMessageTap(RemoteMessage message) {
    _routeFromData(message.data);
  }

  void _onTap(NotificationResponse response) {
    final payload = response.payload;
    if (payload == null) return;
    // Payload format: "session:{id}" or "task:{id}"
    if (payload.startsWith('session:')) {
      _router.push('/sessions/${payload.substring(8)}');
    } else if (payload.startsWith('task:')) {
      _router.push('/tasks?highlight=${payload.substring(5)}');
    }
  }

  void _routeFromData(Map<String, dynamic> data) {
    final sessionId = data['session_id'] as String?;
    final taskId = data['task_id'] as String?;
    if (sessionId != null) {
      _router.push('/sessions/$sessionId');
    } else if (taskId != null) {
      _router.push('/tasks?highlight=$taskId');
    }
  }

  String _payloadFrom(Map<String, dynamic> data) {
    if (data['session_id'] != null) return 'session:${data['session_id']}';
    if (data['task_id'] != null) return 'task:${data['task_id']}';
    return '';
  }

  /// Get the FCM registration token to send to the daemon via push.register.
  Future<String?> getDeviceToken() => _messaging.getToken();
}
