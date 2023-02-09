import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart' as foundation;
import 'package:flutter_native_splash/flutter_native_splash.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/features/trade/settings_screen.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/wallet/receive_screen.dart';
import 'package:get_10101/features/wallet/scanner_screen.dart';
import 'package:get_10101/features/wallet/send_screen.dart';
import 'package:get_10101/features/wallet/settings_screen.dart';
import 'package:get_10101/common/app_bar_wrapper.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';
import 'common/amount_denomination_change_notifier.dart';
import 'features/trade/trade_screen.dart';
import 'features/wallet/wallet_screen.dart';
import 'ffi.dart';

final GlobalKey<NavigatorState> _rootNavigatorKey = GlobalKey<NavigatorState>(debugLabel: 'root');
final GlobalKey<NavigatorState> _shellNavigatorKey = GlobalKey<NavigatorState>(debugLabel: 'shell');

void main() {
  WidgetsBinding widgetsBinding = WidgetsFlutterBinding.ensureInitialized();
  FlutterNativeSplash.preserve(widgetsBinding: widgetsBinding);

  final config = FLog.getDefaultConfigurations();
  config.activeLogLevel = LogLevel.DEBUG;

  FLog.applyConfigurations(config);
  runApp(MultiProvider(providers: [
    ChangeNotifierProvider(create: (context) => TradeValuesChangeNotifier()),
    ChangeNotifierProvider(create: (context) => AmountDenominationChangeNotifier()),
  ], child: const TenTenOneApp()));
}

class TenTenOneApp extends StatefulWidget {
  const TenTenOneApp({Key? key}) : super(key: key);

  @override
  State<TenTenOneApp> createState() => _TenTenOneAppState();
}

class _TenTenOneAppState extends State<TenTenOneApp> {
  final GoRouter _router = GoRouter(
    navigatorKey: _rootNavigatorKey,
    initialLocation: WalletScreen.route,
    routes: <RouteBase>[
      ShellRoute(
        navigatorKey: _shellNavigatorKey,
        builder: (BuildContext context, GoRouterState state, Widget child) {
          return ScaffoldWithNavBar(child: child);
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
                parentNavigatorKey: _rootNavigatorKey,
                builder: (BuildContext context, GoRouterState state) {
                  return const SendScreen();
                },
              ),
              GoRoute(
                path: ReceiveScreen.subRouteName,
                // Use root navigator so the screen overlays the application shell
                parentNavigatorKey: _rootNavigatorKey,
                builder: (BuildContext context, GoRouterState state) {
                  return const ReceiveScreen();
                },
              ),
              GoRoute(
                path: ScannerScreen.subRouteName,
                parentNavigatorKey: _rootNavigatorKey,
                builder: (BuildContext context, GoRouterState state) {
                  return const ScannerScreen();
                },
              ),
              GoRoute(
                  path: WalletSettingsScreen.subRouteName,
                  parentNavigatorKey: _rootNavigatorKey,
                  builder: (BuildContext context, GoRouterState state) {
                    return const WalletSettingsScreen();
                  })
            ],
          ),
          GoRoute(
            path: TradeScreen.route,
            builder: (BuildContext context, GoRouterState state) {
              return const TradeScreen();
            },
            routes: <RouteBase>[
              GoRoute(
                  path: TradeSettingsScreen.subRouteName,
                  parentNavigatorKey: _rootNavigatorKey,
                  builder: (BuildContext context, GoRouterState state) {
                    return const TradeSettingsScreen();
                  })
            ],
          ),
        ],
      ),
    ],
  );

  @override
  void initState() {
    super.initState();
    init();
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp.router(
      title: "10101",
      theme: ThemeData(
        primarySwatch: Colors.blue,
        extensions: const <ThemeExtension<dynamic>>[
          TradeTheme(),
        ],
      ),
      routerConfig: _router,
      debugShowCheckedModeBanner: false,
    );
  }

  Future<void> init() async {
    try {
      await setupRustLogging();

      setState(() {
        FLog.info(text: "10101 is ready!");
      });
    } on FfiException catch (error) {
      FLog.error(text: "Failed to initialise: Error: ${error.message}", exception: error);
    } catch (error) {
      FLog.error(text: "Failed to initialise: Unknown error");
    } finally {
      FlutterNativeSplash.remove();
    }
  }

  Future<void> setupRustLogging() async {
    api.initLogging().listen((event) {
      // Only log to Dart file in release mode - in debug mode it's easier to
      // use stdout
      if (foundation.kReleaseMode) {
        FLog.logThis(text: '${event.target}: ${event.msg}', type: LogLevel.DEBUG);
      }
    });
  }
}

// Wrapper for the main application screens
class ScaffoldWithNavBar extends StatelessWidget {
  const ScaffoldWithNavBar({
    required this.child,
    Key? key,
  }) : super(key: key);

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: child,
      appBar: const PreferredSize(
          preferredSize: Size.fromHeight(40), child: SafeArea(child: AppBarWrapper())),
      bottomNavigationBar: BottomNavigationBar(
        items: const <BottomNavigationBarItem>[
          BottomNavigationBarItem(
            icon: Icon(Icons.wallet),
            label: WalletScreen.label,
          ),
          BottomNavigationBarItem(
            icon: Icon(Icons.bar_chart),
            label: TradeScreen.label,
          ),
        ],
        currentIndex: _calculateSelectedIndex(context),
        onTap: (int idx) => _onItemTapped(idx, context),
      ),
    );
  }

  static int _calculateSelectedIndex(BuildContext context) {
    final String location = GoRouterState.of(context).location;
    if (location.startsWith(WalletScreen.route)) {
      return 0;
    }
    if (location.startsWith(TradeScreen.route)) {
      return 1;
    }
    return 0;
  }

  void _onItemTapped(int index, BuildContext context) {
    switch (index) {
      case 0:
        GoRouter.of(context).go(WalletScreen.route);
        break;
      case 1:
        GoRouter.of(context).go(TradeScreen.route);
        break;
    }
  }
}
