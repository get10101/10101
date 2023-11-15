import 'package:flutter/material.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/features/welcome/loading_screen.dart';
import 'package:get_10101/common/scaffold_with_nav_bar.dart';
import 'package:get_10101/common/settings/app_info_screen.dart';
import 'package:get_10101/common/settings/collab_close_screen.dart';
import 'package:get_10101/common/settings/force_close_screen.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/common/settings/share_logs_screen.dart';
import 'package:get_10101/features/welcome/onboarding.dart';
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

GoRouter createRoutes() {
  return GoRouter(
      navigatorKey: rootNavigatorKey,
      initialLocation: LoadingScreen.route,
      routes: <RouteBase>[
        GoRoute(
          path: LoadingScreen.route,
          pageBuilder: (context, state) => NoTransitionPage<void>(
            child: LoadingScreen(
              future: state.extra as Future<void>?,
            ),
          ),
        ),
        GoRoute(
            path: WelcomeScreen.route,
            parentNavigatorKey: rootNavigatorKey,
            pageBuilder: (context, state) => const NoTransitionPage<void>(
                  child: WelcomeScreen(),
                )),
        GoRoute(
            path: Onboarding.route,
            pageBuilder: (context, state) => const NoTransitionPage<void>(
                  child: Onboarding(),
                ),
            routes: <RouteBase>[
              GoRoute(
                  path: SeedPhraseImporter.subRouteName,
                  parentNavigatorKey: rootNavigatorKey,
                  pageBuilder: (context, state) => const NoTransitionPage<void>(
                        child: SeedPhraseImporter(),
                      )),
            ]),
        GoRoute(
            path: SettingsScreen.route,
            pageBuilder: (BuildContext context, GoRouterState state) {
              return CustomTransitionPage<void>(
                transitionsBuilder: (BuildContext context, Animation<double> animation,
                    Animation<double> secondaryAnimation, Widget child) {
                  const begin = Offset(-1.0, 0.0);
                  const end = Offset.zero;
                  const curve = Curves.ease;

                  var tween = Tween(begin: begin, end: end).chain(CurveTween(curve: curve));

                  return SlideTransition(
                    position: animation.drive(tween),
                    child: child,
                  );
                },
                child: const SettingsScreen(),
              );
            },
            routes: <RouteBase>[
              GoRoute(
                path: AppInfoScreen.subRouteName,
                // Use root navigator so the screen overlays the application shell
                parentNavigatorKey: rootNavigatorKey,
                builder: (BuildContext context, GoRouterState state) {
                  return const AppInfoScreen();
                },
              ),
              GoRoute(
                path: ShareLogsScreen.subRouteName,
                // Use root navigator so the screen overlays the application shell
                parentNavigatorKey: rootNavigatorKey,
                builder: (BuildContext context, GoRouterState state) {
                  return const ShareLogsScreen();
                },
              ),
              GoRoute(
                path: CollabCloseScreen.subRouteName,
                // Use root navigator so the screen overlays the application shell
                parentNavigatorKey: rootNavigatorKey,
                builder: (BuildContext context, GoRouterState state) {
                  return const CollabCloseScreen();
                },
              ),
              GoRoute(
                path: ForceCloseScreen.subRouteName,
                // Use root navigator so the screen overlays the application shell
                parentNavigatorKey: rootNavigatorKey,
                builder: (BuildContext context, GoRouterState state) {
                  return const ForceCloseScreen();
                },
              )
            ]),
        ShellRoute(
          navigatorKey: shellNavigatorKey,
          builder: (BuildContext context, GoRouterState state, Widget child) {
            return ScaffoldWithNavBar(
              child: child,
            );
          },
          routes: <RouteBase>[
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
      ]);
}
