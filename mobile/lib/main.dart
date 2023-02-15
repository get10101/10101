import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart';
import 'dart:io';
import 'package:flutter_native_splash/flutter_native_splash.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/order_change_notifier.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/features/trade/settings_screen.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/wallet/balance_change_notifier.dart';
import 'package:get_10101/features/wallet/receive_screen.dart';
import 'package:get_10101/features/wallet/scanner_screen.dart';
import 'package:get_10101/features/wallet/send_screen.dart';
import 'package:get_10101/features/wallet/settings_screen.dart';
import 'package:get_10101/common/app_bar_wrapper.dart';
import 'package:get_10101/util/constants.dart';
import 'package:go_router/go_router.dart';
import 'package:path_provider/path_provider.dart';
import 'package:provider/provider.dart';
import 'common/amount_denomination_change_notifier.dart';
import 'features/trade/trade_screen.dart';
import 'features/wallet/wallet_screen.dart';
import 'ffi.dart' as rust;

final GlobalKey<NavigatorState> _rootNavigatorKey = GlobalKey<NavigatorState>(debugLabel: 'root');
final GlobalKey<NavigatorState> _shellNavigatorKey = GlobalKey<NavigatorState>(debugLabel: 'shell');

void main() {
  WidgetsBinding widgetsBinding = WidgetsFlutterBinding.ensureInitialized();
  FlutterNativeSplash.preserve(widgetsBinding: widgetsBinding);

  final config = FLog.getDefaultConfigurations();
  config.activeLogLevel = LogLevel.DEBUG;
  config.formatType = FormatType.FORMAT_CUSTOM;
  config.timestampFormat = 'yyyy-MM-dd HH:mm:ss.SSS';
  config.fieldOrderFormatCustom = [
    FieldName.TIMESTAMP,
    FieldName.LOG_LEVEL,
    FieldName.TEXT,
    FieldName.STACKTRACE
  ];
  config.customClosingDivider = "";
  config.customOpeningDivider = "| ";

  FLog.applyConfigurations(config);
  runApp(MultiProvider(providers: [
    ChangeNotifierProvider(create: (context) => TradeValuesChangeNotifier(TradeValuesService())),
    ChangeNotifierProvider(create: (context) => AmountDenominationChangeNotifier()),
    ChangeNotifierProvider(create: (context) => SubmitOrderChangeNotifier(OrderService())),
    ChangeNotifierProvider(create: (context) => OrderChangeNotifier(OrderService())),
    ChangeNotifierProvider(create: (context) => BalanceChangeNotifier())
  ], child: const TenTenOneApp()));
}

class TenTenOneApp extends StatefulWidget {
  const TenTenOneApp({Key? key}) : super(key: key);

  @override
  State<TenTenOneApp> createState() => _TenTenOneAppState();
}

class _TenTenOneAppState extends State<TenTenOneApp> {
  final OrderService orderService = OrderService();

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
    OrderChangeNotifier orderChangeNotifier = context.read<OrderChangeNotifier>();

    super.initState();
    init();
    orderService.subscribeToOrderNotifications(orderChangeNotifier);
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

      final appSupportDir = await getApplicationSupportDirectory();
      FLog.info(text: "App data will be stored in: $appSupportDir");

      api.run(appDir: appSupportDir.path).listen((event) {
        if (event is Event_Ready) {
          FLog.info(text: "10101 is ready!");
        } else if (event is Event_Init) {
          FLog.info(text: event.field0);
        } else if (event is Event_WalletInfo) {
          context.read<BalanceChangeNotifier>().update(event.field0);
        } else {
          FLog.warning(text: "Received unexpected event: $event");
        }
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
    rust.api.initLogging().listen((event) {
      // TODO: this should not be required if we enable mobile loggers for FLog.
      if (Platform.isAndroid || Platform.isIOS) {
        FLog.logThis(
            text: event.target != "" ? '${event.target}: ${event.msg}' : event.msg,
            type: mapLogLevel(event.level));
      }
    });
  }

  LogLevel mapLogLevel(String level) {
    switch (level) {
      case "INFO":
        return LogLevel.INFO;
      case "DEBUG":
        return LogLevel.DEBUG;
      case "ERROR":
        return LogLevel.ERROR;
      case "WARN":
        return LogLevel.WARNING;
      case "TRACE":
        return LogLevel.TRACE;
      default:
        return LogLevel.DEBUG;
    }
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
        items: <BottomNavigationBarItem>[
          BottomNavigationBarItem(
            icon: Container(key: tabWallet, child: const Icon(Icons.wallet)),
            label: WalletScreen.label,
          ),
          BottomNavigationBarItem(
            icon: Container(key: tabTrade, child: const Icon(Icons.bar_chart)),
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
