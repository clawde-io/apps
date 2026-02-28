import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:clawd_core/clawd_core.dart';
import 'package:clawd_proto/clawd_proto.dart';
import 'package:clawd_ui/clawd_ui.dart';
import 'package:clawde/router.dart';
import 'package:clawde/features/chat/widgets/new_session_dialog.dart';

/// A single command exposed in the palette.
class _PaletteCommand {
  const _PaletteCommand({
    required this.label,
    required this.icon,
    this.shortcut,
    required this.onExecute,
    this.category = 'General',
  });

  final String label;
  final IconData icon;
  final String? shortcut;
  final VoidCallback onExecute;
  final String category;
}

/// Shows the command palette as a centered top overlay (VS Code style).
///
/// Call [showCommandPalette] from anywhere to display it.
Future<void> showCommandPalette(BuildContext context, WidgetRef ref) {
  return showDialog<void>(
    context: context,
    barrierColor: Colors.black54,
    builder: (_) => _CommandPaletteDialog(parentContext: context, ref: ref),
  );
}

class _CommandPaletteDialog extends StatefulWidget {
  const _CommandPaletteDialog({
    required this.parentContext,
    required this.ref,
  });

  final BuildContext parentContext;
  final WidgetRef ref;

  @override
  State<_CommandPaletteDialog> createState() => _CommandPaletteDialogState();
}

class _CommandPaletteDialogState extends State<_CommandPaletteDialog> {
  final _controller = TextEditingController();
  final _focusNode = FocusNode();
  int _selectedIndex = 0;

  late final List<_PaletteCommand> _allCommands;

