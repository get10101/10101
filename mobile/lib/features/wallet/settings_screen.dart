import 'package:flutter/material.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/common/settings_screen.dart';

class WalletSettingsScreen extends StatelessWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "settings";

  const WalletSettingsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return const SettingsScreen(fromRoute: route);
  }
}
