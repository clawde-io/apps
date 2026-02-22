// QA-01: Serialization tests for all clawd_proto types.
// Tests fromJson round-trips, enum coverage, and edge cases.
import 'package:clawd_proto/clawd_proto.dart';
import 'package:test/test.dart';

void main() {
  // ── Session ──────────────────────────────────────────────────────────────

  group('Session.fromJson', () {
    final baseJson = {
      'id': 'sess-1',
      'repo_path': '/home/user/myapp',
      'provider': 'claude',
      'status': 'running',
      'created_at': '2024-01-15T10:00:00.000Z',
      'started_at': '2024-01-15T10:00:01.000Z',
      'ended_at': null,
      'metadata': <String, dynamic>{},
    };

    test('parses required fields', () {
      final s = Session.fromJson(baseJson);
      expect(s.id, 'sess-1');
      expect(s.repoPath, '/home/user/myapp');
      expect(s.provider, ProviderType.claude);
      expect(s.status, SessionStatus.running);
    });

    test('parses optional fields when present', () {
      final s = Session.fromJson(baseJson);
      expect(s.startedAt, isNotNull);
      expect(s.endedAt, isNull);
    });

    test('null optional fields become null', () {
      final json = Map<String, dynamic>.from(baseJson)
        ..['started_at'] = null
        ..['ended_at'] = null;
      final s = Session.fromJson(json);
      expect(s.startedAt, isNull);
      expect(s.endedAt, isNull);
    });

    test('all SessionStatus values parse', () {
      for (final status in SessionStatus.values) {
        final json = Map<String, dynamic>.from(baseJson)
          ..['status'] = status.name;
        expect(Session.fromJson(json).status, status);
      }
    });

    test('all ProviderType values parse', () {
      for (final provider in ProviderType.values) {
        final json = Map<String, dynamic>.from(baseJson)
          ..['provider'] = provider.name;
        expect(Session.fromJson(json).provider, provider);
      }
    });

    test('metadata defaults to empty when absent', () {
      final json = Map<String, dynamic>.from(baseJson)..remove('metadata');
      final s = Session.fromJson(json);
      expect(s.metadata, isEmpty);
    });
  });

  // ── Message ───────────────────────────────────────────────────────────────

  group('Message.fromJson', () {
    final baseJson = {
      'id': 'msg-1',
      'sessionId': 'sess-1',
      'role': 'user',
      'content': 'Hello, AI!',
      'status': 'done',
      'createdAt': '2024-01-15T10:01:00.000Z',
      'metadata': <String, dynamic>{},
    };

    test('parses required fields', () {
      final m = Message.fromJson(baseJson);
      expect(m.id, 'msg-1');
      expect(m.sessionId, 'sess-1');
      expect(m.role, MessageRole.user);
      expect(m.content, 'Hello, AI!');
      expect(m.status, 'done');
    });

    test('all MessageRole values parse', () {
      for (final role in MessageRole.values) {
        final json = Map<String, dynamic>.from(baseJson)..['role'] = role.name;
        expect(Message.fromJson(json).role, role);
      }
    });

    test('status defaults to done when absent', () {
      final json = Map<String, dynamic>.from(baseJson)..remove('status');
      expect(Message.fromJson(json).status, 'done');
    });

    test('metadata defaults to empty when absent', () {
      final json = Map<String, dynamic>.from(baseJson)..remove('metadata');
      expect(Message.fromJson(json).metadata, isEmpty);
    });

    test('assistant message content preserved', () {
      final json = Map<String, dynamic>.from(baseJson)
        ..['role'] = 'assistant'
        ..['content'] = '## Answer\n\nHere is the code:\n\n```dart\nprint("hi");\n```';
      final m = Message.fromJson(json);
      expect(m.role, MessageRole.assistant);
      expect(m.content, contains('```dart'));
    });
  });

  // ── ToolCall ──────────────────────────────────────────────────────────────

  group('ToolCall.fromJson', () {
    final baseJson = {
      'id': 'tc-1',
      'sessionId': 'sess-1',
      'messageId': 'msg-1',
      'name': 'bash',
      'input': {'command': 'ls -la'},
      'status': 'pending',
      'createdAt': '2024-01-15T10:02:00.000Z',
      'completedAt': null,
    };

    test('parses required fields', () {
      final tc = ToolCall.fromJson(baseJson);
      expect(tc.id, 'tc-1');
      expect(tc.sessionId, 'sess-1');
      expect(tc.toolName, 'bash');
      expect(tc.status, ToolCallStatus.pending);
    });

    test('accepts name field (daemon snake_case alias)', () {
      final tc = ToolCall.fromJson(baseJson);
      expect(tc.toolName, 'bash');
    });

    test('accepts toolName field directly', () {
      final json = Map<String, dynamic>.from(baseJson)
        ..remove('name')
        ..['toolName'] = 'write_file';
      expect(ToolCall.fromJson(json).toolName, 'write_file');
    });

    test('maps done status to completed', () {
      final json = Map<String, dynamic>.from(baseJson)..['status'] = 'done';
      expect(ToolCall.fromJson(json).status, ToolCallStatus.completed);
    });

    test('all ToolCallStatus values parse (except done alias)', () {
      for (final status in ToolCallStatus.values) {
        final json = Map<String, dynamic>.from(baseJson)
          ..['status'] = status.name;
        expect(ToolCall.fromJson(json).status, status);
      }
    });

    test('completedAt is null when absent', () {
      final json = Map<String, dynamic>.from(baseJson)..['completedAt'] = null;
      expect(ToolCall.fromJson(json).completedAt, isNull);
    });

    test('completedAt parses when present', () {
      final json = Map<String, dynamic>.from(baseJson)
        ..['completedAt'] = '2024-01-15T10:02:05.000Z';
      expect(ToolCall.fromJson(json).completedAt, isNotNull);
    });
  });

  // ── RepoStatus ────────────────────────────────────────────────────────────

  group('RepoStatus.fromJson', () {
    final baseJson = {
      'path': '/home/user/myapp',
      'branch': 'main',
      'is_dirty': true,
      'ahead_by': 2,
      'behind_by': 0,
      'files': [
        {'path': 'lib/main.dart', 'state': 'modified', 'old_path': null},
        {'path': 'lib/new.dart', 'state': 'untracked', 'old_path': null},
      ],
    };

    test('parses required fields', () {
      final rs = RepoStatus.fromJson(baseJson);
      expect(rs.path, '/home/user/myapp');
      expect(rs.branch, 'main');
      expect(rs.isDirty, isTrue);
      expect(rs.aheadBy, 2);
      expect(rs.behindBy, 0);
    });

    test('parses files list', () {
      final rs = RepoStatus.fromJson(baseJson);
      expect(rs.files, hasLength(2));
      expect(rs.files[0].state, FileState.modified);
      expect(rs.files[1].state, FileState.untracked);
    });

    test('branch can be null', () {
      final json = Map<String, dynamic>.from(baseJson)..['branch'] = null;
      expect(RepoStatus.fromJson(json).branch, isNull);
    });

    test('all FileState values parse', () {
      for (final state in FileState.values) {
        final fileJson = {
          'path': 'test.dart',
          'state': state.name,
          'old_path': null,
        };
        expect(FileStatus.fromJson(fileJson).state, state);
      }
    });

    test('FileStatus oldPath parses for renames', () {
      final fileJson = {
        'path': 'lib/new_name.dart',
        'state': 'renamed',
        'old_path': 'lib/old_name.dart',
      };
      final fs = FileStatus.fromJson(fileJson);
      expect(fs.oldPath, 'lib/old_name.dart');
    });

    test('empty files list', () {
      final json = Map<String, dynamic>.from(baseJson)..['files'] = <dynamic>[];
      expect(RepoStatus.fromJson(json).files, isEmpty);
    });
  });

  // ── RpcRequest (toJson) ───────────────────────────────────────────────────

  group('RpcRequest.toJson', () {
    test('includes required fields', () {
      final req = RpcRequest(method: 'session.list', id: 1);
      final json = req.toJson();
      expect(json['jsonrpc'], '2.0');
      expect(json['method'], 'session.list');
      expect(json['id'], 1);
    });

    test('omits params when null', () {
      final json = RpcRequest(method: 'daemon.status', id: 2).toJson();
      expect(json.containsKey('params'), isFalse);
    });

    test('includes params when provided', () {
      final json = RpcRequest(
        method: 'session.create',
        params: {'repoPath': '/tmp/test', 'provider': 'claude'},
        id: 3,
      ).toJson();
      expect(json['params'], {'repoPath': '/tmp/test', 'provider': 'claude'});
    });
  });

  // ── RpcResponse / RpcError ────────────────────────────────────────────────

  group('RpcResponse.fromJson', () {
    test('success response', () {
      final r = RpcResponse.fromJson({
        'jsonrpc': '2.0',
        'result': {'id': 'sess-1'},
        'id': 1,
      });
      expect(r.isError, isFalse);
      expect(r.result, {'id': 'sess-1'});
    });

    test('error response', () {
      final r = RpcResponse.fromJson({
        'jsonrpc': '2.0',
        'error': {'code': -32001, 'message': 'Session not found'},
        'id': 2,
      });
      expect(r.isError, isTrue);
      expect(r.error!.code, -32001);
      expect(r.error!.message, 'Session not found');
    });

    test('null result for void responses', () {
      final r = RpcResponse.fromJson({
        'jsonrpc': '2.0',
        'result': null,
        'id': 3,
      });
      expect(r.result, isNull);
      expect(r.isError, isFalse);
    });
  });

  group('RpcError.fromJson', () {
    test('parses all fields', () {
      final err = RpcError.fromJson({
        'code': -32002,
        'message': 'Provider not available',
        'data': {'provider': 'codex'},
      });
      expect(err.code, -32002);
      expect(err.message, 'Provider not available');
      expect(err.data, {'provider': 'codex'});
    });

    test('data can be null', () {
      final err = RpcError.fromJson({
        'code': -32000,
        'message': 'Unknown error',
      });
      expect(err.data, isNull);
    });

    test('toString includes code and message', () {
      final err = RpcError(code: -32001, message: 'Not found');
      expect(err.toString(), contains('-32001'));
      expect(err.toString(), contains('Not found'));
    });
  });
}
