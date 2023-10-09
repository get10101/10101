import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:get_10101/common/channel_status_notifier.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/logger/hybrid_logger.dart';
import 'package:intl/intl.dart';
import 'package:path_provider/path_provider.dart';
import 'package:provider/provider.dart';
import 'package:share_plus/share_plus.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/logger/logger.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  String _buildNumber = '';
  String _version = '';
  String _nodeId = "";

  // Variable preventing the user from spamming the close channel buttons
  bool _isCloseChannelButtonDisabled = false;

  @override
  void initState() {
    try {
      var nodeId = rust.api.getNodeId();
      _nodeId = nodeId;
    } catch (e) {
      logger.e("Error getting node id: $e");
      _nodeId = "UNKNOWN";
    }

    loadValues();
    super.initState();
  }

  Future<void> loadValues() async {
    var value = await PackageInfo.fromPlatform();

    logger.i("All values $value");
    setState(() {
      _buildNumber = value.buildNumber;
      _version = value.version;
    });
  }

  @override
  Widget build(BuildContext context) {
    final bridge.Config config = context.read<bridge.Config>();

    String commit = const String.fromEnvironment('COMMIT', defaultValue: 'not available');
    String branch = const String.fromEnvironment('BRANCH', defaultValue: 'not available');

    return Scaffold(
      appBar: AppBar(title: const Text("Settings")),
      body: ScrollableSafeArea(
          child: Column(children: [
        ElevatedButton(
            onPressed: _isCloseChannelButtonDisabled
                ? null
                : () async {
                    setState(() {
                      _isCloseChannelButtonDisabled = true;
                    });
                    final messenger = ScaffoldMessenger.of(context);
                    try {
                      ensureCanCloseChannel(context);
                      await rust.api.closeChannel();
                    } catch (e) {
                      showSnackBar(messenger, e.toString());
                    } finally {
                      setState(() {
                        _isCloseChannelButtonDisabled = false;
                      });
                    }
                  },
            child: const Text("Close channel")),
        Visibility(
          visible: config.network == "regtest",
          child: Column(
            children: [
              ElevatedButton(
                  onPressed: _isCloseChannelButtonDisabled
                      ? null
                      : () async {
                          setState(() {
                            _isCloseChannelButtonDisabled = true;
                          });
                          final messenger = ScaffoldMessenger.of(context);
                          try {
                            ensureCanCloseChannel(context);
                            await rust.api.forceCloseChannel();
                          } catch (e) {
                            showSnackBar(messenger, e.toString());
                          } finally {
                            setState(() {
                              _isCloseChannelButtonDisabled = false;
                            });
                          }
                        },
                  child: const Text("Force-close channel")),
            ],
          ),
        ),
        const Divider(),
        const Text("App Info"),
        Table(
          border: TableBorder.symmetric(inside: const BorderSide(width: 1)),
          children: [
            TableRow(
              children: [
                const Center(
                  child: Text('Esplora'),
                ),
                Center(
                  child: SelectableText(config.esploraEndpoint),
                ),
              ],
            ),
            TableRow(
              children: [
                const Center(
                  child: Text('Network'),
                ),
                Center(
                  child: SelectableText(config.network),
                ),
              ],
            ),
            TableRow(
              children: [
                const Center(
                  child: Text('Coordinator'),
                ),
                Center(
                  child: SelectableText(
                      "${config.coordinatorPubkey}@${config.host}:${config.p2PPort}"),
                ),
              ],
            ),
            TableRow(
              children: [
                const Center(
                  child: Text('Branch'),
                ),
                Center(
                  child: SelectableText(branch),
                ),
              ],
            ),
            TableRow(
              children: [
                const Center(
                  child: Text('Commit'),
                ),
                Center(
                  child: SelectableText(commit),
                ),
              ],
            ),
            TableRow(
              children: [
                const Center(
                  child: Text('Build Number'),
                ),
                Center(
                  child: SelectableText(_buildNumber),
                ),
              ],
            ),
            TableRow(
              children: [
                const Center(
                  child: Text('Build Version'),
                ),
                Center(
                  child: SelectableText(_version),
                ),
              ],
            ),
            TableRow(
              children: [
                const Center(
                  child: Text('Node ID'),
                ),
                Center(
                  child: SelectableText(_nodeId),
                ),
              ],
            )
          ],
        ),
        ElevatedButton(
            onPressed: () async {
              // TODO:
              logger.toString();

              // TODO: fix me
              var file = await HybridOutput.logFilePath();
              var logsAsString = await file.readAsString();
              final List<int> bytes = utf8.encode(logsAsString);
              final Directory tempDir = await getTemporaryDirectory();
              String now = DateFormat('yyyy-MM-dd_HHmmss').format(DateTime.now());
              final String filePath = '${tempDir.path}/$now.log';
              await File(filePath).writeAsBytes(bytes);
              final XFile logFile = XFile(filePath);
              Share.shareXFiles([logFile], text: 'Logs from $now');
            },
            child: const Text("Share logs")),
      ])),
    );
  }
}

/// Throws if the channel is not in a state where it can be closed.
void ensureCanCloseChannel(BuildContext context) {
  if (context.read<PositionChangeNotifier>().positions.isNotEmpty) {
    throw Exception("In order to close your Lighting Channel you need to close all your positions");
  }
  if (context.read<ChannelStatusNotifier>().isClosing()) {
    throw Exception("Your channel is already closing");
  }
}
