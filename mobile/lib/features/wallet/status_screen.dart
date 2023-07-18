import 'package:flutter/material.dart';
import 'package:get_10101/common/status_screen.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';

class WalletStatusScreen extends StatelessWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "status";

  const WalletStatusScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return const StatusScreen(fromRoute: route);
  }
}
