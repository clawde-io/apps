// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Japanese (`ja`).
class AppLocalizationsJa extends AppLocalizations {
  AppLocalizationsJa([String locale = 'ja']) : super(locale);

  @override
  String get appTitle => 'ClawDE';

  @override
  String get navChat => 'チャット';

  @override
  String get navSessions => 'セッション';

  @override
  String get navFiles => 'ファイル';

  @override
  String get navTasks => 'タスク';

  @override
  String get navAnalytics => '分析';

  @override
  String get navDoctor => 'ドクター';

  @override
  String get navAutomations => '自動化';

  @override
  String get navSettings => '設定';

  @override
  String get newSession => '新しいセッション';

  @override
  String get noSessionSelected => 'セッションを選択するか、新しいセッションを開始してください';

  @override
  String get connecting => '接続中…';

  @override
  String get connected => '接続済み';

  @override
  String get disconnected => '切断済み';

  @override
  String get sessionComplete => 'セッション完了';

  @override
  String get sessionRunning => '実行中';

  @override
  String get sessionPaused => '一時停止';

  @override
  String get sessionError => 'エラー';

  @override
  String get cancel => 'キャンセル';

  @override
  String get confirm => '確認';

  @override
  String get save => '保存';

  @override
  String get delete => '削除';

  @override
  String get retry => '再試行';

  @override
  String get refresh => '更新';

  @override
  String get errorGeneric => '問題が発生しました。もう一度お試しください。';

  @override
  String get errorDaemonOffline => 'デーモンがオフラインです。clawdを起動して再接続してください。';

  @override
  String get settingsTitle => '設定';

  @override
  String get settingsLanguage => '言語';

  @override
  String get automationsTitle => '自動化';

  @override
  String get evalsTitle => '評価ランナー';
}
