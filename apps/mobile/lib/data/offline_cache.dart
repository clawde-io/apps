// offline_cache.dart — Mobile SQLite offline cache (Sprint RR MO.1).
//
// Schema:
//   offline_sessions — cached session list
//   offline_messages — cached message history per session
//
// Uses `sqflite` for local persistence. The cache mirrors the subset of
// daemon data needed for offline browsing — no writes (read-only when offline).

import 'package:sqflite/sqflite.dart';
import 'package:path/path.dart' as path;

class OfflineCache {
  static const int _schemaVersion = 1;
  static const String _dbName = 'clawd_offline.db';

  Database? _db;

  Future<Database> get _database async {
    _db ??= await _open();
    return _db!;
  }

  Future<Database> _open() async {
    final dbPath = path.join(await getDatabasesPath(), _dbName);
    return openDatabase(
      dbPath,
      version: _schemaVersion,
      onCreate: _onCreate,
      onUpgrade: _onUpgrade,
    );
  }

  Future<void> _onCreate(Database db, int version) async {
    await db.execute('''
      CREATE TABLE IF NOT EXISTS offline_sessions (
        id TEXT PRIMARY KEY,
        repo_path TEXT NOT NULL,
        provider TEXT NOT NULL DEFAULT 'claude',
        status TEXT NOT NULL DEFAULT 'idle',
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        message_count INTEGER NOT NULL DEFAULT 0,
        synced_at TEXT NOT NULL
      )
    ''');

    await db.execute('''
      CREATE TABLE IF NOT EXISTS offline_messages (
        id TEXT PRIMARY KEY,
        session_id TEXT NOT NULL REFERENCES offline_sessions(id) ON DELETE CASCADE,
        role TEXT NOT NULL,
        content TEXT NOT NULL,
        created_at TEXT NOT NULL,
        sequence INTEGER NOT NULL DEFAULT 0
      )
    ''');

    await db.execute(
      'CREATE INDEX IF NOT EXISTS idx_offline_messages_session ON offline_messages(session_id, sequence)',
    );
  }

  Future<void> _onUpgrade(Database db, int oldVersion, int newVersion) async {
    // Reserved for future schema migrations
  }

  // ─── Session cache ─────────────────────────────────────────────────────────

  Future<void> upsertSession(Map<String, dynamic> session) async {
    final db = await _database;
    await db.insert(
      'offline_sessions',
      {
        ...session,
        'synced_at': DateTime.now().toIso8601String(),
      },
      conflictAlgorithm: ConflictAlgorithm.replace,
    );
  }

  Future<List<Map<String, dynamic>>> getSessions() async {
    final db = await _database;
    return db.query(
      'offline_sessions',
      orderBy: 'updated_at DESC',
    );
  }

  Future<Map<String, dynamic>?> getSession(String id) async {
    final db = await _database;
    final rows = await db.query(
      'offline_sessions',
      where: 'id = ?',
      whereArgs: [id],
      limit: 1,
    );
    return rows.isNotEmpty ? rows.first : null;
  }

  // ─── Message cache ─────────────────────────────────────────────────────────

  Future<void> upsertMessages(
    String sessionId,
    List<Map<String, dynamic>> messages,
  ) async {
    final db = await _database;
    final batch = db.batch();
    for (final msg in messages) {
      batch.insert(
        'offline_messages',
        {'session_id': sessionId, ...msg},
        conflictAlgorithm: ConflictAlgorithm.replace,
      );
    }
    await batch.commit(noResult: true);
  }

  Future<List<Map<String, dynamic>>> getMessages(String sessionId) async {
    final db = await _database;
    return db.query(
      'offline_messages',
      where: 'session_id = ?',
      whereArgs: [sessionId],
      orderBy: 'sequence ASC',
    );
  }

  // ─── Housekeeping ──────────────────────────────────────────────────────────

  /// Remove cache entries older than [days] days.
  Future<void> prune({int days = 30}) async {
    final db = await _database;
    final cutoff = DateTime.now().subtract(Duration(days: days)).toIso8601String();
    await db.delete(
      'offline_sessions',
      where: 'synced_at < ?',
      whereArgs: [cutoff],
    );
  }

  Future<void> close() async {
    await _db?.close();
    _db = null;
  }
}
