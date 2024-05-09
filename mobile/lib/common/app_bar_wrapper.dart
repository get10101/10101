import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/features/trade/contract_symbol_icon.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/main.dart';
import 'package:go_router/go_router.dart';

class AppBarWrapper extends StatelessWidget {
  const AppBarWrapper({
    Key? key,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    const appBarHeight = 35.0;
    final String location = GoRouterState.of(context).location;
    final leadingButton = Row(mainAxisSize: MainAxisSize.min, children: [
      IconButton(
        icon: const Icon(Icons.settings),
        tooltip: 'Settings',
        onPressed: () {
          GoRouter.of(context).go(SettingsScreen.route, extra: location);
        },
      )
    ]);

    return Container(
      margin: const EdgeInsets.only(left: 10.0, right: 5.0),
      child: AppBar(
          centerTitle: true,
          elevation: 0,
          backgroundColor: appBackgroundColor,
          iconTheme: const IconThemeData(color: tenTenOnePurple, size: appBarHeight - 8.0),
          leading: leadingButton,
          title: location == TradeScreen.route
              ? Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                      const ContractSymbolIcon(height: 23, width: 23),
                      const SizedBox(width: 5),
                      Text(
                        ContractSymbol.btcusd.label,
                        style: const TextStyle(fontSize: 17),
                      )
                    ])
              : null),
    );
  }
}
