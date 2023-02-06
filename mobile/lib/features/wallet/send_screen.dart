import 'package:flutter/material.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';

class SendScreen extends StatelessWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "send";

  const SendScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text("Send")),
      body: const SafeArea(child: Text("Send Screen")),
    );
  }
}
