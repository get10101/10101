import 'package:flutter/material.dart';
import 'package:get_10101/common/status_screen.dart';
import 'package:get_10101/features/trade/trade_screen.dart';

class TradeStatusScreen extends StatelessWidget {
  static const route = "${TradeScreen.route}/$subRouteName";
  static const subRouteName = "status";

  const TradeStatusScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return const StatusScreen(fromRoute: route);
  }
}
