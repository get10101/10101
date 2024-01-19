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
                pageBuilder: (context, state) => const NoTransitionPage(
                  child: TradeScreen(),
                ),
              ),
            ],
          ),
          StatefulShellBranch(
            navigatorKey: shellNavigatorKeyWallet,
            routes: [
              GoRoute(
                path: WalletScreen.route,
                pageBuilder: (context, state) => const NoTransitionPage(
                  child: WalletScreen(),
                ),
              ),
            ],
          ),
          StatefulShellBranch(
            navigatorKey: shellNavigatorKeySettings,
            routes: [
              GoRoute(
                path: SettingsScreen.route,
                pageBuilder: (context, state) => const NoTransitionPage(
                  child: SettingsScreen(),
                ),
              ),
            ],
          )
        ],
      )
    ]);
