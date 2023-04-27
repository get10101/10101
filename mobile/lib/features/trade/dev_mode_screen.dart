import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/common/dev_mode/dev_mode_screen.dart';

class TradeDevModeScreen extends StatelessWidget {
  static const route = "${TradeScreen.route}/$subRouteName";
  static const subRouteName = "dev_mode";

  const TradeDevModeScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return const DevModeScreen(fromRoute: route);
  }
}
