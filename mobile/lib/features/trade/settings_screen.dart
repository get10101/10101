import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/trade_screen.dart';

class TradeSettingsScreen extends StatelessWidget {
  static const route = "${TradeScreen.route}/$subRouteName";
  static const subRouteName = "settings";

  const TradeSettingsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text("Trade Settings")),
      body: const SafeArea(child: Text("Settings")),
    );
  }
}
