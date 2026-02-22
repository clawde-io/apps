import 'package:auto_updater/auto_updater.dart';

/// Manages app auto-update via Sparkle (macOS) / WinSparkle (Windows).
///
/// Feed URL points to the ClawDE release feed.
/// Requires native Sparkle setup in the macOS Runner — see:
///   https://pub.dev/packages/auto_updater#getting-started
class UpdaterService {
  UpdaterService._();
  static final instance = UpdaterService._();

  static const _feedUrl = 'https://api.clawde.io/updates/appcast.xml';

  Future<void> init() async {
    await autoUpdater.setFeedURL(_feedUrl);
    await autoUpdater.setScheduledCheckInterval(86400); // 24 hours
  }

  /// Silently checks for updates in the background.
  Future<void> checkInBackground() async {
    try {
      await autoUpdater.checkForUpdates(inBackground: true);
    } catch (_) {
      // Feed may be unreachable in dev — fail silently
    }
  }

  /// Shows the update dialog (user-initiated).
  Future<void> checkForUpdates() async {
    try {
      await autoUpdater.checkForUpdates();
    } catch (_) {
      // Feed may be unreachable — fail silently
    }
  }
}
