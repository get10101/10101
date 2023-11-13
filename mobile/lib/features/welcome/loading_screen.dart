import 'package:flutter/material.dart';
import 'package:get_10101/backend.dart';
import 'package:get_10101/features/welcome/onboarding.dart';
import 'package:get_10101/features/stable/stable_screen.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/util/preferences.dart';
import 'package:get_10101/util/file.dart';
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
    Future.wait<dynamic>([
      Preferences.instance.getOpenPosition(),
      isSeedFilePresent(),
    ]).then((value) {
      final position = value[0];
      final isSeedFilePresent = value[1];

      logger.d("Scanning for seed file: $isSeedFilePresent");

      if (isSeedFilePresent) {
        runBackend(context).then((value) {
          logger.i("Backend started");
        });
      } else {
        // No seed file: let the user choose whether they want to create a new
        // wallet or import their old one
        GoRouter.of(context).go(Onboarding.route);
        return;
      }

      switch (position) {
        case StableScreen.label:
          GoRouter.of(context).go(StableScreen.route);
        case TradeScreen.label:
          GoRouter.of(context).go(TradeScreen.route);
        default:
          GoRouter.of(context).go(WalletScreen.route);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    return const Center(child: CircularProgressIndicator());
  }
}
