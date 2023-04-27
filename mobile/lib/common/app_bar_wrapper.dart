import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/dev_mode_screen.dart';
import 'package:get_10101/features/trade/settings_screen.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/features/wallet/scanner_screen.dart';
import 'package:get_10101/features/wallet/settings_screen.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:go_router/go_router.dart';

class AppBarWrapper extends StatelessWidget {
  const AppBarWrapper({
    Key? key,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    final currentRoute = GoRouter.of(context).location;

    var actionButtons = <Widget>[];
    Widget? settingsButton;

    if (currentRoute == WalletScreen.route) {
      actionButtons.add(IconButton(
        icon: const Icon(Icons.qr_code_scanner),
        tooltip: 'Scanner',
        onPressed: () {
          context.go(ScannerScreen.route);
        },
      ));

      settingsButton = IconButton(
        icon: const Icon(Icons.settings),
        tooltip: 'Settings',
        onPressed: () {
          context.go(WalletSettingsScreen.route);
        },
      );
    }

    if (currentRoute == TradeScreen.route) {
      settingsButton = IconButton(
        icon: const Icon(Icons.settings),
        tooltip: 'Settings',
        onPressed: () {
          context.go(TradeSettingsScreen.route);
        },
      );

      actionButtons.add(IconButton(
        icon: const Icon(Icons.developer_mode),
        tooltip: 'Developers',
        onPressed: () {
          context.go(TradeDevModeScreen.route);
        },
      ));
    }

    FLog.info(text: 'Action buttons $actionButtons');

    return AppBar(
      elevation: 0,
      backgroundColor: Colors.transparent,
      iconTheme: const IconThemeData(color: Colors.black),
      leading: settingsButton,
      actions: actionButtons,
    );
  }
}
