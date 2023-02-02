import 'package:flutter/material.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';

class WalletSettingsScreen extends StatelessWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "settings";

  const WalletSettingsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text("Wallet Settings")),
      body: const SafeArea(child: Text("Settings")),
    );
  }
}
