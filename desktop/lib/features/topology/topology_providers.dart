/// Riverpod providers for the Multi-Repo Topology subsystem (Sprint N, MR.T05).
///
/// Wraps the `topology.get`, `topology.addDependency`, and
/// `topology.removeDependency` RPC methods.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';

// ─── Model helpers ────────────────────────────────────────────────────────────

/// A repository node in the topology graph.
class RepoNode {
  const RepoNode({
    required this.path,
    required this.name,
    required this.healthScore,
    required this.stack,
  });

  final String path;
  final String name;
  final int healthScore;
  final List<String> stack;

  factory RepoNode.fromJson(Map<String, dynamic> json) => RepoNode(
        path:        json['path'] as String? ?? '',
        name:        json['name'] as String? ?? '',
        healthScore: (json['healthScore'] as num?)?.toInt() ?? 100,
        stack:       (json['stack'] as List<dynamic>?)
                         ?.map((e) => e.toString())
                         .toList() ??
                     const [],
      );
}

/// A directed dependency edge between two repos.
class RepoDependency {
  const RepoDependency({
    required this.id,
    required this.fromRepo,
    required this.toRepo,
    required this.depType,
    required this.confidence,
    required this.autoDetected,
  });

  final String id;
  final String fromRepo;
  final String toRepo;
  final String depType;
  final double confidence;
  final bool autoDetected;

  factory RepoDependency.fromJson(Map<String, dynamic> json) => RepoDependency(
        id:           json['id'] as String? ?? '',
        fromRepo:     json['fromRepo'] as String? ?? '',
        toRepo:       json['toRepo'] as String? ?? '',
        depType:      json['depType'] as String? ?? 'uses_api',
        confidence:   (json['confidence'] as num?)?.toDouble() ?? 1.0,
        autoDetected: json['autoDetected'] as bool? ?? false,
      );
}

/// The full topology graph returned by `topology.get`.
class TopologyGraph {
  const TopologyGraph({required this.nodes, required this.edges});

  final List<RepoNode> nodes;
  final List<RepoDependency> edges;

  factory TopologyGraph.fromJson(Map<String, dynamic> json) => TopologyGraph(
        nodes: (json['nodes'] as List<dynamic>? ?? const [])
            .map((n) => RepoNode.fromJson(n as Map<String, dynamic>))
            .toList(),
        edges: (json['edges'] as List<dynamic>? ?? const [])
            .map((e) => RepoDependency.fromJson(e as Map<String, dynamic>))
            .toList(),
      );

  TopologyGraph get empty => const TopologyGraph(nodes: [], edges: []);
}

// ─── topologyProvider ─────────────────────────────────────────────────────────

/// Fetches and caches the full topology graph from the daemon.
///
/// Invalidate with `ref.invalidate(topologyProvider)` after mutations.
final topologyProvider = FutureProvider<TopologyGraph>((ref) async {
  final client = ref.read(daemonProvider.notifier).client;
  final result = await client.call<Map<String, dynamic>>('topology.get', {});
  return TopologyGraph.fromJson(result);
});

// ─── repoTopologyProvider ─────────────────────────────────────────────────────

/// Returns the subset of topology edges that involve [repoPath] (as source or
/// target).
final repoTopologyProvider =
    FutureProvider.family<List<RepoDependency>, String>((ref, repoPath) async {
  final graph = await ref.watch(topologyProvider.future);
  return graph.edges
      .where((e) => e.fromRepo == repoPath || e.toRepo == repoPath)
      .toList();
});

// ─── TopologyActions ─────────────────────────────────────────────────────────

/// Fire-and-forget topology mutation actions.
final topologyActionsProvider = Provider<TopologyActions>((ref) {
  return TopologyActions(ref);
});

class TopologyActions {
  const TopologyActions(this._ref);
  final Ref _ref;

  /// Manually add a dependency edge between two repos.
  Future<void> addDependency({
    required String fromRepo,
    required String toRepo,
    String depType = 'uses_api',
    double confidence = 1.0,
  }) async {
    final client = _ref.read(daemonProvider.notifier).client;
    await client.call<Map<String, dynamic>>('topology.addDependency', {
      'fromRepo':   fromRepo,
      'toRepo':     toRepo,
      'depType':    depType,
      'confidence': confidence,
    });
    _ref.invalidate(topologyProvider);
  }

  /// Remove a dependency edge by its id.
  Future<void> removeDependency(String id) async {
    final client = _ref.read(daemonProvider.notifier).client;
    await client
        .call<Map<String, dynamic>>('topology.removeDependency', {'id': id});
    _ref.invalidate(topologyProvider);
  }

  /// Run cross-repo validators for the given repo (or all repos if null).
  Future<List<Map<String, dynamic>>> crossValidate([String? repoPath]) async {
    final client = _ref.read(daemonProvider.notifier).client;
    final params = repoPath != null ? {'repoPath': repoPath} : <String, dynamic>{};
    final result = await client.call<List<dynamic>>('topology.crossValidate', params);
    return result.cast<Map<String, dynamic>>();
  }
}
