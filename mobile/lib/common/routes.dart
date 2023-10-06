import 'package:flutter/material.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/loading_screen.dart';
import 'package:get_10101/common/scaffold_with_nav_bar.dart';
import 'package:get_10101/features/stable/stable_screen.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';
import 'package:get_10101/features/wallet/onboarding/onboarding_screen.dart';
import 'package:get_10101/features/wallet/receive_screen.dart';
import 'package:get_10101/features/wallet/scanner_screen.dart';
import 'package:get_10101/features/welcome/seed_import_screen.dart';
import 'package:get_10101/features/wallet/seed_screen.dart';
import 'package:get_10101/features/wallet/send/send_screen.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/features/welcome/welcome_screen.dart';
import 'package:go_router/go_router.dart';
import 'package:get_10101/features/welcome/new_wallet_screen.dart';

GoRouter createRoutes() {
  return GoRouter(
      navigatorKey: rootNavigatorKey,
      initialLocation: LoadingScreen.route,
      routes: <RouteBase>[
        ShellRoute(
          navigatorKey: shellNavigatorKey,
          builder: (BuildContext context, GoRouterState state, Widget child) {
            return ScaffoldWithNavBar(
              child: child,
            );
          },
          routes: <RouteBase>[
            GoRoute(
              path: LoadingScreen.route,
              builder: (BuildContext context, GoRouterState state) {
                return const LoadingScreen();
              },
            ),
            GoRoute(
              path: WalletScreen.route,
              builder: (BuildContext context, GoRouterState state) {
                return const WalletScreen();
              },
              routes: <RouteBase>[
                GoRoute(
                  path: SendScreen.subRouteName,
                  // Use root navigator so the screen overlays the application shell
                  parentNavigatorKey: rootNavigatorKey,
                  builder: (BuildContext context, GoRouterState state) {
                    if (state.extra != null) {
                      return SendScreen(encodedDestination: state.extra as String?);
                    }
                    return const SendScreen();
                  },
                ),
                GoRoute(
                  path: SeedScreen.subRouteName,
                  // Use root navigator so the screen overlays the application shell
                  parentNavigatorKey: rootNavigatorKey,
                  builder: (BuildContext context, GoRouterState state) {
                    return const SeedScreen();
                  },
                ),
                GoRoute(
                    path: OnboardingScreen.subRouteName,
                    parentNavigatorKey: rootNavigatorKey,
                    builder: (BuildContext context, GoRouterState state) {
                      return const OnboardingScreen();
                    }),
                GoRoute(
                  path: ReceiveScreen.subRouteName,
                  // Use root navigator so the screen overlays the application shell
                  parentNavigatorKey: rootNavigatorKey,
                  builder: (BuildContext context, GoRouterState state) {
                    if (state.extra != null) {
                      return ReceiveScreen(walletType: state.extra as WalletType);
                    }
                    return const ReceiveScreen();
                  },
                ),
                GoRoute(
                  path: ScannerScreen.subRouteName,
                  parentNavigatorKey: rootNavigatorKey,
                  builder: (BuildContext context, GoRouterState state) {
                    return const ScannerScreen();
                  },
                ),
              ],
            ),
            GoRoute(
                path: StableScreen.route,
                builder: (BuildContext context, GoRouterState state) {
                  return const StableScreen();
                }),
            GoRoute(
              path: TradeScreen.route,
              builder: (BuildContext context, GoRouterState state) {
                return const TradeScreen();
              },
              routes: const [],
            ),
          ],
        ),
        GoRoute(
            path: NewWalletScreen.route,
            builder: (BuildContext context, GoRouterState state) {
              return const NewWalletScreen();
            },
            routes: <RouteBase>[
              GoRoute(
                  path: SeedPhraseImporter.subRouteName,
                  parentNavigatorKey: rootNavigatorKey,
                  builder: (BuildContext context, GoRouterState state) {
                    return const SeedPhraseImporter();
                  },
                  routes: const []),
            ]),
        GoRoute(
            path: WelcomeScreen.route,
            parentNavigatorKey: rootNavigatorKey,
            builder: (BuildContext context, GoRouterState state) {
              return const WelcomeScreen();
            },
            routes: const []),
      ]);
}
