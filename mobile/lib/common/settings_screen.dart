import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/settings_screen.dart';
import 'package:get_10101/features/wallet/settings_screen.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:provider/provider.dart';

class SettingsScreen extends StatelessWidget {
  const SettingsScreen({required this.fromRoute, super.key});

  final String fromRoute;

  @override
  Widget build(BuildContext context) {
    final bridge.Config config = context.read<bridge.Config>();

    return Scaffold(
      appBar: AppBar(title: const Text("Settings")),
      body: SafeArea(
          child: Column(children: [
        Text(
          "Wallet Settings",
          style: TextStyle(
              fontWeight:
                  fromRoute == WalletSettingsScreen.route ? FontWeight.bold : FontWeight.normal),
        ),
        const Divider(),
        Text("Trade Settings",
            style: TextStyle(
                fontWeight:
                    fromRoute == TradeSettingsScreen.route ? FontWeight.bold : FontWeight.normal)),
        const Divider(),
        const Text("App Info"),
        Table(
          border: TableBorder.symmetric(inside: const BorderSide(width: 1)),
          children: [
            TableRow(
              children: [
                const Center(
                  child: Text('Electrum'),
                ),
                Center(
                  child: SelectableText(config.electrsEndpoint),
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
          ],
        ),
      ])),
    );
  }
}
