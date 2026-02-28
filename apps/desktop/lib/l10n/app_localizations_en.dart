// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get appTitle => 'ClawDE';

  @override
  String get navChat => 'Chat';

  @override
  String get navSessions => 'Sessions';

  @override
  String get navFiles => 'Files';

  @override
  String get navTasks => 'Tasks';

  @override
  String get navAnalytics => 'Analytics';

  @override
  String get navDoctor => 'Doctor';

  @override
  String get navAutomations => 'Automations';

  @override
  String get navSettings => 'Settings';

  @override
  String get newSession => 'New Session';

  @override
  String get noSessionSelected => 'Select a session or start a new one';

  @override
  String get connecting => 'Connecting…';

  @override
  String get connected => 'Connected';

  @override
  String get disconnected => 'Disconnected';

  @override
  String get sessionComplete => 'Session complete';

  @override
  String get sessionRunning => 'Running';

  @override
  String get sessionPaused => 'Paused';

  @override
  String get sessionError => 'Error';

  @override
  String get cancel => 'Cancel';

  @override
  String get confirm => 'Confirm';

  @override
  String get save => 'Save';

  @override
  String get delete => 'Delete';

  @override
  String get retry => 'Retry';

  @override
  String get refresh => 'Refresh';

  @override
  String get errorGeneric => 'Something went wrong. Please try again.';

  @override
  String get errorDaemonOffline =>
      'Daemon is offline. Start clawd and reconnect.';

  @override
  String get settingsTitle => 'Settings';

  @override
  String get settingsLanguage => 'Language';

  @override
  String get automationsTitle => 'Automations';

  @override
  String get evalsTitle => 'Eval Runner';
}
