import 'package:flutter/material.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';

class ReceiveScreen extends StatelessWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "receive";

  const ReceiveScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text("Receive")),
      body: const SafeArea(child: Text("Receive Screen")),
    );
  }
}
