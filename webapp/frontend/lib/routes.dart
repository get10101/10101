import 'package:flutter/material.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/scaffold_with_nav.dart';
import 'package:get_10101/settings/settings_screen.dart';
import 'package:get_10101/trade/trade_screen.dart';
import 'package:get_10101/wallet/wallet_screen.dart';
import 'package:go_router/go_router.dart';

final goRouter = GoRouter(
    navigatorKey: rootNavigatorKey,
    initialLocation: TradeScreen.route,
    debugLogDiagnostics: true,
    routes: [
      StatefulShellRoute.indexedStack(
        builder: (context, state, navigationShell) {
          return ScaffoldWithNestedNavigation(navigationShell: navigationShell);
        },
        branches: [
          StatefulShellBranch(
            navigatorKey: shellNavigatorKeyTrading,
            routes: [
              GoRoute(
                path: TradeScreen.route,
                pageBuilder: (context, state) => NoTransitionPage(
                  child: routeChild(const TradeScreen()),
                ),
              ),
            ],
          ),
          StatefulShellBranch(
            navigatorKey: shellNavigatorKeyWallet,
            routes: [
              GoRoute(
                path: WalletScreen.route,
                pageBuilder: (context, state) => NoTransitionPage(
                  child: routeChild(const WalletScreen()),
                ),
              ),
            ],
          ),
          StatefulShellBranch(
            navigatorKey: shellNavigatorKeySettings,
            routes: [
              GoRoute(
                path: SettingsScreen.route,
                pageBuilder: (context, state) => NoTransitionPage(
                  child: routeChild(const SettingsScreen()),
                ),
              ),
            ],
          )
        ],
      )
    ]);

Scaffold routeChild(Widget child) {
  return Scaffold(body: Container(padding: const EdgeInsets.all(25), child: Center(child: child)));
}
