import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/trade_screen.dart';

import '../../common/settings_screen.dart';

class TradeSettingsScreen extends StatelessWidget {
  static const route = "${TradeScreen.route}/$subRouteName";
  static const subRouteName = "settings";

  const TradeSettingsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return const SettingsScreen(fromRoute: route);
  }
}
