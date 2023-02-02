import 'package:flutter/material.dart';
import 'package:get_10101/features/wallet/receive_screen.dart';
import 'package:get_10101/features/wallet/settings_screen.dart';
import 'package:go_router/go_router.dart';

class WalletScreen extends StatefulWidget {
  static const route = "/wallet";
  static const label = "Wallet";

  const WalletScreen({Key? key}) : super(key: key);

  @override
  State<WalletScreen> createState() => _WalletScreenState();
}

class _WalletScreenState extends State<WalletScreen> {
  @override
  Widget build(BuildContext context) {
    return Scaffold(
        body: ListView(
      padding: const EdgeInsets.only(left: 25, right: 25),
      children: [
        const Center(child: Text("Wallet Screen")),
        ElevatedButton(
          onPressed: () {
            context.go(ReceiveScreen.route);
          },
          child: const Text("Fund Wallet"),
        ),
        ElevatedButton(
          onPressed: () {
            context.go(WalletSettingsScreen.route);
          },
          child: const Text("Settings"),
        )
      ],
    ));
  }
}
