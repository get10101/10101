import 'package:flutter/material.dart';
import 'package:get_10101/features/stable/stable_screen.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/features/welcome/welcome_screen.dart';
import 'package:get_10101/util/preferences.dart';
import 'package:go_router/go_router.dart';

class LoadingScreen extends StatefulWidget {
  static const route = "/loading";

  const LoadingScreen({super.key});

  @override
  State<LoadingScreen> createState() => _LoadingScreenState();
}

class _LoadingScreenState extends State<LoadingScreen> {
  @override
  void initState() {
    super.initState();
    Future.wait<dynamic>(
            [Preferences.instance.hasEmailAddress(), Preferences.instance.getOpenPosition()])
        .then((value) {
      final bool hasEmailAddress = value[0];
      final String? position = value[1];

      if (!hasEmailAddress) {
        GoRouter.of(context).go(WelcomeScreen.route);
      } else {
        switch (position) {
          case StableScreen.label:
            GoRouter.of(context).go(StableScreen.route);
          case TradeScreen.label:
            GoRouter.of(context).go(TradeScreen.route);
          default:
            GoRouter.of(context).go(WalletScreen.route);
        }
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    return const Center(child: CircularProgressIndicator());
  }
}
