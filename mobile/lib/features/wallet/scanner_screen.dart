import 'package:flutter/material.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';

class ScannerScreen extends StatelessWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "scanner";

  const ScannerScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text("Scanner")),
      body: const SafeArea(child: Text("Scanner Screen")),
    );
  }
}