  @override
  void initState() {
    super.initState();
    _allCommands = _buildCommands();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _focusNode.requestFocus();
    });
  }

  @override
  void dispose() {
    _controller.dispose();
    _focusNode.dispose();
    super.dispose();
  }

  List<_PaletteCommand> _buildCommands() {
    final nav = widget.parentContext;
    final ref = widget.ref;

    return [
      // ── Sessions ─────────────────────────────────────────────────────────
      _PaletteCommand(
        label: 'New Session',
        icon: Icons.add,
        shortcut: '\u2318N',
        category: 'Sessions',
        onExecute: () {
          Navigator.of(context).pop();
          showDialog<void>(
            context: nav,
            builder: (_) => const NewSessionDialog(),
          );
        },
      ),
      _PaletteCommand(
        label: 'Pause Current Session',
        icon: Icons.pause,
        shortcut: '\u2318P',
        category: 'Sessions',
        onExecute: () {
          Navigator.of(context).pop();
          final sessionId = ref.read(activeSessionIdProvider);
          if (sessionId != null) {
            ref.read(sessionListProvider.notifier).pause(sessionId);
          }
        },
      ),
      _PaletteCommand(
        label: 'Resume Current Session',
        icon: Icons.play_arrow,
        category: 'Sessions',
        onExecute: () {
          Navigator.of(context).pop();
          final sessionId = ref.read(activeSessionIdProvider);
          if (sessionId != null) {
            ref.read(sessionListProvider.notifier).resume(sessionId);
          }
        },
      ),
      _PaletteCommand(
        label: 'Cancel Current Generation',
        icon: Icons.stop,
        shortcut: '\u2318.',
        category: 'Sessions',
        onExecute: () {
          Navigator.of(context).pop();
          final sessionId = ref.read(activeSessionIdProvider);
          if (sessionId != null) {
            ref.read(sessionListProvider.notifier).cancel(sessionId);
          }
        },
      ),
      _PaletteCommand(
        label: 'Close Current Session',
        icon: Icons.close,
        shortcut: '\u2318W',
        category: 'Sessions',
        onExecute: () {
          Navigator.of(context).pop();
          final sessionId = ref.read(activeSessionIdProvider);
          if (sessionId != null) {
            ref.read(sessionListProvider.notifier).close(sessionId);
            ref.read(activeSessionIdProvider.notifier).state = null;
          }
        },
      ),

      // ── Navigation ───────────────────────────────────────────────────────
      _PaletteCommand(
        label: 'Go to Chat',
        icon: Icons.chat_bubble_outline,
        shortcut: '\u23181',
        category: 'Navigation',
        onExecute: () {
          Navigator.of(context).pop();
          GoRouter.of(nav).go(routeChat);
        },
      ),
      _PaletteCommand(
        label: 'Go to Sessions',
        icon: Icons.layers_outlined,
        shortcut: '\u23182',
        category: 'Navigation',
        onExecute: () {
          Navigator.of(context).pop();
          GoRouter.of(nav).go(routeSessions);
        },
      ),
      _PaletteCommand(
        label: 'Go to Files',
        icon: Icons.folder_outlined,
        shortcut: '\u23183',
        category: 'Navigation',
        onExecute: () {
          Navigator.of(context).pop();
          GoRouter.of(nav).go(routeFiles);
        },
      ),
      _PaletteCommand(
        label: 'Go to Git',
        icon: Icons.account_tree_outlined,
        shortcut: '\u23184',
        category: 'Navigation',
        onExecute: () {
          Navigator.of(context).pop();
          GoRouter.of(nav).go(routeGit);
        },
      ),
      _PaletteCommand(
        label: 'Go to Tasks',
        icon: Icons.view_kanban_outlined,
        shortcut: '\u23185',
        category: 'Navigation',
        onExecute: () {
          Navigator.of(context).pop();
          GoRouter.of(nav).go(routeDashboard);
        },
      ),
      _PaletteCommand(
        label: 'Go to Search',
        icon: Icons.search,
        shortcut: '\u2318K',
        category: 'Navigation',
        onExecute: () {
          Navigator.of(context).pop();
          GoRouter.of(nav).go(routeSearch);
        },
      ),
      _PaletteCommand(
        label: 'Go to Packs',
        icon: Icons.extension_outlined,
        category: 'Navigation',
        onExecute: () {
          Navigator.of(context).pop();
          GoRouter.of(nav).go(routePacks);
        },
      ),
      _PaletteCommand(
        label: 'Go to Settings',
        icon: Icons.settings_outlined,
        category: 'Navigation',
        onExecute: () {
          Navigator.of(context).pop();
          GoRouter.of(nav).go(routeSettings);
        },
      ),

      // ── Providers ────────────────────────────────────────────────────────
      _PaletteCommand(
        label: 'Switch to Claude',
        icon: Icons.auto_awesome,
        category: 'Providers',
        onExecute: () {
          Navigator.of(context).pop();
          ref.read(settingsProvider.notifier).setDefaultProvider(
                ProviderType.claude,
              );
        },
      ),
      _PaletteCommand(
        label: 'Switch to Codex',
        icon: Icons.auto_awesome,
        category: 'Providers',
        onExecute: () {
          Navigator.of(context).pop();
          ref.read(settingsProvider.notifier).setDefaultProvider(
                ProviderType.codex,
              );
        },
      ),
      _PaletteCommand(
        label: 'Switch to Cursor',
        icon: Icons.auto_awesome,
        category: 'Providers',
        onExecute: () {
          Navigator.of(context).pop();
          ref.read(settingsProvider.notifier).setDefaultProvider(
                ProviderType.cursor,
              );
        },
      ),

      // ── Daemon ───────────────────────────────────────────────────────────
      _PaletteCommand(
        label: 'Reconnect Daemon',
        icon: Icons.refresh,
        category: 'Daemon',
        onExecute: () {
          Navigator.of(context).pop();
          ref.read(daemonProvider.notifier).reconnect();
        },
      ),
    ];
  }

  List<_PaletteCommand> get _filtered {
    final query = _controller.text.trim().toLowerCase();
    if (query.isEmpty) return _allCommands;
    return _allCommands.where((cmd) {
      final labelLower = cmd.label.toLowerCase();
      final categoryLower = cmd.category.toLowerCase();
      // Substring match.
      if (labelLower.contains(query) || categoryLower.contains(query)) {
        return true;
      }
      // Fuzzy character-order match on label.
      if (query.length >= 2) {
        int qi = 0;
        for (int ti = 0;
            ti < labelLower.length && qi < query.length;
            ti++) {
          if (labelLower[ti] == query[qi]) qi++;
        }
        if (qi == query.length) return true;
      }
      return false;
    }).toList();
  }

  void _execute(_PaletteCommand cmd) {
    cmd.onExecute();
  }

  void _handleKey(KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) return;
    final filtered = _filtered;
    if (event.logicalKey == LogicalKeyboardKey.arrowDown) {
      setState(() {
        _selectedIndex = (_selectedIndex + 1).clamp(0, filtered.length - 1);
      });
    } else if (event.logicalKey == LogicalKeyboardKey.arrowUp) {
      setState(() {
        _selectedIndex = (_selectedIndex - 1).clamp(0, filtered.length - 1);
      });
    } else if (event.logicalKey == LogicalKeyboardKey.enter) {
      if (filtered.isNotEmpty && _selectedIndex < filtered.length) {
        _execute(filtered[_selectedIndex]);
      }
    } else if (event.logicalKey == LogicalKeyboardKey.escape) {
      Navigator.of(context).pop();
    }
  }

  @override
  Widget build(BuildContext context) {
    final filtered = _filtered;
    // Reset index if it's out of bounds after filtering.
    if (_selectedIndex >= filtered.length) {
      _selectedIndex = filtered.isEmpty ? 0 : filtered.length - 1;
    }

    return Align(
      alignment: const Alignment(0, -0.4),
      child: Material(
        color: Colors.transparent,
        child: Container(
          width: 520,
          constraints: const BoxConstraints(maxHeight: 440),
          decoration: BoxDecoration(
            color: ClawdTheme.surfaceElevated,
            borderRadius: BorderRadius.circular(12),
            border: Border.all(color: ClawdTheme.surfaceBorder),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withValues(alpha: 0.5),
                blurRadius: 24,
                offset: const Offset(0, 8),
              ),
            ],
          ),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              // ── Search input ─────────────────────────────────────────────
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 12, 16, 8),
                child: KeyboardListener(
                  focusNode: FocusNode(),
                  onKeyEvent: _handleKey,
                  child: TextField(
                    controller: _controller,
                    focusNode: _focusNode,
                    onChanged: (_) => setState(() => _selectedIndex = 0),
                    style:
                        const TextStyle(fontSize: 14, color: Colors.white),
                    decoration: InputDecoration(
                      hintText: 'Type a command...',
                      hintStyle: TextStyle(
                        fontSize: 14,
                        color: Colors.white.withValues(alpha: 0.3),
                      ),
                      prefixIcon: const Icon(Icons.terminal,
                          size: 18, color: ClawdTheme.clawLight),
                      filled: true,
                      fillColor: ClawdTheme.surface,
                      contentPadding: const EdgeInsets.symmetric(
                          horizontal: 12, vertical: 10),
                      border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(8),
                        borderSide:
                            const BorderSide(color: ClawdTheme.surfaceBorder),
                      ),
                      enabledBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(8),
                        borderSide:
                            const BorderSide(color: ClawdTheme.surfaceBorder),
                      ),
                      focusedBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(8),
                        borderSide:
                            const BorderSide(color: ClawdTheme.claw),
                      ),
                    ),
                  ),
                ),
              ),

              const Divider(height: 1, color: ClawdTheme.surfaceBorder),

              // ── Results ──────────────────────────────────────────────────
              if (filtered.isEmpty)
                Padding(
                  padding: const EdgeInsets.all(24),
                  child: Text(
                    'No matching commands',
                    style: TextStyle(
                      fontSize: 13,
                      color: Colors.white.withValues(alpha: 0.4),
                    ),
                  ),
                )
              else
                Flexible(
                  child: ListView.builder(
                    shrinkWrap: true,
                    padding: const EdgeInsets.symmetric(vertical: 4),
                    itemCount: filtered.length,
                    itemBuilder: (context, i) {
                      final cmd = filtered[i];
                      final isSelected = i == _selectedIndex;
                      // Show category header if it differs from the previous.
                      final showCategory = i == 0 ||
                          filtered[i - 1].category != cmd.category;

                      return Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          if (showCategory)
                            Padding(
                              padding: const EdgeInsets.fromLTRB(
                                  16, 8, 16, 4),
                              child: Text(
                                cmd.category,
                                style: TextStyle(
                                  fontSize: 10,
                                  fontWeight: FontWeight.w700,
                                  letterSpacing: 0.8,
                                  color:
                                      Colors.white.withValues(alpha: 0.3),
                                ),
                              ),
                            ),
                          InkWell(
                            onTap: () => _execute(cmd),
                            onHover: (hovering) {
                              if (hovering) {
                                setState(() => _selectedIndex = i);
                              }
                            },
                            child: Container(
                              height: 36,
                              margin: const EdgeInsets.symmetric(
                                  horizontal: 8, vertical: 1),
                              padding: const EdgeInsets.symmetric(
                                  horizontal: 12),
                              decoration: BoxDecoration(
                                color: isSelected
                                    ? ClawdTheme.claw
                                        .withValues(alpha: 0.15)
                                    : Colors.transparent,
                                borderRadius: BorderRadius.circular(6),
                              ),
                              child: Row(
                                children: [
                                  Icon(
                                    cmd.icon,
                                    size: 16,
                                    color: isSelected
                                        ? ClawdTheme.clawLight
                                        : Colors.white54,
                                  ),
                                  const SizedBox(width: 10),
                                  Expanded(
                                    child: Text(
                                      cmd.label,
                                      style: TextStyle(
                                        fontSize: 13,
                                        color: isSelected
                                            ? Colors.white
                                            : Colors.white70,
                                      ),
                                    ),
                                  ),
                                  if (cmd.shortcut != null)
                                    Container(
                                      padding: const EdgeInsets.symmetric(
                                          horizontal: 6, vertical: 2),
                                      decoration: BoxDecoration(
                                        color: Colors.white
                                            .withValues(alpha: 0.06),
                                        borderRadius:
                                            BorderRadius.circular(4),
                                      ),
                                      child: Text(
                                        cmd.shortcut!,
                                        style: TextStyle(
                                          fontSize: 11,
                                          fontFamily: 'monospace',
                                          color: Colors.white
                                              .withValues(alpha: 0.4),
                                        ),
                                      ),
                                    ),
                                ],
                              ),
                            ),
                          ),
                        ],
                      );
                    },
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}
