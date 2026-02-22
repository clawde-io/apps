// QA-01: Serialization tests for all clawd_proto types.
// Tests fromJson round-trips, enum coverage, and edge cases.
import 'package:clawd_proto/clawd_proto.dart';
import 'package:test/test.dart';

void main() {
  // ── Session ──────────────────────────────────────────────────────────────

  group('Session.fromJson', () {
    final baseJson = {
      'id': 'sess-1',
      'repoPath': '/home/user/myapp',
      'title': 'My Session',
      'provider': 'claude',
      'status': 'running',
      'createdAt': '2024-01-15T10:00:00.000Z',
      'updatedAt': '2024-01-15T10:00:01.000Z',
      'messageCount': 3,
    };

    test('parses required fields', () {
      final s = Session.fromJson(baseJson);
      expect(s.id, 'sess-1');
      expect(s.repoPath, '/home/user/myapp');
      expect(s.title, 'My Session');
      expect(s.provider, ProviderType.claude);
      expect(s.status, SessionStatus.running);
      expect(s.messageCount, 3);
    });

    test('parses timestamps', () {
      final s = Session.fromJson(baseJson);
      expect(s.createdAt, DateTime.parse('2024-01-15T10:00:00.000Z'));
      expect(s.updatedAt, DateTime.parse('2024-01-15T10:00:01.000Z'));
    });

    test('title defaults to empty when absent', () {
      final json = Map<String, dynamic>.from(baseJson)..remove('title');
      expect(Session.fromJson(json).title, '');
    });

    test('messageCount defaults to 0 when absent', () {
      final json = Map<String, dynamic>.from(baseJson)..remove('messageCount');
      expect(Session.fromJson(json).messageCount, 0);
    });

    test('all SessionStatus values parse', () {
      for (final status in SessionStatus.values) {
        final json = Map<String, dynamic>.from(baseJson)
          ..['status'] = status.name;
        expect(Session.fromJson(json).status, status);
      }
    });

    test('unknown status falls back to idle', () {
      final json = Map<String, dynamic>.from(baseJson)
        ..['status'] = 'unknown_future_status';
      expect(Session.fromJson(json).status, SessionStatus.idle);
    });

    test('all ProviderType values parse', () {
      for (final provider in ProviderType.values) {
        final json = Map<String, dynamic>.from(baseJson)
          ..['provider'] = provider.name;
        expect(Session.fromJson(json).provider, provider);
      }
    });

    test('unknown provider falls back to claude', () {
      final json = Map<String, dynamic>.from(baseJson)
        ..['provider'] = 'unknown_provider';
      expect(Session.fromJson(json).provider, ProviderType.claude);
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
      'repoPath': '/home/user/myapp',
      'branch': 'main',
      'ahead': 2,
      'behind': 0,
      'hasConflicts': false,
      'files': [
        {'path': 'lib/main.dart', 'status': 'modified', 'oldPath': null},
        {'path': 'lib/new.dart', 'status': 'untracked', 'oldPath': null},
      ],
    };

    test('parses required fields', () {
      final rs = RepoStatus.fromJson(baseJson);
      expect(rs.repoPath, '/home/user/myapp');
      expect(rs.branch, 'main');
      expect(rs.files.isNotEmpty, isTrue);
      expect(rs.ahead, 2);
      expect(rs.behind, 0);
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
          'status': state.name,
          'oldPath': null,
        };
        expect(FileStatus.fromJson(fileJson).state, state);
      }
    });

    test('FileStatus oldPath parses for moved files', () {
      final fileJson = {
        'path': 'lib/new_name.dart',
        'status': 'modified',
        'oldPath': 'lib/old_name.dart',
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
