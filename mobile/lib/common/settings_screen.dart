import 'dart:convert';
import 'dart:io';

import 'package:f_logs/f_logs.dart';
import 'package:feedback/feedback.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/application/config_service.dart';
import 'package:get_10101/common/feedback.dart';
import 'package:get_10101/common/network_toggle_button.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:intl/intl.dart';
import 'package:path_provider/path_provider.dart';
import 'package:provider/provider.dart';
import 'package:share_plus/share_plus.dart';
import 'package:get_10101/ffi.dart' as rust;

class SettingsScreen extends StatefulWidget {
  static const subRouteName = "settings";

  const SettingsScreen({super.key});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  String _nodeId = "";

  @override
  void initState() {
    var nodeId = rust.api.getNodeId();
    _nodeId = nodeId;
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    final configService = context.watch<ConfigService>();
    final config = configService.getConfig();
    final devMode = configService.devMode;

    return Scaffold(
      appBar: AppBar(title: const Text("Settings")),
      body: ScrollableSafeArea(
          child: Column(children: [
        Visibility(
          visible: config.network == "regtest",
          child: Column(
            children: [
              ElevatedButton(
                  onPressed: () {
                    rust.api.closeChannel();
                  },
                  child: const Text("Close channel")),
              ElevatedButton(
                  onPressed: () {
                    rust.api.forceCloseChannel();
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
                  child: SelectableText(config.network + (devMode ? " (dev)" : "")),
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
                  child: SelectableText(configService.branch),
                ),
              ],
            ),
            TableRow(
              children: [
                const Center(
                  child: Text('Commit'),
                ),
                Center(
                  child: SelectableText(configService.commit),
                ),
              ],
            ),
            TableRow(
              children: [
                const Center(
                  child: Text('Build Number'),
                ),
                Center(
                  child: SelectableText(configService.buildNumber),
                ),
              ],
            ),
            TableRow(
              children: [
                const Center(
                  child: Text('Build Version'),
                ),
                Center(
                  child: SelectableText(configService.version),
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
              var file = await FLog.exportLogs();
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
        const SizedBox(height: 10),
        ElevatedButton(
          child: const Text('Provide feedback'),
          onPressed: () {
            try {
              BetterFeedback.of(context).show(submitFeedback);
            } on Exception catch (e) {
              ScaffoldMessenger.of(context).showSnackBar(
                  SnackBar(content: Text('Failed to share feedback via email app because: $e')));
            }
          },
        ),
        const SizedBox(height: 10),
        const NetworkToggleButton(),
      ])),
    );
  }
}
