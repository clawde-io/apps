// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for French (`fr`).
class AppLocalizationsFr extends AppLocalizations {
  AppLocalizationsFr([String locale = 'fr']) : super(locale);

  @override
  String get appTitle => 'ClawDE';

  @override
  String get navChat => 'Discussion';

  @override
  String get navSessions => 'Sessions';

  @override
  String get navFiles => 'Fichiers';

  @override
  String get navTasks => 'Tâches';

  @override
  String get navAnalytics => 'Analytique';

  @override
  String get navDoctor => 'Docteur';

  @override
  String get navAutomations => 'Automatisations';

  @override
  String get navSettings => 'Paramètres';

  @override
  String get newSession => 'Nouvelle session';

  @override
  String get noSessionSelected =>
      'Sélectionnez une session ou démarrez-en une nouvelle';

  @override
  String get connecting => 'Connexion…';

  @override
  String get connected => 'Connecté';

  @override
  String get disconnected => 'Déconnecté';

  @override
  String get sessionComplete => 'Session terminée';

  @override
  String get sessionRunning => 'En cours';

  @override
  String get sessionPaused => 'En pause';

  @override
  String get sessionError => 'Erreur';

  @override
  String get cancel => 'Annuler';

  @override
  String get confirm => 'Confirmer';

  @override
  String get save => 'Enregistrer';

  @override
  String get delete => 'Supprimer';

  @override
  String get retry => 'Réessayer';

  @override
  String get refresh => 'Actualiser';

  @override
  String get errorGeneric =>
      'Quelque chose s\'est mal passé. Veuillez réessayer.';

  @override
  String get errorDaemonOffline =>
      'Le démon est hors ligne. Démarrez clawd et reconnectez-vous.';

  @override
  String get settingsTitle => 'Paramètres';

  @override
  String get settingsLanguage => 'Langue';

  @override
  String get automationsTitle => 'Automatisations';

  @override
  String get evalsTitle => 'Exécuteur d\'évaluations';
}
