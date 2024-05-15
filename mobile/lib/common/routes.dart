import 'package:flutter/material.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/settings/channel_screen.dart';
import 'package:get_10101/common/settings/emergency_kit_screen.dart';
import 'package:get_10101/common/settings/user_screen.dart';
import 'package:get_10101/common/settings/wallet_settings.dart';
import 'package:get_10101/common/status_screen.dart';
import 'package:get_10101/common/background_task_dialog_screen.dart';
import 'package:get_10101/features/wallet/domain/destination.dart';
import 'package:get_10101/features/wallet/send/send_onchain_screen.dart';
import 'package:get_10101/features/welcome/error_screen.dart';
import 'package:get_10101/features/welcome/loading_screen.dart';
import 'package:get_10101/common/scaffold_with_nav_bar.dart';
import 'package:get_10101/common/settings/app_info_screen.dart';
import 'package:get_10101/common/settings/collab_close_screen.dart';
import 'package:get_10101/common/settings/force_close_screen.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/common/settings/share_logs_screen.dart';
import 'package:get_10101/features/welcome/onboarding.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';
import 'package:get_10101/features/wallet/receive_screen.dart';
import 'package:get_10101/features/wallet/scanner_screen.dart';
import 'package:get_10101/features/welcome/seed_import_screen.dart';
import 'package:get_10101/common/settings/seed_screen.dart';
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
            path: ErrorScreen.route,
            parentNavigatorKey: rootNavigatorKey,
            pageBuilder: (context, state) => const NoTransitionPage<void>(
                  child: ErrorScreen(),
                )),
        ShellRoute(
            builder: (BuildContext context, GoRouterState state, Widget child) {
              return BackgroundTaskDialogScreen(
                child: child,
              );
            },
            routes: [
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
                      child: SettingsScreen(location: state.extra! as String),
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
                      path: SeedScreen.subRouteName,
                      // Use root navigator so the screen overlays the application shell
                      parentNavigatorKey: rootNavigatorKey,
                      builder: (BuildContext context, GoRouterState state) {
                        return const SeedScreen();
                      },
                    ),
                    GoRoute(
                      path: WalletSettings.subRouteName,
                      // Use root navigator so the screen overlays the application shell
                      parentNavigatorKey: rootNavigatorKey,
                      builder: (BuildContext context, GoRouterState state) {
                        return const WalletSettings();
                      },
                    ),
                    GoRoute(
                      path: UserSettings.subRouteName,
                      // Use root navigator so the screen overlays the application shell
                      parentNavigatorKey: rootNavigatorKey,
                      builder: (BuildContext context, GoRouterState state) {
                        return const UserSettings();
                      },
                    ),
                    GoRoute(
                      path: ChannelScreen.subRouteName,
                      // Use root navigator so the screen overlays the application shell
                      parentNavigatorKey: rootNavigatorKey,
                      builder: (BuildContext context, GoRouterState state) {
                        return const ChannelScreen();
                      },
                    ),
                    GoRoute(
                      path: StatusScreen.subRouteName,
                      // Use root navigator so the screen overlays the application shell
                      parentNavigatorKey: rootNavigatorKey,
                      builder: (BuildContext context, GoRouterState state) {
                        return const StatusScreen();
                      },
                    ),
                    GoRoute(
                      path: EmergencyKitScreen.subRouteName,
                      // Use root navigator so the screen overlays the application shell
                      parentNavigatorKey: rootNavigatorKey,
                      builder: (BuildContext context, GoRouterState state) {
                        return const EmergencyKitScreen();
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
              StatefulShellRoute.indexedStack(
                builder: (BuildContext context, GoRouterState state, Widget child) {
                  return ScaffoldWithNavBar(
                    child: child,
                  );
                },
                branches: <StatefulShellBranch>[
                  StatefulShellBranch(routes: [
                    GoRoute(
                      path: WalletScreen.route,
                      builder: (BuildContext context, GoRouterState state) {
                        return const WalletScreen();
                      },
                      routes: <RouteBase>[
                        GoRoute(
                          path: SendOnChainScreen.subRouteName,
                          // Use root navigator so the screen overlays the application shell
                          parentNavigatorKey: rootNavigatorKey,
                          builder: (BuildContext context, GoRouterState state) {
                            return SendOnChainScreen(destination: state.extra as OnChainAddress);
                          },
                        ),
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
                  ]),
                  StatefulShellBranch(routes: [
                    GoRoute(
                      path: TradeScreen.route,
                      builder: (BuildContext context, GoRouterState state) {
                        return const TradeScreen();
                      },
                      routes: const [],
                    ),
                  ])
                ],
              ),
            ]),
      ]);
}
