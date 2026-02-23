import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';

/// Thread view — shows the control thread (persistent management conversation)
/// and a list of active task threads.
class ThreadView extends ConsumerStatefulWidget {
  const ThreadView({super.key});

  @override
  ConsumerState<ThreadView> createState() => _ThreadViewState();
}

class _ThreadViewState extends ConsumerState<ThreadView>
    with SingleTickerProviderStateMixin {
  late TabController _tabController;

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 2, vsync: this);
  }

  @override
  void dispose() {
    _tabController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // ── Header + tabs ────────────────────────────────────────────────────
        Container(
          decoration: const BoxDecoration(
            color: ClawdTheme.surfaceElevated,
            border: Border(bottom: BorderSide(color: ClawdTheme.surfaceBorder)),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Padding(
                padding: EdgeInsets.fromLTRB(20, 14, 20, 0),
                child: Row(
                  children: [
                    Icon(Icons.forum_outlined, size: 16,
                        color: ClawdTheme.clawLight),
                    SizedBox(width: 8),
                    Text(
                      'Threads',
                      style: TextStyle(
                        fontSize: 16,
                        fontWeight: FontWeight.w700,
                        color: Colors.white,
                      ),
                    ),
                  ],
                ),
              ),
              TabBar(
                controller: _tabController,
                tabs: const [
                  Tab(text: 'Control'),
                  Tab(text: 'Task Threads'),
                ],
                labelColor: ClawdTheme.clawLight,
                unselectedLabelColor: Colors.white54,
                indicatorColor: ClawdTheme.claw,
                labelStyle: const TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                ),
                unselectedLabelStyle: const TextStyle(fontSize: 12),
              ),
            ],
          ),
        ),

        // ── Tab content ──────────────────────────────────────────────────────
        Expanded(
          child: TabBarView(
            controller: _tabController,
            children: const [
              _ControlThreadTab(),
              _TaskThreadsTab(),
            ],
          ),
        ),
      ],
    );
  }
}

// ── Control thread tab ─────────────────────────────────────────────────────────

class _ControlThreadTab extends ConsumerWidget {
  const _ControlThreadTab();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // The control thread is the active session's message stream.
    final session = ref.watch(activeSessionProvider);

    if (session == null) {
      return const EmptyState(
        icon: Icons.chat_bubble_outline,
        title: 'No active session',
        subtitle: 'Select a session to see the control thread.',
      );
    }

    final messagesAsync = ref.watch(messageListProvider(session.id));

    return messagesAsync.when(
      loading: () => const Center(
        child: CircularProgressIndicator(color: ClawdTheme.claw),
      ),
      error: (e, _) => ErrorState(
        icon: Icons.error_outline,
        title: 'Failed to load messages',
        description: e.toString(),
        onRetry: () => ref.refresh(messageListProvider(session.id)),
      ),
      data: (messages) {
        if (messages.isEmpty) {
          return const EmptyState(
            icon: Icons.chat_bubble_outline,
            title: 'No messages yet',
            subtitle: 'The control thread will show the agent conversation.',
          );
        }
        return ListView.builder(
          reverse: true,
          padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 12),
          itemCount: messages.length,
          itemBuilder: (context, i) {
            final msg = messages[messages.length - 1 - i];
            return ChatBubble(message: msg);
          },
        );
      },
    );
  }
}

// ── Task threads tab ───────────────────────────────────────────────────────────

class _TaskThreadsTab extends ConsumerWidget {
  const _TaskThreadsTab();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tasksAsync = ref.watch(taskListProvider(const TaskFilter()));
    final agentsAsync = ref.watch(agentsProvider);

    return tasksAsync.when(
      loading: () => const Center(
        child: CircularProgressIndicator(color: ClawdTheme.claw),
      ),
      error: (e, _) => ErrorState(
        icon: Icons.error_outline,
        title: 'Failed to load task threads',
        description: e.toString(),
        onRetry: () => ref.refresh(taskListProvider(const TaskFilter())),
      ),
      data: (tasks) {
        // Show only active tasks (in progress or pending).
        final active = tasks
            .where((t) =>
                t.status == TaskStatus.inProgress ||
                t.status == TaskStatus.pending ||
                t.status == TaskStatus.inQa)
            .toList();

        if (active.isEmpty) {
          return const EmptyState(
            icon: Icons.forum_outlined,
            title: 'No active task threads',
            subtitle: 'Task threads appear here when agents are working.',
          );
        }

        final agents = agentsAsync.valueOrNull ?? [];

        return ListView.builder(
          padding: const EdgeInsets.symmetric(vertical: 8),
          itemCount: active.length,
          itemBuilder: (context, i) {
            final task = active[i];
            final taskAgents = agents.where((a) => a.taskId == task.id).toList();
            return _TaskThreadRow(task: task, agents: taskAgents);
          },
        );
      },
    );
  }
}

class _TaskThreadRow extends StatelessWidget {
  const _TaskThreadRow({required this.task, required this.agents});
  final AgentTask task;
  final List<AgentRecord> agents;

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: ClawdTheme.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: ClawdTheme.surfaceBorder),
      ),
      child: Row(
        children: [
          TaskStatusBadge(status: task.status),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  task.title,
                  style: const TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                    color: Colors.white,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
                if (agents.isNotEmpty) ...[
                  const SizedBox(height: 3),
                  Wrap(
                    spacing: 4,
                    children: agents
                        .map(
                          (a) => Container(
                            padding: const EdgeInsets.symmetric(
                                horizontal: 5, vertical: 1),
                            decoration: BoxDecoration(
                              color: ClawdTheme.claw.withValues(alpha: 0.15),
                              borderRadius: BorderRadius.circular(3),
                            ),
                            child: Text(
                              a.role.displayName,
                              style: const TextStyle(
                                fontSize: 9,
                                color: ClawdTheme.clawLight,
                              ),
                            ),
                          ),
                        )
                        .toList(),
                  ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}
