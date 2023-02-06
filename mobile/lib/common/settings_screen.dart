import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/settings_screen.dart';
import 'package:get_10101/features/wallet/settings_screen.dart';

class SettingsScreen extends StatelessWidget {
  const SettingsScreen({required this.fromRoute, super.key});

  final String fromRoute;

  @override
  Widget build(BuildContext context) {
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
        const Text("App Info")
      ])),
    );
  }
}
