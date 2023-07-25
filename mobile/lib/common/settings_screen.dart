import 'dart:convert';
import 'dart:io';

import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/features/trade/settings_screen.dart';
import 'package:get_10101/features/wallet/settings_screen.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:intl/intl.dart';
import 'package:path_provider/path_provider.dart';
import 'package:provider/provider.dart';
import 'package:share_plus/share_plus.dart';
import 'package:package_info_plus/package_info_plus.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({required this.fromRoute, super.key});

  final String fromRoute;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  String _buildNumber = '';
  String _version = '';
  String _nodeId = "";

  @override
  void initState() {
    loadValues();
    super.initState();
  }

  Future<void> loadValues() async {
    var value = await PackageInfo.fromPlatform();

    FLog.info(text: "All values $value");
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

    final channelService = context.read<ChannelInfoService>();

    _nodeId = channelService.getNodeId();

    return Scaffold(
      appBar: AppBar(title: const Text("Settings")),
      body: SafeArea(
          child: Column(children: [
        Text(
          "Wallet Settings",
          style: TextStyle(
              fontWeight: widget.fromRoute == WalletSettingsScreen.route
                  ? FontWeight.bold
                  : FontWeight.normal),
        ),
        Visibility(
          visible: config.network == "regtest",
          child: Column(
            children: [
              ElevatedButton(
                  onPressed: () {
                    channelService.closeChannel();
                  },
                  child: const Text("Close channel")),
              ElevatedButton(
                  onPressed: () {
                    channelService.forceCloseChannel();
                  },
                  child: const Text("Force-close channel")),
            ],
          ),
        ),
        const Divider(),
        Text("Trade Settings",
            style: TextStyle(
                fontWeight: widget.fromRoute == TradeSettingsScreen.route
                    ? FontWeight.bold
                    : FontWeight.normal)),
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
      ])),
    );
  }
}
