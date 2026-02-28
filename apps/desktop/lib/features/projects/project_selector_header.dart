import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde/features/projects/new_project_dialog.dart';

/// Project selector shown at the top of the NavigationRail.
///
/// Shows a compact dropdown when projects exist, or a "New Project" button
/// when no projects have been created yet.
class ProjectSelectorHeader extends ConsumerWidget {
  const ProjectSelectorHeader({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final projectsAsync = ref.watch(projectListProvider);
    final activeProject = ref.watch(activeProjectProvider);

    return projectsAsync.when(
      data: (projects) => _buildSelector(context, ref, projects, activeProject),
      loading: () => const SizedBox(height: 48),
      error: (_, __) => const SizedBox(height: 48),
    );
  }

  Widget _buildSelector(
    BuildContext context,
    WidgetRef ref,
    List<Project> projects,
    Project? active,
  ) {
    if (projects.isEmpty) {
      return Padding(
        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 6),
        child: Tooltip(
          message: 'New Project',
          child: InkWell(
            onTap: () => _showNewProjectDialog(context, ref),
            borderRadius: BorderRadius.circular(6),
            child: Container(
              height: 36,
              decoration: BoxDecoration(
                color: ClawdTheme.claw.withValues(alpha: 0.12),
                borderRadius: BorderRadius.circular(6),
                border: Border.all(
                  color: ClawdTheme.claw.withValues(alpha: 0.3),
                  style: BorderStyle.solid,
                ),
              ),
              child: const Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Icon(Icons.add, size: 14, color: ClawdTheme.clawLight),
                  SizedBox(width: 4),
                  Text(
                    'New',
                    style: TextStyle(
                      fontSize: 11,
                      color: ClawdTheme.clawLight,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      );
    }

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 6),
      child: PopupMenuButton<String>(
        tooltip: active?.name ?? 'Select project',
        color: ClawdTheme.surfaceElevated,
        offset: const Offset(72, 0),
        constraints: const BoxConstraints(minWidth: 200, maxWidth: 280),
        itemBuilder: (_) => [
          // Project items
          ...projects.map(
            (p) => PopupMenuItem<String>(
              value: p.id,
              height: 36,
              child: Row(
                children: [
                  Icon(
                    Icons.folder,
                    size: 14,
                    color: p.id == active?.id ? ClawdTheme.clawLight : Colors.white54,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      p.name,
                      style: TextStyle(
                        fontSize: 13,
                        color: p.id == active?.id ? ClawdTheme.clawLight : Colors.white,
                        fontWeight: p.id == active?.id ? FontWeight.w600 : FontWeight.normal,
                        overflow: TextOverflow.ellipsis,
                      ),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  if (p.id == active?.id)
                    const Icon(Icons.check, size: 12, color: ClawdTheme.clawLight),
                ],
              ),
            ),
          ),
          // Divider
          const PopupMenuDivider(),
          // New project
          const PopupMenuItem<String>(
            value: '__new__',
            height: 36,
            child: Row(
              children: [
                Icon(Icons.add, size: 14, color: Colors.white54),
                SizedBox(width: 8),
                Text(
                  'New project',
                  style: TextStyle(fontSize: 13, color: Colors.white70),
                ),
              ],
            ),
          ),
        ],
        onSelected: (id) {
          if (id == '__new__') {
            _showNewProjectDialog(context, ref);
          } else {
            ref.read(activeProjectIdProvider.notifier).state = id;
          }
        },
        child: Container(
          height: 36,
          padding: const EdgeInsets.symmetric(horizontal: 8),
          decoration: BoxDecoration(
            color: ClawdTheme.surfaceElevated,
            borderRadius: BorderRadius.circular(6),
            border: Border.all(color: ClawdTheme.surfaceBorder),
          ),
          child: Row(
            children: [
              Icon(
                active != null ? Icons.folder : Icons.folder_open,
                size: 13,
                color: active != null ? ClawdTheme.clawLight : Colors.white38,
              ),
              const SizedBox(width: 6),
              Expanded(
                child: Text(
                  active?.name ?? 'Project',
                  style: TextStyle(
                    fontSize: 11,
                    color: active != null ? Colors.white : Colors.white38,
                    fontWeight: FontWeight.w500,
                    overflow: TextOverflow.ellipsis,
                  ),
                  overflow: TextOverflow.ellipsis,
                  maxLines: 1,
                ),
              ),
              if (active?.rootPath case final path?) ...[
                DoctorBadge(projectPath: path),
                const SizedBox(width: 2),
              ],
              const Icon(Icons.arrow_drop_down, size: 14, color: Colors.white38),
            ],
          ),
        ),
      ),
    );
  }

  void _showNewProjectDialog(BuildContext context, WidgetRef ref) {
    showDialog<void>(
      context: context,
      builder: (_) => NewProjectDialog(
        onCreated: (project) {
          ref.read(activeProjectIdProvider.notifier).state = project.id;
        },
      ),
    );
  }
}
